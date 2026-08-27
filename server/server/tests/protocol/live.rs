//! The ports the binary actually serves, and the accept loop that serves them.
//!
//! Everything here runs against a real SQLite file in a temporary directory,
//! and the last two tests dial a real `ServerRuntime` over a loopback endpoint.
//! That distinction is the point: a test against fakes proves the protocol, and
//! only a test that dials proves the binary.

mod assets;
mod blocks;
mod conversations;
mod resume;

use std::sync::Arc;
use std::time::Duration;

use iroh::endpoint::{Connection, RecvStream, SendStream};
use iroh::EndpointAddr;
use sea_orm::DatabaseConnection;
use tethera_common::protocol::capability::{self, CapabilityId, CapabilitySet, HasCapability};
use tethera_common::protocol::handshake::{
    ClientHello, ClientInfo, EnrollCode, EnrollResult, Handshake, Intent, Platform, RefuseReason,
    ServerHello,
};
use tethera_common::protocol::request::Request;
use tethera_common::protocol::response::{Payload, Response};
use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::WireVersion;
use tethera_common::structs::asset::AssetScope;
use tethera_common::structs::conversation::ConversationFilter;
use tethera_common::structs::device::DeviceState;
use tethera_common::structs::ids::{ConversationId, RequestId, ServerId};
use tethera_server_lib::config::ApplicationConfig;
use tethera_server_lib::machine::{Installed, MachineAddress, Offer};
use tethera_server_lib::protocol::dispatch::{Dispatcher, HandshakeOutcome};
use tethera_server_lib::protocol::ports::{
    AssetPort, ConversationPort, Enrolment, MachinePort, Ports,
};
use tethera_server_lib::protocol::LivePorts;
use tethera_server_lib::runtime::ServerRuntime;
use tethera_server_lib::services::{DeviceService, PairingService, TransportService};
use tethera_server_lib::storage::Storage;
use tethera_transport::endpoint::TetheraEndpoint;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

const PHONE: &str = "cf0f2b2c9d1e4a5b8c7d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b";
const TABLET: &str = "1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f809";
const MACHINE: &str = "9f8e7d6c5b4a39281706f5e4d3c2b1a09f8e7d6c5b4a39281706f5e4d3c2b1a0";
const TTL: u64 = 300;

/// A backend name nothing can resolve, so the terminal probe fails the same way
/// on every machine. Without it this suite would advertise different
/// capabilities on a developer's box than in CI, and its own honesty test would
/// mean two different things.
const NO_BACKEND: &str = "tethera-no-such-terminal-backend";

/// A machine's own storage, in a directory that dies with the test.
struct Fixture {
    _dir: tempfile::TempDir,
    config: Arc<ApplicationConfig>,
    db: Arc<DatabaseConnection>,
}

impl Fixture {
    async fn start() -> Self {
        Self::start_capped(ApplicationConfig::DEFAULT_MAX_CONNECTIONS).await
    }

    async fn start_capped(max_connections: usize) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut config = ApplicationConfig::with_data_dir(dir.path().to_path_buf());
        config.herdr_binary = NO_BACKEND.to_string();
        config.max_connections = max_connections;

        let config = Arc::new(config);
        let db = Arc::new(Storage::connect(&config).await.expect("storage"));

        Self {
            _dir: dir,
            config,
            db,
        }
    }

    async fn ports(&self) -> Arc<LivePorts> {
        self.ports_for(MACHINE).await
    }

    /// A second call is what a reconnect is: fresh ports, same database.
    async fn ports_for(&self, endpoint_id: &str) -> Arc<LivePorts> {
        LivePorts::new_shared(self.config.clone(), self.db.clone(), endpoint_id.to_string()).await
    }

    fn pairing(&self) -> PairingService {
        PairingService::new(self.db.clone())
    }

    async fn open_window(&self) -> String {
        self.pairing()
            .open_window(TTL, Self::now())
            .await
            .expect("a window")
            .0
    }

    fn now() -> i64 {
        chrono::Utc::now().timestamp()
    }

    /// A code this window will not accept, whatever it happens to have minted.
    fn wrong_code(real: &str) -> &'static str {
        if real == "000000" {
            "111111"
        } else {
            "000000"
        }
    }
}

