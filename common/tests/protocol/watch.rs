use tethera_common::protocol::watch::{WatchControl, WatchEvent, WatchOpen, WatchSpec};
use tethera_common::structs::agent::AgentStatus;
use tethera_common::structs::conversation::Conversation;
use tethera_common::structs::ids::{
    ConversationId, PaneId, ProfileId, QuestionId, TabId, WorkspaceId,
};
use tethera_common::structs::primitives::{Cursor, Fingerprint, Timestamp};
use tethera_common::structs::terminal::Workspace;
use tethera_common::structs::transcript::{Ask, Question, QuestionOption};

fn a_conversation() -> Conversation {
    Conversation {
        id: ConversationId::parse("cv_9f21").expect("valid"),
        profile: ProfileId("claude".into()),
        profile_label: "Claude Code".into(),
        title: None,
        preview: Some("Which route owns pair?".into()),
        cwd: "/tmp".into(),
        workspace: None,
        started_at: Timestamp(1),
        last_active: None,
        turn_count: None,
        status: AgentStatus::Blocked,
        has_transcript: true,
        resumable: true,
        binding: None,
    }
}

fn a_question() -> Question {
    Question {
        id: QuestionId::parse("qs_a1").expect("valid"),
        fingerprint: Fingerprint("9f21ab".into()),
        asks: vec![Ask {
            header: None,
            prompt: "Run this?".into(),
            options: vec![QuestionOption {
                label: "Allow".into(),
                description: None,
            }],
            multi_select: false,
            allows_free_text: false,
        }],
    }
}

#[test]
fn a_conversation_watch_resumes_from_a_cursor() {
    let spec = WatchSpec::Conversation {
        id: ConversationId::parse("cv_9f21").expect("valid"),
        after: Some(Cursor("t1:8814".into())),
    };
    let bytes = postcard::to_stdvec(&spec).expect("encode");

    assert_eq!(
        postcard::from_bytes::<WatchSpec>(&bytes).expect("decode"),
        spec
    );
}

// One frame, every rank. Sending less means a request per rank per machine
// before the first screen appears.
#[test]
fn a_machine_watch_opens_with_the_whole_tree() {
    let open = WatchOpen::Machine {
        workspaces: vec![Workspace {
            id: WorkspaceId::parse("ws_c3").expect("valid"),
            name: "tethera-3".into(),
            cwd: None,
            tab_count: 2,
            conversation: None,
        }],
        tabs: Vec::new(),
        panes: Vec::new(),
        conversations: vec![a_conversation()],
    };
    let bytes = postcard::to_stdvec(&open).expect("encode");

    assert_eq!(
        postcard::from_bytes::<WatchOpen>(&bytes).expect("decode"),
        open
    );
}

// `from` is what the stream actually starts at, and it is not always the `after`
// the client asked for. A client whose cursor predates the earliest surviving
// record learns it here and refetches the gap, rather than rendering a history
// with a hole in it that it cannot see.
#[test]
fn a_conversation_watch_says_where_it_actually_starts() {
    let open = WatchOpen::Conversation {
        conversation: a_conversation(),
        from: Cursor("t1:9000".into()),
    };
    let bytes = postcard::to_stdvec(&open).expect("encode");

    assert_eq!(
        postcard::from_bytes::<WatchOpen>(&bytes).expect("decode"),
        open
    );
}

#[test]
fn a_blocked_event_carries_the_whole_question() {
    let event = WatchEvent::Blocked {
        question: a_question(),
    };
    let bytes = postcard::to_stdvec(&event).expect("encode");

    assert_eq!(
        postcard::from_bytes::<WatchEvent>(&bytes).expect("decode"),
        event
    );
}

// Level 2 of the client is a live view of workspaces and their tabs, so both
// ranks need change events or the tree goes stale while the screen is open.
#[test]
fn every_rank_of_the_tree_can_be_removed_by_an_event() {
    for event in [
        WatchEvent::WorkspaceRemoved(WorkspaceId::parse("ws_c3").expect("valid")),
        WatchEvent::TabRemoved(TabId::parse("tb_b2").expect("valid")),
        WatchEvent::PaneRemoved(PaneId::parse("pn_a1").expect("valid")),
        WatchEvent::ConversationRemoved(ConversationId::parse("cv_9f21").expect("valid")),
        WatchEvent::Unblocked {
            question: QuestionId::parse("qs_a1").expect("valid"),
        },
    ] {
        let bytes = postcard::to_stdvec(&event).expect("encode");

        assert_eq!(
            postcard::from_bytes::<WatchEvent>(&bytes).expect("decode"),
            event
        );
    }
}

// An unblock names only the question. Sending the whole question back would
// invite a client to render a stale copy of something already answered.
#[test]
fn an_unblock_names_only_the_question_it_closes() {
    let event = WatchEvent::Unblocked {
        question: QuestionId::parse("qs_a1").expect("valid"),
    };

    match event {
        WatchEvent::Unblocked { question } => {
            assert_eq!(question.as_str(), "qs_a1");
        }
        _ => panic!("expected an unblock"),
    }
}

#[test]
fn a_watch_can_be_closed_in_an_orderly_way() {
    let control = WatchControl::Close;
    let bytes = postcard::to_stdvec(&control).expect("encode");

    assert_eq!(
        postcard::from_bytes::<WatchControl>(&bytes).expect("decode"),
        control
    );
}
