use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::protocol::response::{
    ConversationPreview, Limits, Page, Payload, Progress, ProgressStage, Response,
};
use tethera_common::structs::ids::{PaneId, TabId, WorkspaceId};
use tethera_common::structs::terminal::{Pane, Size};

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
        focused: true,
        foreground_command: None,
        conversation: None,
        agent: None,
        streamed: false,
    }
}

// A slow operation proves it is alive. The predecessor's symptom was "a new
// workspace appears but the agent never starts", intermittently, with nothing
// logged at either end.
#[test]
fn a_progress_frame_round_trips() {
    let response = Response::Progress(Progress {
        stage: ProgressStage::StartingAgent,
        detail: Some("waiting for the prompt".into()),
    });
    let bytes = postcard::to_stdvec(&response).expect("encode");

    assert_eq!(
        postcard::from_bytes::<Response>(&bytes).expect("decode"),
        response
    );
}

#[test]
fn every_progress_stage_round_trips() {
    for stage in [
        ProgressStage::Accepted,
        ProgressStage::WaitingOnBackend,
        ProgressStage::StartingAgent,
        ProgressStage::Ready,
    ] {
        let bytes = postcard::to_stdvec(&stage).expect("encode");

        assert_eq!(
            postcard::from_bytes::<ProgressStage>(&bytes).expect("decode"),
            stage
        );
    }
}

// A handler writes zero or more non-terminal frames then exactly one terminal
// frame. A client that saw two - or none - could not tell a finished call from
// a stalled one.
#[test]
fn progress_is_not_terminal_and_a_result_is() {
    assert!(!Response::Progress(Progress {
        stage: ProgressStage::Accepted,
        detail: None,
    })
    .is_terminal());

    assert!(Response::Ok(Payload::Ack).is_terminal());
    assert!(Response::Err(WireError::Busy).is_terminal());
}

// A create answers with what it made. Never create-then-list to find it.
#[test]
fn opening_a_terminal_answers_with_the_pane_it_made() {
    let response = Response::Ok(Payload::Pane(a_pane()));
    let bytes = postcard::to_stdvec(&response).expect("encode");

    assert_eq!(
        postcard::from_bytes::<Response>(&bytes).expect("decode"),
        response
    );
}

#[test]
fn an_error_response_round_trips() {
    let response = Response::Err(WireError::NotFound {
        kind: EntityKind::Pane,
    });
    let bytes = postcard::to_stdvec(&response).expect("encode");

    assert_eq!(
        postcard::from_bytes::<Response>(&bytes).expect("decode"),
        response
    );
}

// `has_earlier` is the source's own answer, believed over any client heuristic.
// A pane's scroll metric answers the wrong question: an agent owning the
// alternate screen reports no scrollback while its transcript runs to megabytes.
#[test]
fn a_page_states_whether_more_exists_rather_than_leaving_it_to_be_inferred() {
    let page: Page<String> = Page {
        items: vec!["one".into()],
        next_before: None,
        has_earlier: true,
    };

    let bytes = postcard::to_stdvec(&page).expect("encode");
    let back: Page<String> = postcard::from_bytes(&bytes).expect("decode");

    assert!(back.has_earlier);
    assert!(back.next_before.is_none());
}

// An empty page is not the same as a page with nothing more behind it: a filter
// that matches nothing still has to say whether earlier pages exist.
#[test]
fn an_empty_page_still_answers_whether_more_exists() {
    let page: Page<String> = Page {
        items: Vec::new(),
        next_before: None,
        has_earlier: false,
    };

    let bytes = postcard::to_stdvec(&page).expect("encode");
    let back: Page<String> = postcard::from_bytes(&bytes).expect("decode");

    assert!(back.items.is_empty());
    assert!(!back.has_earlier);
}

// The bounds a machine enforces, so a client can size its own requests rather
// than discovering a limit by hitting it.
#[test]
fn limits_round_trip_and_an_absent_upload_bound_is_none() {
    let limits = Limits {
        max_control_frame: 64 * 1024,
        max_streams: 64,
        transcript_page: 200,
        scrollback_page: 500,
        max_upload: None,
    };

    let bytes = postcard::to_stdvec(&limits).expect("encode");
    let back: Limits = postcard::from_bytes(&bytes).expect("decode");

    assert_eq!(back, limits);
    assert!(back.max_upload.is_none());
}

#[test]
fn a_conversation_preview_names_what_would_be_created() {
    let preview = ConversationPreview {
        workspace_label: "tethera-4".into(),
        tab_label: "claude".into(),
        creates_workspace: true,
        will_have_transcript: false,
    };

    let bytes = postcard::to_stdvec(&preview).expect("encode");

    assert_eq!(
        postcard::from_bytes::<ConversationPreview>(&bytes).expect("decode"),
        preview
    );
}