/// A client, dialling a real address and writing every frame by hand.
///
/// Deliberately not a reusable client library, for the same reason the
/// dispatcher suite's harness is not: a test should read as the protocol rather
/// than as a wrapper around it.
struct Dialled {
    connection: Connection,
    codec: FrameCodec,
    /// Held for the test's life. Dropping an endpoint closes everything it
    /// opened, which would end the connection mid-assertion. It is also the
    /// identity: a redial from the same endpoint is the same device.
    endpoint: TetheraEndpoint,
}

impl Dialled {
    async fn to(addr: EndpointAddr) -> Self {
        Self::try_to(addr).await.expect("dial")
    }

    /// A dial that reports a refusal instead of panicking on it.
    async fn try_to(addr: EndpointAddr) -> Option<Self> {
        let endpoint = TetheraEndpoint::bind_local().await.expect("client endpoint");
        let connection = endpoint.connect(addr).await.ok()?;

        Some(Self {
            connection,
            codec: FrameCodec::default(),
            endpoint,
        })
    }

    /// Hangs up, the way a client that is done does, and waits for the close to
    /// actually leave.
    ///
    /// Closing queues a frame; the endpoint's driver sends it. Dropping the
    /// endpoint straight after `close` therefore delivers nothing, and the far
    /// side learns only at the QUIC idle timeout. That is also true of a peer
    /// that simply vanishes, which is the practical limit of any connection
    /// bound: a slot is held until the timeout, not until the peer is gone.
    async fn close_and_wait(&self) {
        self.connection.close(0u32.into(), b"done");
        self.connection.closed().await;
    }

    /// A second connection from the same endpoint id, which is what a client
    /// reconnecting looks like to the server.
    async fn redial(&mut self, addr: EndpointAddr) {
        self.connection = self.endpoint.connect(addr).await.expect("redial");
    }

    /// A hello that reports a refused connection instead of panicking on it.
    async fn try_hello(&self, intent: Intent) -> Option<ServerHello> {
        let (mut send, mut recv) = self.connection.open_bi().await.ok()?;

        let hello = StreamOpen::Hello(ClientHello {
            versions: WireVersion::SUPPORTED.to_vec(),
            client: ClientInfo {
                app_version: "0.1.0".into(),
                platform: Platform::Ios,
                install_id: "3f9a2c".into(),
            },
            intent,
        });

        FrameIo::write(&mut send, &self.codec, &hello).await.ok()?;

        FrameIo::read(&mut recv, &self.codec).await.ok()?
    }

    async fn hello(&self, intent: Intent) -> (ServerHello, SendStream, RecvStream) {
        let (mut send, mut recv) = self.connection.open_bi().await.expect("open");

        let hello = StreamOpen::Hello(ClientHello {
            versions: WireVersion::SUPPORTED.to_vec(),
            client: ClientInfo {
                app_version: "0.1.0".into(),
                platform: Platform::Ios,
                install_id: "3f9a2c".into(),
            },
            intent,
        });

        FrameIo::write(&mut send, &self.codec, &hello)
            .await
            .expect("write hello");

        let answer: ServerHello = FrameIo::read(&mut recv, &self.codec)
            .await
            .expect("read")
            .expect("a server hello");

        (answer, send, recv)
    }

    async fn type_code(
        &self,
        send: &mut SendStream,
        recv: &mut RecvStream,
        code: &str,
    ) -> EnrollResult {
        let typed = EnrollCode {
            request_id: RequestId("req".into()),
            code: code.to_string(),
            device_name: "phone".to_string(),
        };

        FrameIo::write(send, &self.codec, &typed)
            .await
            .expect("write code");

        FrameIo::read(recv, &self.codec)
            .await
            .expect("read")
            .expect("an enrol result")
    }

