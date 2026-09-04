use std::time::Duration;
use tethera_client_core::endpoint::ClientEndpoint;
use tethera_client_core::sweep::{Sweep, SweepBudget};
use tethera_common::protocol::capability::CapabilitySet;
use tethera_common::protocol::handshake::{
    ClientInfo, DeviceRecord, Platform, RefuseReason, ServerInfo,
};
use tethera_common::structs::client::ServerEntry;
use tethera_common::structs::conversation::ConversationFilter;
use tethera_common::structs::ids::{DeviceId, ServerId};
use tethera_common::structs::link::LinkKind;
use tethera_common::structs::primitives::Timestamp;

use super::fake::{Answer, FakeMachine};

fn a_client() -> ClientInfo {
    ClientInfo {
        app_version: "0.1.0".to_string(),
        platform: Platform::Ios,
        install_id: "3f9a2c".to_string(),
    }
}

fn a_device() -> DeviceRecord {
    DeviceRecord {
        id: DeviceId::parse("dv_phone").expect("valid"),
        name: "phone".to_string(),
        paired_at: Timestamp(1),
    }
}

fn an_entry_for(machine: &FakeMachine) -> ServerEntry {
    ServerEntry {
        server: machine.server_info(),
        endpoint_id: machine.endpoint_id(),
        relay: None,
        direct_addrs: machine.direct_addrs(),
        device: a_device(),
        capabilities: CapabilitySet::new(),
        last_seen_at: None,
        conversations: Vec::new(),
    }
}

// A well-formed endpoint id nothing is listening on, and a direct address that
// refuses immediately. This is a machine that is switched off, not a malformed
// entry.
fn an_entry_for_nothing() -> ServerEntry {
    ServerEntry {
        server: ServerInfo {
            id: ServerId::parse("sv_keel").expect("valid"),
            label: "keel".to_string(),
            app_version: "0.1.0".to_string(),
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
        },
        endpoint_id: "555bfc3855cdecba82293424903294304243b330c42e32c7e5356e8ad2e690bb"
            .to_string(),
        relay: None,
        direct_addrs: vec!["127.0.0.1:1".to_string()],
        device: a_device(),
        capabilities: CapabilitySet::new(),
        last_seen_at: None,
        conversations: Vec::new(),
    }
}

async fn an_endpoint() -> ClientEndpoint {
    ClientEndpoint::bind_local().await.expect("bind")
}

// A short dial deadline keeps the offline case fast. The production budget
// allows ten seconds for a cold relay-assisted connect; a suite that waited that
// long for one switched-off machine is a suite people stop running.
fn a_budget() -> SweepBudget {
    SweepBudget::new()
        .with_settle(Duration::from_millis(200))
        .with_dial(Duration::from_millis(600))
        .with_total(Duration::from_secs(5))
}

#[tokio::test]
async fn a_machine_that_answers_gets_a_measured_link_and_a_last_seen() {
    let machine = FakeMachine::start(Answer::Session).await;
    let endpoint = an_endpoint().await;

    let rows = Sweep::run(&endpoint, vec![an_entry_for(&machine)], a_client(), a_budget()).await;

    assert_eq!(rows.len(), 1);
    assert_ne!(rows[0].link.kind, LinkKind::Offline);
    assert!(rows[0].entry.last_seen_at.is_some());
    assert!(rows[0].refusal.is_none());
}

// One dead machine must not take the sweep down with it, or a single stale
// entry would blank the whole list.
#[tokio::test]
async fn a_machine_that_does_not_answer_becomes_offline_without_failing_the_sweep() {
    let machine = FakeMachine::start(Answer::Session).await;
    let endpoint = an_endpoint().await;

    let rows = Sweep::run(
        &endpoint,
        vec![an_entry_for_nothing(), an_entry_for(&machine)],
        a_client(),
        a_budget(),
    )
    .await;

    assert_eq!(rows.len(), 2, "rows keep the order they went in");
    assert_eq!(rows[0].link.kind, LinkKind::Offline);
    assert_eq!(rows[0].entry.server.label, "keel");
    assert_ne!(rows[1].link.kind, LinkKind::Offline);
}

// A machine that answers and turns this device away is up. Reporting "no route"
// would send a person to debug a network that is working.
#[tokio::test]
async fn a_refusal_is_recorded_as_a_refusal_rather_than_as_no_route() {
    let machine = FakeMachine::start(Answer::Refuse(RefuseReason::Revoked)).await;
    let endpoint = an_endpoint().await;

    let rows = Sweep::run(&endpoint, vec![an_entry_for(&machine)], a_client(), a_budget()).await;

    assert_eq!(rows[0].refusal, Some(RefuseReason::Revoked));
    assert_ne!(rows[0].link.kind, LinkKind::Offline);
}

