use std::sync::Arc;

use tethera_common::protocol::handshake::{EnrollResult, Intent, RefuseReason, ServerHello};
use tethera_common::protocol::request::Request;
use tethera_common::protocol::response::{Payload, Response};
use tethera_common::protocol::terminal::{
    AttachSpec, Key, Mods, TerminalFrame, TerminalInput,
};
use tethera_common::protocol::transfer::{FetchSpec, PutSpec};
use tethera_common::protocol::watch::{WatchEvent, WatchOpen, WatchSpec};
use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::protocol::grid::TerminalGrid;
use tethera_common::structs::conversation::ConversationFilter;
use tethera_common::structs::ids::PaneId;
use tethera_common::structs::terminal::Size;
use tethera_common::structs::primitives::{Cursor, Fingerprint, Sha256};
use tethera_common::structs::transcript::Answer;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

use super::client::Harness;
use super::fakes::{self, FakeAssets, FakeConversations, FakePorts};

/// Enrols a device and returns a harness whose connection is a session.
///
/// Enrolment happens on one connection and the session on the next, which is
/// what a phone actually does: the pairing screen closes and the app reconnects.
async fn enrolled() -> Harness {
    let ports = Arc::new(FakePorts::new());

    {
        let pairing = Harness::start_with(ports.clone()).await;
        let (offer, mut send, mut recv) = pairing.hello(Intent::Enroll).await;

        assert!(matches!(offer, ServerHello::EnrollPending { .. }));

        // A wrong code first, because a pairing flow that only ever sees the
        // right one has never proved it can refuse.
        match pairing.type_code(&mut send, &mut recv, "000000").await {
            EnrollResult::Refused { attempts_left, .. } => assert_eq!(attempts_left, 2),
            other => panic!("expected a refusal, got {other:?}"),
        }

        match pairing.type_code(&mut send, &mut recv, fakes::CODE).await {
            EnrollResult::Accepted { .. } => {}
            other => panic!("expected an acceptance, got {other:?}"),
        }
    }

    let session = Harness::start_with(ports).await;
    let (answer, _send, _recv) = session.hello(Intent::Session).await;

    // No code is asked for the second time. That is the whole point of
    // enrolment.
    assert!(
        matches!(answer, ServerHello::Session { .. }),
        "expected a session, got {answer:?}"
    );

    session
}