    async fn rpc(&self, request: Request) -> Response {
        let (mut send, mut recv) = self.connection.open_bi().await.expect("open");

        FrameIo::write(&mut send, &self.codec, &StreamOpen::Rpc(request))
            .await
            .expect("write request");
        send.finish().ok();

        loop {
            let frame: Response = FrameIo::read(&mut recv, &self.codec)
                .await
                .expect("read")
                .expect("a response");

            if frame.is_terminal() {
                return frame;
            }
        }
    }
}

/// A bound endpoint, a running accept loop, and the address to dial it on.
struct RunningServer {
    addr: EndpointAddr,
    runtime: tokio::task::JoinHandle<anyhow::Result<()>>,
}

// Ctrl-C is the only thing that ends the accept loop, so a test cannot ask it to
// stop. Dropping the handle does not stop the task either, and a loop still
// running after the test returns keeps writing `endpoint.json` into a temporary
// directory that is being deleted.
impl Drop for RunningServer {
    fn drop(&mut self) {
        self.runtime.abort();
    }
}

impl RunningServer {
    async fn start(fixture: &Fixture) -> Self {
        let endpoint = TetheraEndpoint::bind_local().await.expect("server endpoint");
        let addr = endpoint.loopback_addr().expect("loopback address");
        let transport = TransportService::new(endpoint);
        let transport = Arc::new(transport);
        let ports = fixture.ports_for(&transport.id().to_string()).await;
        let runtime = ServerRuntime::new(fixture.config.clone());

        let handle = tokio::spawn(async move { runtime.serve(transport, ports).await });

        Self {
            addr,
            runtime: handle,
        }
    }
}

// What stops a stranger is a window a human opened. Without one, an unknown
// endpoint id is refused before anything is displayed on the machine.
#[tokio::test]
async fn an_unknown_endpoint_with_no_window_open_is_refused_as_pairing_window_closed() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;

    assert!(
        ports.machine().pairing_window().await.is_none(),
        "a window was open before anybody opened one"
    );

    let outcome = Dispatcher::<LivePorts>::decide(
        &ports.machine().enrolment(PHONE).await,
        ports.machine().pairing_window().await,
        Intent::Enroll,
        WireVersion::SUPPORTED,
    );

    assert_eq!(
        outcome,
        HandshakeOutcome::Refuse(RefuseReason::PairingWindowClosed)
    );
}

#[tokio::test]
async fn opening_a_window_offers_enrolment_with_the_length_a_client_must_draw() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;
    let code = fixture.open_window().await;

    let offer = ports.machine().pairing_window().await.expect("an offer");

    assert_eq!(usize::from(offer.code_length), code.len());
    assert!(offer.expires_in_ms > 0, "the window expired as it opened");
    assert_eq!(offer.server.id, ports.machine().server_info().id);
}

// The counter is on the row, not in the connection. A client that redials and
// finds its attempts restored has an unbounded number of guesses at six digits.
#[tokio::test]
async fn a_wrong_code_spends_one_attempt_and_the_count_survives_a_reconnect() {
    let fixture = Fixture::start().await;
    let code = fixture.open_window().await;
    let wrong = Fixture::wrong_code(&code);

    let first = fixture
        .ports()
        .await
        .machine()
        .redeem_code(PHONE, wrong, "phone")
        .await;

    assert_eq!(first.err(), Some(PairingService::DEFAULT_ATTEMPTS as u8 - 1));

    // A redial is a fresh port set over the same database.
    let second = fixture
        .ports()
        .await
        .machine()
        .redeem_code(PHONE, wrong, "phone")
        .await;

    assert_eq!(second.err(), Some(PairingService::DEFAULT_ATTEMPTS as u8 - 2));
}

