use iroh::endpoint::Connection;
use tethera_client_core::book::ServerBook;
use tethera_client_core::endpoint::ClientEndpoint;
use tethera_client_core::pairing::PairingSession;
use tethera_client_core::session::Session;
use tethera_client_core::settings::SettingsStore;
use tethera_common::protocol::handshake::{ClientInfo, Platform, ServerHello};
use tethera_common::structs::client::ServerEntry;
use tethera_common::structs::ids::ServerId;
use crate::downloads::Downloads;
use crate::panes::PaneAttachments;
use crate::machine_watch::MachineWatch;
use crate::watches::ConversationWatches;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;

pub struct AppState {
    version: String,
    endpoint: ClientEndpoint,
    book: ServerBook,
    client: ClientInfo,
    // A tokio mutex rather than a std one: submitting a code awaits an I/O round
    // trip while holding this, and a std guard is not Send across an await.
    //
    // Pairing is modal, so exactly one attempt exists. A second `pair_begin`
    // replaces this one, and dropping it resets the stream, which the machine
    // observes.
    pairing: Mutex<Option<PairingSession>>,
    watches: ConversationWatches,
    machine: MachineWatch,
    panes: PaneAttachments,
    // One live connection per machine, keyed by its id.
    //
    // A connection carries the completed handshake, and every request on it is
    // a stream rather than a new dial. Dialling per command means a QUIC
    // handshake and possibly a hole punch each time, which on a phone radio is
    // both slow and unreliable: a screen that fires three commands makes three
    // dials to the same machine and they contend with each other.
    live: Mutex<HashMap<String, Connection>>,
    settings: SettingsStore,
    downloads: Downloads,
    /// Set by `resumed` and taken by the next sweep, so the log carries the one
    /// pass whose outcome answers whether the resume recovered the transport.
    ///
    /// A flag rather than a line on every sweep: the list sweeps every five
    /// seconds for as long as it is open, and a log that records all of them
    /// rotates the interesting pass away before anybody reads it.
    resumed: AtomicBool,
}

impl AppState {
    pub const LOCKED: &'static str =
        "this phone is locked; authenticate to reach your machines again";

    pub fn new(
        endpoint: ClientEndpoint,
        book: ServerBook,
        client: ClientInfo,
        settings: SettingsStore,
        downloads: Downloads,
    ) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            endpoint,
            book,
            client,
            pairing: Mutex::new(None),
            watches: ConversationWatches::new(),
            machine: MachineWatch::new(),
            panes: PaneAttachments::new(),
            live: Mutex::new(HashMap::new()),
            settings,
            downloads,
            resumed: AtomicBool::new(false),
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn endpoint(&self) -> &ClientEndpoint {
        &self.endpoint
    }

    pub fn settings(&self) -> &SettingsStore {
        &self.settings
    }

    pub fn book(&self) -> &ServerBook {
        &self.book
    }

    pub fn client(&self) -> ClientInfo {
        self.client.clone()
    }

    pub fn pairing(&self) -> &Mutex<Option<PairingSession>> {
        &self.pairing
    }

    pub fn watches(&self) -> &ConversationWatches {
        &self.watches
    }

    pub fn machine_watch(&self) -> &MachineWatch {
        &self.machine
    }

    pub fn panes(&self) -> &PaneAttachments {
        &self.panes
    }

    pub fn downloads(&self) -> &Downloads {
        &self.downloads
    }