/// The protocol, in order, over one connection.
///
/// Every assertion here is a sentence from the spec. This test is the evidence
/// that the wire works: the types round-tripping proves only that postcard is
/// symmetric.
#[tokio::test]
async fn the_whole_protocol_over_one_connection() {
    let harness = enrolled().await;

    // --- the machine answers what it can do, rather than being inferred from a
    // --- version string
    let Payload::Describe(describe) = harness.payload(Request::Describe).await else {
        panic!("expected a describe");
    };

    assert_eq!(describe.server.label, "atlas");
    assert_eq!(describe.limits.max_control_frame, 64 * 1024);
    assert!(!describe.capabilities.is_empty());

    // --- one frame carries every rank of the tree
    let (open, mut tree_events) = harness.watch(WatchSpec::Machine).await;

    let WatchOpen::Machine {
        workspaces,
        tabs,
        panes,
        conversations,
        layouts: _,
    } = open
    else {
        panic!("expected a machine snapshot");
    };

    assert_eq!(workspaces.len(), 2);
    assert_eq!(tabs.len(), 3);
    assert_eq!(panes.len(), 3);
    assert_eq!(conversations.len(), 2);

    // --- a conversation with no live pane is listed, because history reads
    // --- whether or not the machine is running anything
    let Payload::Conversations(page) = harness
        .payload(Request::ListConversations {
            filter: ConversationFilter::All,
            before: None,
            limit: 25,
        })
        .await
    else {
        panic!("expected conversations");
    };

    let unbound = page
        .items
        .iter()
        .find(|c| c.binding.is_none())
        .expect("an unbound conversation");

    assert!(unbound.has_transcript);

    // --- paging backwards walks next_before to the start and stops, and every
    // --- turn arrives exactly once
    let mut before: Option<Cursor> = None;
    let mut collected = Vec::new();

    loop {
        let Payload::Transcript(page) = harness
            .payload(Request::Transcript {
                conversation: fakes::bound_conversation(),
                before: before.clone(),
                limit: 10,
            })
            .await
        else {
            panic!("expected a transcript");
        };

        // Oldest first within a page.
        for turn in &page.items {
            collected.push(turn.id.as_str().to_string());
        }

        if !page.has_earlier {
            assert!(page.next_before.is_none());
            break;
        }

        before = page.next_before.clone();
        assert!(before.is_some(), "has_earlier with no cursor is unwalkable");
    }

    assert_eq!(collected.len(), fakes::TURNS);

    let unique: std::collections::BTreeSet<&String> = collected.iter().collect();
    assert_eq!(unique.len(), fakes::TURNS, "a turn arrived twice");

    // --- a conversation watch says where it actually starts
    let newest = FakeConversations::newest_cursor();
    let (open, mut turn_events) = harness
        .watch(WatchSpec::Conversation {
            id: fakes::bound_conversation(),
            after: Some(newest.clone()),
        })
        .await;

    let WatchOpen::Conversation { from, .. } = open else {
        panic!("expected a conversation snapshot");
    };

    assert_eq!(from, newest, "a fresh cursor must be honoured exactly");

    // --- a pushed turn reaches the watch
    harness
        .ports
        .publish_turn(WatchEvent::Unblocked {
            question: fakes::question_id(),
        });

    let event: WatchEvent = FrameIo::read(&mut turn_events, harness.codec())
        .await
        .expect("read")
        .expect("an event");

    assert!(matches!(event, WatchEvent::Unblocked { .. }));

    // --- a stale fingerprint is refused rather than answering the wrong question
    let stale = harness
        .rpc(Request::AnswerQuestion {
            conversation: fakes::bound_conversation(),
            question: fakes::question_id(),
            fingerprint: Fingerprint("not-the-one".into()),
            answers: vec![Answer::Choice(0)],
        })
        .await;

    assert_eq!(stale, Response::Err(WireError::Stale));

    // --- the right fingerprint is accepted
    let answered = harness
        .rpc(Request::AnswerQuestion {
            conversation: fakes::bound_conversation(),
            question: fakes::question_id(),
            fingerprint: fakes::question().fingerprint,
            answers: vec![Answer::Choice(0)],
        })
        .await;

    assert_eq!(answered, Response::Ok(Payload::Ack));

    // --- a create answers with what it made
    let Payload::Pane(pane) = harness
        .payload(Request::OpenTerminal {
            workspace: None,
            cwd: Some("/tmp".into()),
        })
        .await
    else {
        panic!("expected a pane");
    };

    assert_eq!(pane.cwd.as_deref(), Some("/tmp"));
    assert_eq!(pane.size.cols, 120);

    // --- a tree change reaches the machine watch that is still open
    harness
        .ports
        .publish_tree_event(WatchEvent::PaneRemoved(pane.id.clone()));

    let event: WatchEvent = FrameIo::read(&mut tree_events, harness.codec())
        .await
        .expect("read")
        .expect("an event");

    assert_eq!(event, WatchEvent::PaneRemoved(pane.id));

    // --- attaching opens with a snapshot before any damage, and the damage
    // --- applies to the grid exactly as the spec says
    let (mut input, mut frames) = harness
        .attach(AttachSpec {
            pane: fakes::agent_pane(),
            viewport: Size { cols: 80, rows: 24 },
        })
        .await;

    let first: TerminalFrame = FrameIo::read(&mut frames, harness.codec())
        .await
        .expect("read")
        .expect("a frame");

    assert!(
        matches!(first, TerminalFrame::Snapshot { .. }),
        "an attach must open with a snapshot"
    );

    let mut grid = TerminalGrid::default();
    grid.apply(&first);
    assert_eq!(grid.line(0), "hello ");

    for _ in 0..2 {
        let damage: TerminalFrame = FrameIo::read(&mut frames, harness.codec())
            .await
            .expect("read")
            .expect("a frame");

        assert!(matches!(damage, TerminalFrame::Damage { .. }));
        grid.apply(&damage);
    }

    assert_eq!(grid.line(0), "first ");
    assert_eq!(grid.line(1), "second");

    // --- CTRL+C reaches the pane as intent, not as bytes
    FrameIo::write(
        &mut input,
        harness.codec(),
        &TerminalInput::Key {
            key: Key::Char('c'),
            mods: Mods::CTRL,
        },
    )
    .await
    .expect("write input");

    let recorded = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let inputs = harness.ports.recorded_inputs();

            if !inputs.is_empty() {
                return inputs;
            }

            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the pane received input");

    assert_eq!(
        recorded[0],
        TerminalInput::Key {
            key: Key::Char('c'),
            mods: Mods::CTRL,
        }
    );

    // --- an upload is told where to start before it sends a byte
    let whole = FakeAssets::body();
    let (ready, result) = harness
        .put(
            PutSpec {
                name: "screenshot.png".into(),
                len: whole.len() as u64,
                sha256: Sha256("e3b0c442".into()),
                offset: 0,
            },
            &whole,
        )
        .await;

    assert_eq!(ready.offset, 0);
    assert!(result.asset.as_str().starts_with("as_"));

    // --- a resumed upload seeks to the offset the server named, not the one the
    // --- client proposed
    let (ready, _result) = harness
        .put(
            PutSpec {
                name: fakes::RESUMED_NAME.into(),
                len: whole.len() as u64,
                // A deliberately wrong proposal. Only the server knows how much
                // of the earlier attempt reached disk.
                sha256: Sha256("e3b0c442".into()),
                offset: 0,
            },
            &whole,
        )
        .await;

    assert_eq!(ready.offset, fakes::RESUMED_OFFSET);

    // --- a fetch states the whole size and starts where the server says
    let (head, body) = harness
        .fetch(FetchSpec {
            asset: FakeAssets::asset(),
            offset: 1000,
        })
        .await;

    assert_eq!(head.len, fakes::ASSET_BYTES as u64, "len is the whole asset");
    assert_eq!(head.offset, 1000);
    assert_eq!(body, whole[1000..]);

    // --- a prompt to a conversation with no live pane is a missing pane, which
    // --- is the client's cue to offer a resume rather than resuming silently
    let unbound_prompt = harness
        .rpc(Request::SendPrompt {
            conversation: fakes::unbound_conversation(),
            text: "are you there".into(),
            attachments: Vec::new(),
        })
        .await;

    assert_eq!(
        unbound_prompt,
        Response::Err(WireError::NotFound {
            kind: EntityKind::Pane
        })
    );
}