#[tokio::test]
async fn a_window_whose_attempts_are_spent_is_no_longer_open() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;
    let code = fixture.open_window().await;
    let wrong = Fixture::wrong_code(&code);

    let mut last = None;

    for _ in 0..PairingService::DEFAULT_ATTEMPTS {
        last = ports.machine().redeem_code(PHONE, wrong, "phone").await.err();
    }

    assert_eq!(last, Some(0));
    assert!(
        ports.machine().pairing_window().await.is_none(),
        "a spent window stayed open"
    );

    // And the right code no longer helps, because the window is gone.
    assert_eq!(
        ports.machine().redeem_code(PHONE, &code, "phone").await.err(),
        Some(0)
    );
}

#[tokio::test]
async fn the_right_code_enrols_the_device_and_the_next_connection_is_a_session() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;
    let code = fixture.open_window().await;

    let record = ports
        .machine()
        .redeem_code(PHONE, &Handshake::normalize_code(&code), "charl's phone")
        .await
        .expect("enrolled");

    assert_eq!(record.name, "charl's phone");

    match ports.machine().enrolment(PHONE).await {
        Enrolment::Known(known) => assert_eq!(known.id, record.id),
        other => panic!("expected a known device, got {other:?}"),
    }

    let outcome = Dispatcher::<LivePorts>::decide(
        &ports.machine().enrolment(PHONE).await,
        None,
        Intent::Session,
        WireVersion::SUPPORTED,
    );

    assert!(matches!(outcome, HandshakeOutcome::Session { .. }));
}

// Single use is the whole of what stops one code enrolling a fleet.
#[tokio::test]
async fn a_redeemed_code_cannot_be_redeemed_by_a_second_endpoint() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;
    let code = fixture.open_window().await;

    ports
        .machine()
        .redeem_code(PHONE, &code, "phone")
        .await
        .expect("enrolled");

    assert!(ports.machine().redeem_code(TABLET, &code, "tablet").await.is_err());
    assert_eq!(ports.machine().enrolment(TABLET).await, Enrolment::Unknown);
}

// One code enrols one device, whatever the interleaving.
//
// Multi-threaded and through two port sets, so both redemptions reach the
// database rather than queueing on one process-local mutex. Worth stating what
// this does and does not prove: it was run with both the conditional consume and
// the mutex removed and it still passed, because SQLite refuses the second
// writer on its own. So this guards the outcome, not the mechanism. The
// mechanism is here anyway, because it is what makes single use a property of
// the statement rather than of which database is underneath, and it turns the
// loser's "database is locked" into a clean refusal.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_endpoints_racing_the_same_code_enrol_exactly_one() {
    let fixture = Fixture::start().await;
    let first_ports = fixture.ports().await;
    let second_ports = fixture.ports().await;
    let code = fixture.open_window().await;

    let (first, second) = tokio::join!(
        first_ports.machine().redeem_code(PHONE, &code, "phone"),
        second_ports.machine().redeem_code(TABLET, &code, "tablet"),
    );

    assert_eq!(
        usize::from(first.is_ok()) + usize::from(second.is_ok()),
        1,
        "one code enrolled {} devices",
        usize::from(first.is_ok()) + usize::from(second.is_ok())
    );
}

// Revoked is distinct from unknown. A revoked device that read as a stranger
// could re-enrol through the next open window, which would make revocation
// cosmetic.
#[tokio::test]
async fn a_revoked_device_reads_as_revoked_rather_than_as_a_stranger() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;
    let code = fixture.open_window().await;

    let record = ports
        .machine()
        .redeem_code(PHONE, &code, "phone")
        .await
        .expect("enrolled");

    // The device id, which is what `Rpc` hands to this port for
    // `RevokeThisDevice`, rather than the endpoint id the signature names.
    ports
        .machine()
        .revoke(record.id.as_str())
        .await
        .expect("revoked");

    assert_eq!(ports.machine().enrolment(PHONE).await, Enrolment::Revoked);

    let outcome = Dispatcher::<LivePorts>::decide(
        &ports.machine().enrolment(PHONE).await,
        ports.machine().pairing_window().await,
        Intent::Enroll,
        WireVersion::SUPPORTED,
    );

    assert_eq!(outcome, HandshakeOutcome::Refuse(RefuseReason::Revoked));
}