#[tokio::test]
async fn an_empty_book_sweeps_to_an_empty_list() {
    let endpoint = an_endpoint().await;

    let rows = Sweep::run(&endpoint, Vec::new(), a_client(), a_budget()).await;

    assert!(rows.is_empty());
}

use tethera_common::structs::agent::AgentStatus;
use tethera_common::structs::conversation::Conversation;
use tethera_common::structs::ids::{ConversationId, PaneId, ProfileId};

fn a_conversation(id: &str, title: &str, bound: bool) -> Conversation {
    Conversation {
        id: ConversationId::parse(id).expect("a valid conversation id"),
        profile: ProfileId("claude".to_string()),
        profile_label: "claude".to_string(),
        title: Some(title.to_string()),
        preview: Some("Which route should own tethera://pair?".to_string()),
        cwd: "/home/charl/projects/tethera".to_string(),
        workspace: None,
        started_at: Timestamp(1),
        last_active: Some(Timestamp(2)),
        turn_count: Some(7),
        status: AgentStatus::Blocked,
        has_transcript: true,
        resumable: true,
        binding: bound.then(|| PaneId::parse("pn_1").expect("a valid pane id")),
    }
}

// The twigs on a server row. Without this the sweep reports a machine as
// answering and says nothing about what it is doing, which is the whole
// content of the list screen.
#[tokio::test]
async fn a_sweep_carries_back_what_the_machine_is_running() {
    let machine = FakeMachine::start_running(
        Answer::Session,
        vec![
            a_conversation("cv_one", "Pairing deep link", true),
            a_conversation("cv_two", "Flaky NAT punch test", false),
        ],
    )
    .await;
    let endpoint = an_endpoint().await;

    let rows = Sweep::run(&endpoint, vec![an_entry_for(&machine)], a_client(), a_budget()).await;

    assert_eq!(rows[0].entry.conversations.len(), 2);
    assert_eq!(
        rows[0].entry.conversations[0].title.as_deref(),
        Some("Pairing deep link")
    );
}

// What is running, rather than what is newest on disk. `All` returns the newest
// sessions the machine has recorded, so an agent blocked since Tuesday sits
// behind a week of finished work and never reaches a row at all. Every status a
// row can draw belongs to a bound conversation, which is what `Live` selects.
#[tokio::test]
async fn a_sweep_asks_for_the_conversations_that_are_running() {
    let machine = FakeMachine::start_running(
        Answer::Session,
        vec![a_conversation("cv_one", "still going", true)],
    )
    .await;
    let endpoint = an_endpoint().await;

    let _rows = Sweep::run(&endpoint, vec![an_entry_for(&machine)], a_client(), a_budget()).await;

    assert_eq!(machine.filters_asked(), vec![ConversationFilter::Live]);
}

// The list is a glance, not an index. A machine running more than a row can
// show must not push the rest of the list off the screen.
#[tokio::test]
async fn a_row_carries_no_more_conversations_than_it_can_show() {
    let many: Vec<Conversation> = (0..20)
        .map(|n| a_conversation(&format!("cv_{n}"), "work", false))
        .collect();
    let machine = FakeMachine::start_running(Answer::Session, many).await;
    let endpoint = an_endpoint().await;

    let rows = Sweep::run(&endpoint, vec![an_entry_for(&machine)], a_client(), a_budget()).await;

    assert_eq!(
        rows[0].entry.conversations.len(),
        Sweep::ROW_CONVERSATIONS as usize
    );
}

// A machine that answers and reports nothing running is telling the truth, and
// the row has to follow it. Keeping stale twigs here would show work that
// finished hours ago as though it were still going.
#[tokio::test]
async fn a_machine_with_nothing_running_clears_the_row() {
    let machine = FakeMachine::start(Answer::Session).await;
    let endpoint = an_endpoint().await;

    let mut entry = an_entry_for(&machine);
    entry.conversations = vec![a_conversation("cv_old", "finished", false)];

    let rows = Sweep::run(&endpoint, vec![entry], a_client(), a_budget()).await;

    assert!(rows[0].entry.conversations.is_empty());
}

// Losing the call is different from being told there is nothing. A machine whose
// conversation port refuses keeps the twigs it last reported, because a failed
// request is not evidence that the work stopped.
#[tokio::test]
async fn a_machine_that_will_not_list_keeps_what_was_last_seen() {
    let machine = FakeMachine::start(Answer::SessionRefusingRequests).await;
    let endpoint = an_endpoint().await;

    let mut entry = an_entry_for(&machine);
    entry.conversations = vec![a_conversation("cv_old", "remembered", false)];

    let rows = Sweep::run(&endpoint, vec![entry], a_client(), a_budget()).await;

    assert_eq!(rows[0].entry.conversations.len(), 1);
    assert_eq!(
        rows[0].entry.conversations[0].title.as_deref(),
        Some("remembered")
    );
}