// Starting a conversation proves it is alive before it answers, because that
// call can take tens of seconds. The predecessor's version of this was a request
// that never returned and logged nothing.
#[tokio::test]
async fn a_slow_call_sends_progress_then_exactly_one_terminal_frame() {
    let harness = enrolled().await;

    let frames = harness
        .rpc_frames(Request::StartConversation {
            profile: tethera_common::structs::ids::ProfileId("claude".into()),
            cwd: "/tmp".into(),
            prompt: None,
            attachments: Vec::new(),
        })
        .await;

    assert!(frames.len() >= 2, "expected progress before the result");
    assert!(
        frames[..frames.len() - 1]
            .iter()
            .all(|frame| !frame.is_terminal()),
        "only the last frame may be terminal"
    );
    assert!(frames.last().expect("a frame").is_terminal());
}

// Revocation removes the identity, and it takes effect on the next connection.
#[tokio::test]
async fn revoking_this_device_refuses_its_next_connection() {
    let ports = Arc::new(FakePorts::new());

    {
        let pairing = Harness::start_with(ports.clone()).await;
        let (_offer, mut send, mut recv) = pairing.hello(Intent::Enroll).await;
        pairing.type_code(&mut send, &mut recv, fakes::CODE).await;
    }

    {
        let session = Harness::start_with(ports.clone()).await;
        let (answer, _s, _r) = session.hello(Intent::Session).await;
        assert!(matches!(answer, ServerHello::Session { .. }));

        assert_eq!(
            session.rpc(Request::RevokeThisDevice).await,
            Response::Ok(Payload::Ack)
        );
    }

    let after = Harness::start_with(ports).await;
    let (answer, _s, _r) = after.hello(Intent::Session).await;

    assert_eq!(answer, ServerHello::Refuse(RefuseReason::Revoked));
}

// Attaching a pane that does not exist ends the stream with a reason rather than
// hanging, so a client is never left waiting on a screen that will never draw.
#[tokio::test]
async fn attaching_an_unknown_pane_closes_with_a_reason() {
    let harness = enrolled().await;
    let (_input, mut frames) = harness
        .attach(AttachSpec {
            pane: PaneId::parse("pn_nothere").expect("valid"),
            viewport: Size { cols: 80, rows: 24 },
        })
        .await;

    let frame: TerminalFrame = FrameIo::read(&mut frames, harness.codec())
        .await
        .expect("read")
        .expect("a frame");

    assert!(matches!(frame, TerminalFrame::Closed { .. }));
}

// A cursor older than the earliest surviving record must not silently produce a
// hole: `from` comes back later than asked, and the client refetches the gap.
#[tokio::test]
async fn a_cursor_older_than_the_source_opens_later_and_says_so() {
    let harness = enrolled().await;

    let (open, _events) = harness
        .watch(WatchSpec::Conversation {
            id: fakes::bound_conversation(),
            after: Some(Cursor("t0".into())),
        })
        .await;

    let WatchOpen::Conversation { from, .. } = open else {
        panic!("expected a conversation snapshot");
    };

    assert_ne!(
        from,
        Cursor("t0".into()),
        "the server must say it could not honour the cursor"
    );
}

// An unknown conversation answers rather than hanging.
#[tokio::test]
async fn an_unknown_conversation_answers_not_found() {
    let harness = enrolled().await;

    let response = harness
        .rpc(Request::GetConversation {
            conversation: tethera_common::structs::ids::ConversationId::parse("cv_nothere")
                .expect("valid"),
        })
        .await;

    assert_eq!(
        response,
        Response::Err(WireError::NotFound {
            kind: EntityKind::Conversation
        })
    );
}

// Every framed exchange above rides the same codec, so a control frame cap
// change would break all of it at once. Pinning it here means that shows up as
// one obvious failure rather than a dozen mysterious ones.
#[test]
fn the_protocol_uses_the_control_frame_cap() {
    assert_eq!(FrameCodec::default().encode(&()).expect("encode").len(), 4);
    assert_eq!(FrameCodec::CONTROL_MAX_FRAME_BYTES, 64 * 1024);
}