// Opening a window closes any earlier one. Without that, every code ever minted
// stays live for its whole TTL and the guess budget is per code rather than per
// window.
#[tokio::test]
async fn a_superseded_code_no_longer_redeems() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;

    let first = fixture.open_window().await;
    let second = fixture.open_window().await;

    assert_ne!(first, second, "two windows minted the same code");

    // The superseded code is now simply a wrong code, so it is judged against
    // the surviving window and spends one of its attempts. Worth stating
    // plainly: replaying an old code drains the current window, and the missing
    // half of that is a per-endpoint sub-budget rather than a per-window one.
    assert_eq!(
        ports.machine().redeem_code(PHONE, &first, "phone").await.err(),
        Some(PairingService::narrow(PairingService::DEFAULT_ATTEMPTS) - 1)
    );

    ports
        .machine()
        .redeem_code(PHONE, &second, "phone")
        .await
        .expect("the newest code still enrols");
}

// `Offer::build` is what stops a dead server's addresses reaching a QR, and what
// stops one machine's record being read as another's.
#[tokio::test]
async fn an_offer_carries_addresses_only_from_a_live_record_for_this_machine() {
    let fixture = Fixture::start().await;
    let now = Fixture::now();
    let address = "10.0.0.4:41000";

    MachineAddress::new(
        MACHINE.to_string(),
        vec![address.to_string()],
        None,
        now,
    )
    .publish(&fixture.config)
    .expect("published");

    let fresh = Offer::build(&fixture.config, MACHINE, now);
    assert_eq!(fresh.direct_addrs, vec![address.to_string()]);

    // Older than the staleness bound: the server that wrote it is not running,
    // so nothing is listening on those addresses.
    let stale = Offer::build(
        &fixture.config,
        MACHINE,
        now + MachineAddress::STALE_AFTER_SECONDS + 1,
    );
    assert!(stale.direct_addrs.is_empty());

    // A record belonging to a different endpoint id is not this machine's.
    let other = Offer::build(&fixture.config, PHONE, now);
    assert!(other.direct_addrs.is_empty());
}

#[tokio::test]
async fn an_expired_window_is_not_open() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;

    fixture
        .pairing()
        .open_window(TTL, Fixture::now() - (TTL as i64 * 2))
        .await
        .expect("a window");

    assert!(ports.machine().pairing_window().await.is_none());
}

// The freshness of a status mark is exactly how often the tree is re-read, so
// the interval that governs it is short. What makes a short interval affordable
// is that it does no work at all with nobody watching - a machine spends most of
// its life in that state, and a timer that shelled out to a terminal backend
// every few seconds regardless would run forever for nobody.
#[tokio::test]
async fn a_tree_is_not_re_read_while_nothing_is_watching_it() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;

    assert!(
        !ports.refresh_watched_tree().await,
        "the tree was read with nobody watching"
    );
}

// And the case it exists for. One read serves every watcher, where a client-side
// poll costs a full listing per client per tick.
#[tokio::test]
async fn an_open_watch_is_what_makes_the_tree_refresh() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;

    let _watching = ports.machine().tree_events();

    assert!(
        ports.refresh_watched_tree().await,
        "a watch was open and the tree was never refreshed for it"
    );
}