    /// What the machine will list this device as.
    ///
    /// No pairing screen collects a name, so this is derived rather than asked
    /// for. Letting a person edit it is a follow-up.
    pub fn device_name(&self) -> String {
        format!(
            "{} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    }

    /// A remembered machine, by id.
    pub fn entry(&self, id: &ServerId) -> Result<ServerEntry, String> {
        self.book()
            .entries()
            .into_iter()
            .find(|held| &held.server.id == id)
            .ok_or_else(|| "that machine is not paired".to_string())
    }

    /// A live, handshaken connection to a paired machine.
    ///
    /// Reused rather than redialled. Every command needs both halves — a
    /// connection without a completed handshake is one the dispatcher will not
    /// serve a request on — and both halves survive as long as the connection
    /// does, so paying for them per command buys nothing.
    ///
    /// Callers must not close what they are handed: it belongs to this map, and
    /// closing it takes every other caller's requests with it.
    pub async fn connect(&self, id: &ServerId) -> Result<Connection, String> {
        // The gate is here rather than on each command, and rather than on the
        // screen. Every call that reaches a machine dials through this one
        // function, so one check covers sending a prompt, reading a transcript,
        // opening a watch and interrupting an agent - and a screen that was
        // navigated around cannot reach past it.
        if !self.settings.unlocked() {
            return Err(Self::LOCKED.to_string());
        }

        let mut live = self.live.lock().await;

        // `close_reason` is `None` only while the connection is usable. Handing
        // back a closed one would fail every request on it with no attempt to
        // recover.
        if let Some(held) = live.get(id.as_str()) {
            if held.close_reason().is_none() {
                return Ok(held.clone());
            }

            live.remove(id.as_str());
        }

        let connection = self.dial(id).await?;

        live.insert(id.as_str().to_owned(), connection.clone());

        Ok(connection)
    }

    /// Called when this app returns to the foreground.
    ///
    /// Two things are wrong with the transport at this moment, and neither
    /// announces itself. iroh has been frozen for as long as the phone was
    /// away, so its NAT mappings have expired and the relay socket the
    /// operating system reclaimed has not been noticed — and on iOS iroh's own
    /// wake detection is switched off, so nothing tells it to look. The cached
    /// connections, meanwhile, still answer `close_reason() == None`, so
    /// `connect` would hand back a connection whose path is gone and every
    /// request on it would wait out the full deadline.
    ///
    /// So: hint, then forget. Neither is expensive, and this is the one moment
    /// where both are certainly needed.
    ///
    /// The log lines are the point of the shape. A resume either recovered the
    /// transport or it did not, and from the screen the two are the same picture
    /// of a machine that will not answer.
    pub async fn resumed(&self, hidden: u64) {
        log::info!(
            "resumed after {hidden}ms hidden; endpoint {}",
            self.endpoint.health()
        );

        // First, because everything after it resolves a hostname. The device's
        // own log is what put this in front of the hint: minutes of `Failed to
        // connect to relay server: unable to connect: Resolve failed` while the
        // phone plainly had a network, because the resolver still held the
        // nameservers from before it was suspended.
        if !self.endpoint.reset_dns() {
            log::warn!("the endpoint has no dns resolver to reset; it is closed");
        }

        if !self.endpoint.network_change().await {
            log::warn!(
                "the network-change hint was not taken within {:?}; the socket actor is not answering",
                ClientEndpoint::NUDGE_DEADLINE
            );
        }

        let dropped = {
            let mut live = self.live.lock().await;
            let held = live.len();

            live.clear();

            held
        };

        self.resumed.store(true, Ordering::SeqCst);

        log::info!(
            "dropped {dropped} cached connection(s) on resume; endpoint {}",
            self.endpoint.health()
        );
    }

    /// Whether a resume has happened that no sweep has reported on yet.
    ///
    /// Takes the flag, so exactly one sweep carries the answer.
    pub fn took_resume(&self) -> bool {
        self.resumed.swap(false, Ordering::SeqCst)
    }

    /// Throws away the held connection and dials again.
    ///
    /// `close_reason` only answers once *this* end has noticed, and a machine
    /// that stopped abruptly is not noticed until the idle timeout expires. Until
    /// then the cached connection looks usable and every request on it waits out
    /// the full deadline. A caller that has just seen a request fail knows more
    /// than `close_reason` does, and this is how it says so.
    pub async fn reconnect(&self, id: &ServerId) -> Result<Connection, String> {
        self.live.lock().await.remove(id.as_str());

        self.connect(id).await
    }

    async fn dial(&self, id: &ServerId) -> Result<Connection, String> {
        let entry = self.entry(id)?;

        let connection = self
            .endpoint()
            .dial(
                &entry.endpoint_id,
                entry.relay.as_deref(),
                &entry.direct_addrs,
            )
            .await
            .map_err(|error| error.to_string())?;

        match Session::open(&connection, self.client())
            .await
            .map_err(|error| error.to_string())?
        {
            ServerHello::Session { .. } => Ok(connection),
            // A machine that will not open a session will not serve a request
            // either. Saying so here beats a request that fails for a reason
            // that looks unrelated.
            ServerHello::Refuse(reason) => {
                Err(format!("{} refused this device: {reason:?}", entry.server.label))
            }
            ServerHello::EnrollPending { .. } => Err(format!(
                "{} no longer recognises this device and is offering to pair again",
                entry.server.label
            )),
        }
    }

    pub fn platform() -> Platform {
        #[cfg(target_os = "ios")]
        return Platform::Ios;

        #[cfg(target_os = "android")]
        return Platform::Android;

        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        Platform::Desktop
    }
}
