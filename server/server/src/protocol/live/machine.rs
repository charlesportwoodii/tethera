use crate::config::ApplicationConfig;
use crate::machine::{Installed, Machine};
use crate::protocol::live::{LiveAssets, LiveConversations, LiveTerminals, TreeWatcher};
use crate::protocol::ports::{
    ConversationPort, EnrollOffer, Enrolment, MachinePort, TreeSnapshot,
};
use crate::services::{DeviceService, PairingService};
use sea_orm::DatabaseConnection;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tethera_common::protocol::capability::{self, CapabilityId, CapabilitySet};
use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::protocol::handshake::{DeviceRecord, ServerInfo};
use tethera_common::protocol::response::{Describe, Limits};
use tethera_common::protocol::watch::WatchEvent;
use tethera_common::structs::agent::{Agent, AgentProfile};
use tethera_common::structs::conversation::ConversationFilter;
use tethera_common::structs::device::DeviceState;
use tethera_common::structs::primitives::Timestamp;
use tethera_common::traits::AgentTrait;
use tokio::sync::broadcast;

pub struct LiveMachine {
    devices: Arc<DeviceService>,
    pairing: Arc<PairingService>,
    terminals: Arc<LiveTerminals>,
    conversations: Arc<LiveConversations>,
    // What the ports other than the terminal one can do. Settled once, because
    // none of them changes while the process runs.
    base_capabilities: CapabilitySet,
    // The terminal half does change: a backend can appear or die under a
    // running server, and a capability advertised after its backend died is the
    // exact failure a capability set exists to prevent. Re-read on every
    // Describe, republished by the runtime's heartbeat.
    terminals_available: Arc<AtomicBool>,
    // Resolved once at construction. The label and the id are what a person
    // reads off a pairing screen, and a value that could change between two
    // frames of one handshake would describe two different machines.
    info: ServerInfo,
    events: broadcast::Sender<WatchEvent>,
    /// Turns each tree read into events for anything watching.
    ///
    /// Every read goes through `tree`, so diffing there is enough to make a watch
    /// fire without a separate poll loop for a machine nobody is watching.
    watcher: TreeWatcher,
}

impl LiveMachine {
    /// § 20 of the protocol design. The control-frame bound is deliberately
    /// under what `FrameCodec` enforces, so a client that sizes to what it is
    /// told is always inside what the server will accept.
    pub const MAX_CONTROL_FRAME: u32 = 64 * 1024;
    pub const MAX_STREAMS: u16 = 64;
    pub const TRANSCRIPT_PAGE: u16 = 200;
    pub const SCROLLBACK_PAGE: u16 = 500;

    /// Enough for the machine watch's own tree events. Nothing publishes to it
    /// until a terminal backend exists.
    pub const EVENT_CAPACITY: usize = 64;

    /// How many conversations the tree carries.
    ///
    /// The tree is one frame, and a frame is bounded by bytes — so "every
    /// conversation" is not a number this can be. A machine with two hundred
    /// sessions on disk encodes to well past the control cap, and the whole tree
    /// then fails to send rather than arriving long: no workspaces, no panes, no
    /// conversations, on a screen that shows only the machine.
    ///
    /// This is the newest that fit comfortably. `ListConversations` pages the
    /// rest, which is what a client uses for a full index anyway; the tree's
    /// conversations are the few twigs drawn beside each machine.
    pub const TREE_CONVERSATIONS: u16 = 40;

    pub fn new(
        config: Arc<ApplicationConfig>,
        db: Arc<DatabaseConnection>,
        endpoint_id: String,
        base_capabilities: CapabilitySet,
        terminals_available: Arc<AtomicBool>,
        terminals: Arc<LiveTerminals>,
        conversations: Arc<LiveConversations>,
    ) -> Self {
        let (events, _) = broadcast::channel(Self::EVENT_CAPACITY);
        let watcher = TreeWatcher::new(events.clone());

        Self {
            info: Machine::info(&config, &endpoint_id),
            devices: DeviceService::new_shared(db.clone()),
            pairing: PairingService::new_shared(db),
            terminals,
            conversations,
            base_capabilities,
            terminals_available,
            events,
            watcher,
        }
    }

    /// What this port itself can do, as opposed to what the machine advertises.
    ///
    /// `LivePorts` unions this with what the other three say, so a capability
    /// arrives with the code that honours it rather than from a list somebody
    /// has to remember to prune.
    ///
    /// `recent_cwds` is here rather than behind the terminal probe: the
    /// directories are read off the sessions on disk, which a machine with no
    /// terminal backend still has. An empty answer from it means no agent has
    /// worked in a directory that still exists, which is a real state and not a
    /// missing feature.
    pub fn own_capabilities() -> CapabilitySet {
        [
            capability::AGENT_CATALOG,
            capability::RECENT_CWDS,
            capability::DEVICE_SELF_REVOKE,
        ]
        .into_iter()
        .map(CapabilityId::from)
        .collect()
    }