// An absent capability renders as nothing at all in the client. An advertised
// one that refuses renders as a control a person taps and watches fail.
#[tokio::test]
async fn the_machine_advertises_only_what_its_ports_can_answer() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;
    let describe = ports.machine().describe().await;

    // Exactly these, not "at least". The fixture points the terminal backend at
    // a name nothing resolves, so the probe fails the same way everywhere and
    // this set does not depend on what is installed on the machine running it.
    let expected: CapabilitySet = [
        capability::AGENT_CATALOG,
        capability::DEVICE_SELF_REVOKE,
        // Read off the sessions on disk, which a machine with no terminal
        // backend still has.
        capability::RECENT_CWDS,
        // Reading and paging an agent's own records works, and so does saying
        // what a start would create. Nothing that types into a pane does, so
        // `questions`, `conversation_start`, `prompt_send` and `interrupt`
        // stay out.
        capability::TRANSCRIPT_PAGING,
        capability::CONVERSATION_PREVIEW,
        // Both halves answer, and neither needs a terminal backend: a file an
        // agent handed over is read from the records, and one a phone sends
        // lands in this machine's own upload directory.
        capability::ASSETS_READ,
        capability::ASSETS_WRITE,
    ]
    .into_iter()
    .map(CapabilityId::from)
    .collect();

    assert_eq!(describe.capabilities, expected);

    // Not "not empty": the catalog reports the harnesses installed here, and a
    // machine with none is a real state a client can render. What every row must
    // satisfy is that this machine could really launch it, and that it carries
    // the modes `agent_modes` promises.
    for profile in ports.machine().agent_profiles().await {
        assert!(
            Installed::has(profile.id.as_str()),
            "the catalog lists {} and this machine cannot run it",
            profile.id.as_str()
        );
    }

    assert!(ports
        .conversations()
        .transcript(&ConversationId::mint("absent"), None, 10)
        .await
        .is_err());

    // A conversation with no records has no files, which is an empty listing
    // rather than a failure. What must not happen is a card for a file this
    // machine cannot serve.
    for card in ports
        .assets()
        .list(&AssetScope::Conversation(ConversationId::mint("absent")), None, 10)
        .await
        .map(|page| page.items)
        .unwrap_or_default()
    {
        assert!(
            ports.assets().fetch(&card.asset, 0).await.is_ok(),
            "listed {:?} and cannot serve it",
            card.name
        );
    }

    // Advertised, so what it answers has to be usable. Every suggestion is a
    // directory `start` would accept, or it is a choice the next call rejects.
    // The list is read from this machine's real home, so its contents are not
    // assertable here; the contract every entry satisfies is.
    for suggested in ports.machine().recent_cwds(10).await {
        assert!(
            std::path::Path::new(&suggested).is_dir(),
            "suggested a working directory that is not one: {suggested}"
        );
    }

    // A machine with no terminal backend has no workspaces, and says so rather
    // than failing. A machine watch's first frame is mandatory, and `watch.rs`
    // closes the stream without one if this errors.
    let tree = ports.machine().tree().await.expect("a tree");
    assert!(tree.workspaces.is_empty());
    assert!(tree.tabs.is_empty());
    assert!(tree.panes.is_empty());

    // `list` reads whatever this machine has under its own home directory, so
    // what it returns is not assertable here. What is assertable is the contract
    // every row must satisfy - and an empty machine is covered where the home
    // directory can be controlled, in `protocol::live::conversations`.
    let listed = ports
        .conversations()
        .list(ConversationFilter::All, None, 10)
        .await;

    assert!(listed.items.len() <= 10);

    for conversation in &listed.items {
        assert!(conversation.id.as_str().starts_with(ConversationId::PREFIX));
        assert!(
            conversation.has_transcript,
            "a listed conversation whose records cannot be read would open onto nothing"
        );
    }

    // Nothing that needs to drive a pane is advertised, and each refuses.
    assert!(ports
        .conversations()
        .answer(
            &ConversationId::mint("absent"),
            &tethera_common::structs::ids::QuestionId::mint("q"),
            &tethera_common::structs::primitives::Fingerprint("x".into()),
            &[tethera_common::structs::transcript::Answer::Choice(0)],
        )
        .await
        .is_err());

    assert!(ports
        .conversations()
        .send_prompt(&ConversationId::mint("absent"), "hello", &[])
        .await
        .is_err());
}

// A client compares the id it scanned against the id TLS proved, and refuses
// the pairing when they differ. With `server_id` set to an endpoint id that
// comparison could never pass.
#[tokio::test]
async fn the_pairing_offer_names_the_same_machine_as_the_handshake() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;
    let offer = Offer::build(&fixture.config, MACHINE, Fixture::now());

    assert!(offer.server_id.starts_with(ServerId::PREFIX));
    assert_eq!(
        ServerId::parse(&offer.server_id).expect("a server id"),
        ports.machine().server_info().id
    );
    assert_eq!(offer.endpoint_id.as_deref(), Some(MACHINE));
}

