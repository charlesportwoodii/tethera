//! One fixture per shape that actually crosses the wire.
//!
//! Ten composites rather than one per leaf type: a fixture per leaf would pin
//! encodings nothing sends while still missing a reordering *inside* a
//! composite, which is the failure that matters.

use tethera_common::protocol::handshake::{ClientHello, ClientInfo, Intent, Platform};
use tethera_common::protocol::request::Request;
use tethera_common::protocol::response::{Payload, Response};
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::terminal::{
    attrs, Color, RowUpdate, Span, Style, TerminalFrame,
};
use tethera_common::protocol::transfer::FetchHead;
use tethera_common::protocol::watch::WatchOpen;
use tethera_common::protocol::WireVersion;
use tethera_common::structs::agent::AgentStatus;
use tethera_common::structs::conversation::Conversation;
use tethera_common::structs::ids::{
    ConversationId, PaneId, ProfileId, ServerId, TabId, TurnId, WorkspaceId,
};
use tethera_common::structs::primitives::{Cursor, Sha256, Timestamp};
use tethera_common::structs::terminal::{Pane, PaneRect, PaneSlot, Size, TabLayout};
use tethera_common::structs::transcript::{Part, Role, ToolStatus, Turn};

use crate::golden::assert_golden;

const V4: WireVersion = WireVersion(4);

fn a_pane() -> Pane {
    Pane {
        id: PaneId::parse("pn_a1").expect("valid"),
        tab_id: TabId::parse("tb_b2").expect("valid"),
        workspace_id: WorkspaceId::parse("ws_c3").expect("valid"),
        label: "zsh".into(),
        title: None,
        cwd: None,
        size: Size {
            cols: 200,
            rows: 50,
        },
        focused: false,
        foreground_command: None,
        conversation: None,
        agent: None,
        streamed: false,
    }
}

fn an_unbound_conversation() -> Conversation {
    Conversation {
        id: ConversationId::parse("cv_9f21").expect("valid"),
        profile: ProfileId("claude".into()),
        profile_label: "Claude Code".into(),
        title: Some("protocol design".into()),
        preview: None,
        cwd: "/home/charl/tethera".into(),
        workspace: None,
        started_at: Timestamp(1_766_000_000_000),
        last_active: Some(Timestamp(1_766_000_900_000)),
        turn_count: Some(42),
        status: AgentStatus::Idle,
        has_transcript: true,
        resumable: true,
        binding: None,
    }
}

#[test]
fn a_client_hello_encodes_the_same_bytes_it_always_has() {
    let hello = StreamOpen::Hello(ClientHello {
        versions: vec![WireVersion(1)],
        client: ClientInfo {
            app_version: "0.1.0".into(),
            platform: Platform::Ios,
            install_id: "3f9a2c".into(),
        },
        intent: Intent::Session,
    });

    assert_golden(V4, "client_hello", &hello);
}

#[test]
fn a_transcript_request_encodes_the_same_bytes_it_always_has() {
    let request = Request::Transcript {
        conversation: ConversationId::parse("cv_9f21").expect("valid"),
        before: Some(Cursor("t1:8814".into())),
        limit: 30,
    };

    assert_golden(V4, "request_transcript", &request);
}

#[test]
fn a_pane_response_encodes_the_same_bytes_it_always_has() {
    assert_golden(V4, "response_pane", &Response::Ok(Payload::Pane(a_pane())));
}

#[test]
fn a_text_turn_encodes_the_same_bytes_it_always_has() {
    let turn = Turn {
        cursor: Cursor("t1:8814".into()),
        id: TurnId("rec-9f21".into()),
        at: Timestamp(1_766_000_000_000),
        role: Role::Agent,
        parts: vec![Part::Text {
            text: "hello".into(),
        }],
    };

    assert_golden(V4, "turn_text", &turn);
}

#[test]
fn a_tool_use_turn_encodes_the_same_bytes_it_always_has() {
    let turn = Turn {
        cursor: Cursor("t1:8815".into()),
        id: TurnId("rec-9f22".into()),
        at: Timestamp(1_766_000_000_001),
        role: Role::Agent,
        parts: vec![Part::ToolUse {
            name: "Bash".into(),
            input: "grep -rn tethera".into(),
            result: Some("2 hits".into()),
            status: ToolStatus::Ok,
            fallback_text: "ran Bash".into(),
        }],
    };

    assert_golden(V4, "turn_tool_use", &turn);
}

#[test]
fn an_unbound_conversation_encodes_the_same_bytes_it_always_has() {
    assert_golden(V4, "conversation_unbound", &an_unbound_conversation());
}

#[test]
fn a_terminal_snapshot_encodes_the_same_bytes_it_always_has() {
    let frame = TerminalFrame::Snapshot {
        cols: 80,
        rows: 2,
        styles: vec![Style {
            fg: Color::Default,
            bg: Color::Default,
            attrs: attrs::NONE,
        }],
        rows_data: vec![RowUpdate {
            y: 0,
            from_x: 0,
            spans: vec![Span {
                style: 0,
                text: "hello".into(),
            }],
        }],
        cursor: None,
        alt_screen: false,
        scrollback_len: Some(1200),
    };

    assert_golden(V4, "terminal_snapshot", &frame);
}

#[test]
fn a_machine_watch_open_encodes_the_same_bytes_it_always_has() {
    let open = WatchOpen::Machine {
        workspaces: Vec::new(),
        tabs: Vec::new(),
        panes: vec![a_pane()],
        conversations: vec![an_unbound_conversation()],
        layouts: Vec::new(),
    };

    assert_golden(V4, "watch_open_machine", &open);
}

#[test]
fn a_layout_payload_encodes_the_same_bytes_it_always_has() {
    let layout = TabLayout {
        tab: TabId::parse("tb_b2").expect("valid"),
        slots: vec![
            PaneSlot {
                pane: PaneId::parse("pn_a1").expect("valid"),
                rect: PaneRect::new(0, 0, 100, 50),
            },
            PaneSlot {
                pane: PaneId::parse("pn_a2").expect("valid"),
                rect: PaneRect::new(100, 0, 100, 50),
            },
        ],
        zoomed: None,
    };

    assert_golden(V4, "response_layout", &Response::Ok(Payload::Layout(layout)));
}

#[test]
fn a_fetch_head_encodes_the_same_bytes_it_always_has() {
    let head = FetchHead {
        len: 1_048_576,
        mime: Some("text/markdown".into()),
        sha256: Sha256("e3b0c44298fc1c14".into()),
        offset: 4096,
    };

    assert_golden(V4, "fetch_head", &head);
}

#[test]
fn a_server_id_still_encodes_as_its_whole_prefixed_string() {
    let id = ServerId::parse("sv_a1").expect("valid");

    assert_golden(V4, "server_id", &id);
}
