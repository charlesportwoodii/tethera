use tethera_common::structs::agent::AgentStatus;
use tethera_common::structs::conversation::{Conversation, ConversationFilter};
use tethera_common::structs::ids::{ConversationId, PaneId, ProfileId, WorkspaceId};
use tethera_common::structs::primitives::Timestamp;

fn a_conversation(binding: Option<PaneId>) -> Conversation {
    Conversation {
        id: ConversationId::parse("cv_9f21").expect("valid"),
        profile: ProfileId("claude".into()),
        profile_label: "Claude Code".into(),
        title: Some("protocol design".into()),
        preview: Some("Which route should own tethera://pair?".into()),
        cwd: "/home/charl/projects/tethera".into(),
        workspace: Some(WorkspaceId::parse("ws_c3").expect("valid")),
        started_at: Timestamp(1_766_000_000_000),
        last_active: Some(Timestamp(1_766_000_900_000)),
        turn_count: Some(42),
        status: AgentStatus::Blocked,
        has_transcript: true,
        resumable: true,
        binding,
    }
}

// A conversation outlives any pane. The machine rebooted, the multiplexer is
// gone, and the list still shows last week's work: that is the product.
#[test]
fn a_conversation_with_no_live_pane_is_representable() {
    assert!(a_conversation(None).binding.is_none());
}

#[test]
fn a_conversation_round_trips_through_postcard() {
    let conversation = a_conversation(Some(PaneId::parse("pn_a1").expect("valid")));
    let bytes = postcard::to_stdvec(&conversation).expect("encode");

    assert_eq!(
        postcard::from_bytes::<Conversation>(&bytes).expect("decode"),
        conversation
    );
}

// The tree groups conversations under workspaces, and an unbound conversation
// has no pane to walk up through - which is exactly the rebooted-machine case
// where the grouping matters most.
#[test]
fn a_conversation_names_its_workspace_directly() {
    assert_eq!(
        a_conversation(None).workspace,
        Some(WorkspaceId::parse("ws_c3").expect("valid"))
    );
}

// Drawing a home screen across three machines must not cost a transcript
// request per conversation before anything appears.
#[test]
fn a_conversation_carries_one_line_of_recent_text() {
    assert!(a_conversation(None).preview.is_some());
}

// A pane whose agent has no readable transcript has no conversation surface at
// all, and the client has to know before it opens a screen so it can offer the
// terminal instead of an empty conversation.
#[test]
fn a_conversation_says_whether_it_has_a_readable_transcript() {
    let mut conversation = a_conversation(None);
    conversation.has_transcript = false;

    let bytes = postcard::to_stdvec(&conversation).expect("encode");
    let back: Conversation = postcard::from_bytes(&bytes).expect("decode");

    assert!(!back.has_transcript);
}

#[test]
fn every_conversation_filter_round_trips() {
    for filter in [
        ConversationFilter::All,
        ConversationFilter::Live,
        ConversationFilter::Blocked,
    ] {
        let bytes = postcard::to_stdvec(&filter).expect("encode");

        assert_eq!(
            postcard::from_bytes::<ConversationFilter>(&bytes).expect("decode"),
            filter
        );
    }
}