// The state machine's own invariant, through the service that writes the row.
// A revoked device that could be returned to Active by a fresh code would make
// revocation cosmetic.
#[tokio::test]
async fn a_revoked_device_cannot_be_returned_to_active_by_a_new_code() {
    let fixture = Fixture::start().await;
    let devices = DeviceService::new(fixture.db.clone());
    let now = Fixture::now();

    devices
        .activate(&*fixture.db, PHONE, "phone", now)
        .await
        .expect("enrolled");
    devices
        .set_state(&*fixture.db, PHONE, DeviceState::Revoked, now)
        .await
        .expect("revoked");

    assert!(
        devices.activate(&*fixture.db, PHONE, "phone", now).await.is_err(),
        "a revoked device was re-enrolled"
    );
}

// A retry after a dropped acknowledgement is the ordinary case, and
// `can_transition_to` has no self-pair, so this would otherwise tell a device
// that revoked itself twice that it does not exist.
#[tokio::test]
async fn revoking_a_device_twice_is_not_an_error() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;
    let code = fixture.open_window().await;

    let record = ports
        .machine()
        .redeem_code(PHONE, &code, "phone")
        .await
        .expect("enrolled");

    ports.machine().revoke(record.id.as_str()).await.expect("revoked");
    ports
        .machine()
        .revoke(record.id.as_str())
        .await
        .expect("revoking twice is the same request, not a failure");
}

// `Device.id` is the database row id and `DeviceRecord.id` is `dv_<endpoint>`.
// Reading one as the other would look up a device whose endpoint id is "1".
#[tokio::test]
async fn revoking_a_value_that_is_not_a_device_id_is_refused_rather_than_guessed() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;

    assert_eq!(
        ports.machine().revoke("1").await,
        Err(WireError::NotFound {
            kind: EntityKind::Device
        })
    );
}

// The offer is computed from the row, not from the ttl it was minted with, so a
// client dialling four minutes into a five-minute window is told sixty seconds.
#[tokio::test]
async fn a_window_reports_the_time_it_has_left_rather_than_the_time_it_was_given() {
    let fixture = Fixture::start().await;
    let ports = fixture.ports().await;
    let elapsed = 240;

    fixture
        .pairing()
        .open_window(TTL, Fixture::now() - elapsed)
        .await
        .expect("a window");

    let offer = ports.machine().pairing_window().await.expect("an offer");
    let remaining_ms = (TTL as i64 - elapsed) * 1_000;

    assert!(
        offer.expires_in_ms as i64 <= remaining_ms
            && offer.expires_in_ms as i64 > remaining_ms - 5_000,
        "expected about {remaining_ms} ms left, got {}",
        offer.expires_in_ms
    );
}

/// Two seconds of polling for a released slot. The release is a task finishing,
/// not an event this side can await.
const SLOT_RETURN_POLLS: usize = 20;

// The bound is a memory bound, and a permit that leaked would turn a working
// server into one that refuses everything after its lifetime quota.
#[tokio::test]
async fn a_connection_past_the_limit_is_refused_and_its_slot_returns() {
    let fixture = Fixture::start_capped(2).await;
    fixture.open_window().await;

    let server = RunningServer::start(&fixture).await;

    let first = Dialled::to(server.addr.clone()).await;
    let second = Dialled::to(server.addr.clone()).await;

    assert!(first.try_hello(Intent::Enroll).await.is_some());
    assert!(second.try_hello(Intent::Enroll).await.is_some());

    let third = Dialled::to(server.addr.clone()).await;
    assert!(
        third.try_hello(Intent::Enroll).await.is_none(),
        "a third connection was served past a limit of two"
    );

    // Hanging one up hands its slot back once the server's task for it
    // finishes. Closed rather than merely dropped: a peer that vanishes holds
    // its slot until the QUIC idle timeout, which is a real property of the
    // bound and not one worth spending twenty seconds of the suite proving.
    //
    // Redialled rather than retried on the refused connection: that one is
    // closed, and asking it again can never succeed.
    second.close_and_wait().await;

    let mut served = false;

    for _ in 0..SLOT_RETURN_POLLS {
        tokio::time::sleep(Duration::from_millis(100)).await;

        if let Some(candidate) = Dialled::try_to(server.addr.clone()).await {
            if candidate.try_hello(Intent::Enroll).await.is_some() {
                served = true;
                break;
            }
        }
    }

    assert!(served, "the released slot was never handed back");
}

