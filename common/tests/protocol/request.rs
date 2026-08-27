use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::protocol::request::Request;
use tethera_common::protocol::WireVersion;
use tethera_common::structs::conversation::ConversationFilter;
use tethera_common::structs::ids::{ConversationId, PaneId, ProfileId, QuestionId};
use tethera_common::structs::primitives::{Cursor, Fingerprint};
use tethera_common::structs::transcript::Answer;

fn round_trip(request: Request) {
    let bytes = postcard::to_stdvec(&request).expect("encode");

    assert_eq!(
        postcard::from_bytes::<Request>(&bytes).expect("decode"),
        request
    );
}

#[test]
fn a_transcript_request_round_trips() {
    round_trip(Request::Transcript {
        conversation: ConversationId::parse("cv_9f21").expect("valid"),
        before: Some(Cursor("t1:8814".into())),
        limit: 30,
    });
}

// `before: None` asks for the most recent page. Paging backwards is the client
// walking `next_before` from there.
#[test]
fn a_transcript_request_with_no_cursor_asks_for_the_most_recent_page() {
    let request = Request::Transcript {
        conversation: ConversationId::parse("cv_9f21").expect("valid"),
        before: None,
        limit: 30,
    };

    match request {
        Request::Transcript { before, .. } => assert!(before.is_none()),
        _ => panic!("expected a transcript request"),
    }
}

#[test]
fn starting_a_conversation_names_a_profile_and_a_directory() {
    round_trip(Request::StartConversation {
        profile: ProfileId("claude".into()),
        cwd: "/home/charl/projects/tethera".into(),
        prompt: Some("read the spec".into()),
        attachments: Vec::new(),
    });
}

// "Nothing starts until you press start." A preview asks the machine what it
// would create so the client can show a workspace name it did not invent.
#[test]
fn a_preview_creates_nothing_and_asks_for_names() {
    round_trip(Request::PreviewConversation {
        profile: ProfileId("claude".into()),
        cwd: "/home/charl/projects/tethera".into(),
        workspace: None,
    });
}

// Interrupt stops what the agent is doing; StopConversation ends the process.
// Different consequences, so the client can label them differently and a person
// cannot hit one meaning the other.
#[test]
fn interrupting_and_stopping_are_different_requests() {
    let conversation = ConversationId::parse("cv_9f21").expect("valid");

    assert_ne!(
        postcard::to_stdvec(&Request::Interrupt {
            conversation: conversation.clone()
        })
        .expect("encode"),
        postcard::to_stdvec(&Request::StopConversation { conversation }).expect("encode")
    );
}

#[test]
fn answering_a_question_carries_the_fingerprint_it_was_asked_with() {
    round_trip(Request::AnswerQuestion {
        conversation: ConversationId::parse("cv_9f21").expect("valid"),
        question: QuestionId::parse("qs_a1").expect("valid"),
        fingerprint: Fingerprint("9f21ab".into()),
        answers: vec![Answer::Choice(0)],
    });
}

// A client may create and destroy panes, because that is intent a person
// expressed. It may not move, resize or focus them: no such request exists.
#[test]
fn the_request_set_creates_and_destroys_panes_but_never_moves_them() {
    round_trip(Request::OpenTerminal {
        workspace: None,
        cwd: None,
    });
    round_trip(Request::ClosePane {
        pane: PaneId::parse("pn_a1").expect("valid"),
    });
}

#[test]
fn listing_conversations_takes_a_filter_and_a_cursor() {
    round_trip(Request::ListConversations {
        filter: ConversationFilter::Live,
        before: None,
        limit: 25,
    });
}

// Revoking is removing the identity, not forgetting a token: the endpoint id is
// the authentication, so there is nothing else to drop.
#[test]
fn a_device_can_revoke_itself() {
    round_trip(Request::RevokeThisDevice);
}

// A conversation with no live pane answers NotFound for the pane, which is the
// client's cue to offer a resume. Resuming starts a process on the machine, so
// it must be a deliberate tap and never an implicit fallback.
#[test]
fn a_missing_pane_is_distinguishable_from_a_missing_conversation() {
    assert_ne!(
        postcard::to_stdvec(&WireError::NotFound {
            kind: EntityKind::Pane
        })
        .expect("encode"),
        postcard::to_stdvec(&WireError::NotFound {
            kind: EntityKind::Conversation
        })
        .expect("encode")
    );
}

// No common version is a refusal that names what the server can speak, so a
// client can tell a person to update rather than showing "connection failed".
#[test]
fn a_version_refusal_names_what_the_server_speaks() {
    let error = WireError::NoCommonVersion {
        server_supports: vec![WireVersion(2), WireVersion(3)],
    };
    let bytes = postcard::to_stdvec(&error).expect("encode");

    assert_eq!(
        postcard::from_bytes::<WireError>(&bytes).expect("decode"),
        error
    );
}

// Describe is the first variant, so its encoded body is a single zero byte. If
// this changes, the variant order changed and every shipped client decodes the
// wrong request.
#[test]
fn describe_is_the_first_variant_and_encodes_to_one_zero_byte() {
    assert_eq!(
        postcard::to_stdvec(&Request::Describe).expect("encode"),
        vec![0u8]
    );
}