    /// What this machine will tell a client it can do, as of now.
    ///
    /// How many machine watches are open on this port.
    ///
    /// A behavioural accessor, and the only way to ask whether keeping the tree
    /// fresh is work anybody will read. A machine nobody is watching should do
    /// nothing at all rather than shell out to a terminal backend forever on a
    /// timer.
    pub fn watchers(&self) -> usize {
        self.events.receiver_count()
    }

    /// Computed rather than stored, because the terminal half can stop being
    /// true while the process runs.
    pub fn capabilities(&self) -> CapabilitySet {
        let mut capabilities = self.base_capabilities.clone();

        if self.terminals_ready() {
            capabilities.extend(self.terminals.capabilities());
            // Starting a conversation is a pane operation wearing a
            // conversation's name, so it lives or dies with the same probe
            // rather than with the half of the conversation port that reads
            // records off disk.
            capabilities.extend(LiveConversations::backed_capabilities());
        }

        capabilities
    }

    /// Whether a terminal backend answered the last time anything asked.
    pub fn terminals_ready(&self) -> bool {
        self.terminals_available.load(Ordering::SeqCst)
    }

    pub fn limits() -> Limits {
        Limits {
            max_control_frame: Self::MAX_CONTROL_FRAME,
            max_streams: Self::MAX_STREAMS,
            transcript_page: Self::TRANSCRIPT_PAGE,
            scrollback_page: Self::SCROLLBACK_PAGE,
            // Stated, and enforced by `put_ready`. A client that reads this
            // refuses an oversized file before the transfer rather than after
            // it, which over a relayed link is the difference between a refusal
            // and a wasted minute.
            max_upload: Some(LiveAssets::MAX_UPLOAD),
        }
    }

    fn connection(&self) -> &DatabaseConnection {
        self.devices.db()
    }

    // Stored timestamps are seconds, because that is what every row already
    // holds. The wire is milliseconds.
    fn now() -> i64 {
        chrono::Utc::now().timestamp()
    }

    fn to_millis(seconds: i64) -> Timestamp {
        Timestamp(seconds * 1_000)
    }

    fn record(endpoint_id: &str, name: String, paired_at: Option<i64>) -> DeviceRecord {
        DeviceRecord {
            id: Machine::device_id(endpoint_id),
            name,
            // A device reaches Active only through `activate`, which stamps
            // paired_at. The fallback is here because the column is nullable,
            // not because the case is reachable.
            paired_at: Self::to_millis(paired_at.unwrap_or_default()),
        }
    }
}

impl MachinePort for LiveMachine {
    async fn describe(&self) -> Describe {
        Describe {
            server: self.info.clone(),
            capabilities: self.capabilities(),
            limits: Self::limits(),
        }
    }

    fn server_info(&self) -> ServerInfo {
        self.info.clone()
    }

    /// The harnesses this machine can actually launch.
    ///
    /// `Agent::ALL` is a compile-time list of what this build knows how to drive,
    /// which is a different question from what is installed here. Answering the
    /// first would put a row in front of somebody whose start fails the moment
    /// they tap it, with nothing on the way in to warn them.
    ///
    /// An empty answer is a real state — a machine with no agent installed — and
    /// is why `agent_catalog` stays advertised rather than disappearing with the
    /// last harness: a client that is told the catalog is empty can say so, and
    /// one that sees no capability at all cannot tell that from an old server.
    async fn agent_profiles(&self) -> Vec<AgentProfile> {
        Agent::ALL
            .iter()
            .filter(|agent| Installed::has(agent.binary()))
            .map(Agent::profile)
            .collect()
    }

    async fn recent_cwds(&self, limit: u16) -> Vec<String> {
        self.conversations.recent_cwds(limit).await
    }

    async fn tree(&self) -> Result<TreeSnapshot, WireError> {
        let terminal = match self.terminals.tree().await {
            Ok(tree) => Some(tree),
            // A machine that advertises no pane capability has no terminal
            // backend, so it has no workspaces, and saying so is not the same
            // as failing. It also matters that this answers at all: a machine
            // watch's first frame is mandatory, and `watch.rs` closes the
            // stream without one when this errors.
            Err(_) if !self.terminals_ready() => None,
            // A backend this machine does advertise, which then failed, is a
            // real failure. Reporting it as an empty tree would tell a person
            // their workspaces had gone.
            Err(error) => return Err(error),
        };

        let (workspaces, tabs, panes) = terminal.unwrap_or_default();

        let snapshot = TreeSnapshot {
            workspaces,
            tabs,
            panes,
            conversations: self
                .conversations
                .list(ConversationFilter::All, None, Self::TREE_CONVERSATIONS)
                .await
                .items,
        };

        // Diffed against the last read, so anything watching hears about a change
        // without a second source of truth to disagree with this one.
        self.watcher.observe(snapshot.clone());

        Ok(snapshot)
    }