// The binary, not the dispatcher: a real endpoint, the real accept loop, and a
// peer that dials it.
#[tokio::test]
async fn a_dialled_server_completes_an_enrolment_and_answers_a_request() {
    let fixture = Fixture::start().await;
    let code = fixture.open_window().await;
    let server = RunningServer::start(&fixture).await;
    let client = Dialled::to(server.addr.clone()).await;

    let (answer, mut send, mut recv) = client.hello(Intent::Enroll).await;

    match answer {
        ServerHello::EnrollPending { code_length, .. } => {
            assert_eq!(usize::from(code_length), code.len())
        }
        other => panic!("expected an enrolment offer, got {other:?}"),
    }

    match client.type_code(&mut send, &mut recv, &code).await {
        EnrollResult::Accepted { capabilities, .. } => {
            assert!(capabilities.has(capability::AGENT_CATALOG))
        }
        other => panic!("expected an acceptance, got {other:?}"),
    }

    match client.rpc(Request::Describe).await {
        Response::Ok(Payload::Describe(describe)) => {
            assert!(!describe.server.label.is_empty());
            assert_eq!(describe.limits.transcript_page, 200);
        }
        other => panic!("expected a describe, got {other:?}"),
    }
}

// `device_self_revoke` is one of three advertised capabilities, and this is the
// only path that proves it end to end: `Rpc` hands the port a `DeviceId` where
// the signature documents an endpoint id, and the next connection has to be
// refused for the right reason.
#[tokio::test]
async fn revoking_this_device_over_the_wire_refuses_its_next_connection() {
    let fixture = Fixture::start().await;
    let code = fixture.open_window().await;
    let server = RunningServer::start(&fixture).await;
    let mut client = Dialled::to(server.addr.clone()).await;

    let (_answer, mut send, mut recv) = client.hello(Intent::Enroll).await;

    assert!(matches!(
        client.type_code(&mut send, &mut recv, &code).await,
        EnrollResult::Accepted { .. }
    ));

    assert!(matches!(
        client.rpc(Request::RevokeThisDevice).await,
        Response::Ok(Payload::Ack)
    ));

    client.redial(server.addr.clone()).await;
    let (answer, _send, _recv) = client.hello(Intent::Session).await;

    assert_eq!(answer, ServerHello::Refuse(RefuseReason::Revoked));
}

// Serving inline would park the accept loop behind the first peer, and a phone
// that connects and opens nothing would lock every other device out.
#[tokio::test]
async fn a_second_connection_is_served_while_the_first_is_still_open() {
    let fixture = Fixture::start().await;
    fixture.open_window().await;

    let server = RunningServer::start(&fixture).await;

    let first = Dialled::to(server.addr.clone()).await;
    let (_answer, _send, _recv) = first.hello(Intent::Enroll).await;

    let second = Dialled::to(server.addr.clone()).await;

    // Bounded, because the regression this guards against is the accept loop
    // parking inside the first peer's enrolment loop. That fails by hanging,
    // and a suite that hangs is the hardest kind of failure to read in CI.
    let answer = tokio::time::timeout(
        Duration::from_secs(10),
        second.hello(Intent::Enroll),
    )
    .await
    .expect("the accept loop is parked behind the first connection")
    .0;

    assert!(
        matches!(answer, ServerHello::EnrollPending { .. }),
        "the second connection was not served while the first was open"
    );
}