    fn tree_events(&self) -> broadcast::Receiver<WatchEvent> {
        self.events.subscribe()
    }

    async fn enrolment(&self, endpoint_id: &str) -> Enrolment {
        let found = match self
            .devices
            .find_by_endpoint(self.connection(), endpoint_id)
            .await
        {
            Ok(found) => found,
            // `Revoked` rather than `Unknown`, because `Unknown` is an
            // admitting state: with a pairing window open the dispatcher offers
            // it enrolment, so an unreadable device table would let a revoked
            // device re-enrol by presenting itself as a stranger. `Revoked` is
            // refused for both intents, which is the only fail-closed answer
            // this signature can give. An enrolled device is told the wrong
            // reason, which is the right trade against admitting a revoked one.
            Err(error) => {
                tracing::error!(
                    %error,
                    "device lookup failed; refusing this connection as revoked"
                );

                return Enrolment::Revoked;
            }
        };

        let Some(device) = found else {
            return Enrolment::Unknown;
        };

        match device.state {
            DeviceState::Active => {
                // Best effort. A device that connected is a fact worth
                // recording, and failing to record it is not a reason to refuse
                // the connection.
                if let Err(error) = self
                    .devices
                    .touch(self.connection(), endpoint_id, Self::now())
                    .await
                {
                    tracing::warn!(%error, "could not record the device's last contact");
                }

                Enrolment::Known(Self::record(endpoint_id, device.name, device.paired_at))
            }
            // Revoked is distinct from unknown, and a banned device is at least
            // as refused as a revoked one. Reporting either as unknown would let
            // it re-enrol through an open window by presenting itself as a
            // stranger, which is what revocation exists to prevent.
            DeviceState::Revoked | DeviceState::Banned => Enrolment::Revoked,
            // Approved by nobody yet. Pending is the state a stranger is in, so
            // it enrols through the window like any other.
            DeviceState::Pending => Enrolment::Unknown,
        }
    }

    async fn pairing_window(&self) -> Option<EnrollOffer> {
        let now = Self::now();

        let row = match self.pairing.current_window(self.connection(), now).await {
            Ok(row) => row?,
            Err(error) => {
                tracing::error!(%error, "could not read the pairing window");

                return None;
            }
        };

        Some(EnrollOffer {
            server: self.info.clone(),
            // A value this machine did not write is not a length to draw. Only
            // `CODE_DIGITS` is ever stored, so this is unreachable rather than
            // defensive, but a zero-length code field is a screen a person
            // cannot type into.
            code_length: if row.code_length > 0 {
                PairingService::narrow(row.code_length)
            } else {
                PairingService::narrow(PairingService::CODE_DIGITS)
            },
            // Computed against the row rather than the ttl it was minted with,
            // so a client dialling four minutes in is told sixty seconds.
            expires_in_ms: u32::try_from((row.expires_at - now).max(0).saturating_mul(1_000))
                .unwrap_or(u32::MAX),
        })
    }

    async fn redeem_code(
        &self,
        endpoint_id: &str,
        code: &str,
        device_name: &str,
    ) -> Result<DeviceRecord, u8> {
        let device = self
            .pairing
            .redeem(endpoint_id, code, device_name, Self::now())
            .await?;

        Ok(Self::record(endpoint_id, device.name, device.paired_at))
    }

    async fn revoke(&self, device: &str) -> Result<(), WireError> {
        // `Rpc::handle` passes the device id where this signature documents an
        // endpoint id. A value that is neither is refused rather than read as
        // the one it is not.
        let Some(endpoint_id) = Machine::endpoint_id_of(device) else {
            return Err(WireError::NotFound {
                kind: EntityKind::Device,
            });
        };

        self.devices
            .set_state(
                self.connection(),
                endpoint_id,
                DeviceState::Revoked,
                Self::now(),
            )
            .await
            .map(|_| ())
            // The message is ours, not the database's. A `DbErr` here would
            // hand the machine's file paths to a paired device.
            .map_err(|error| {
                tracing::error!(%error, "could not revoke a device");

                WireError::Backend {
                    message: "this machine could not revoke the device".to_string(),
                }
            })
    }
}
