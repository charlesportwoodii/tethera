use tethera_common::structs::ids::{ConversationId, PaneId, TabId, WorkspaceId};
use tethera_common::structs::terminal::{Pane, Size, SplitDirection, Tab, Workspace};

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
    }
}

#[test]
fn a_pane_round_trips_through_postcard() {
    let pane = a_pane();
    let bytes = postcard::to_stdvec(&pane).expect("encode");

    assert_eq!(postcard::from_bytes::<Pane>(&bytes).expect("decode"), pane);
}

// Size is observed, never requested. Geometry is decided by the server when it
// creates a pane and is stable for that pane's life; a client wanting a
// different size scales its own rendering.
#[test]
fn a_pane_carries_the_geometry_it_was_created_with() {
    assert_eq!(
        a_pane().size,
        Size {
            cols: 200,
            rows: 50
        }
    );
}

// Absent is not zero. A title the backend did not report is None, not an empty
// string that renders as a blank label pretending to be a value.
#[test]
fn unreported_pane_fields_are_absent_rather_than_empty() {
    let pane = a_pane();

    assert!(pane.title.is_none());
    assert!(pane.cwd.is_none());
    assert!(pane.foreground_command.is_none());
}

// A collapsed workspace row says "2 tabs" without fetching them.
#[test]
fn a_workspace_states_its_tab_count_without_a_second_request() {
    let workspace = Workspace {
        id: WorkspaceId::parse("ws_c3").expect("valid"),
        name: "tethera-3".into(),
        cwd: Some("/home/charl/projects/tethera".into()),
        tab_count: 3,
        conversation: Some(ConversationId::parse("cv_d4").expect("valid")),
    };

    let bytes = postcard::to_stdvec(&workspace).expect("encode");
    let back: Workspace = postcard::from_bytes(&bytes).expect("decode");

    assert_eq!(back, workspace);
    assert_eq!(back.tab_count, 3);
}

// A tab row draws its own status glyph and what is running in it. Both facts
// live on Pane, and without them the client issues a ListPanes per tab - five
// workspaces on three machines is a request storm on a phone.
#[test]
fn a_tab_carries_enough_to_draw_its_own_row() {
    let agent_tab = Tab {
        id: TabId::parse("tb_b2").expect("valid"),
        workspace_id: WorkspaceId::parse("ws_c3").expect("valid"),
        index: 1,
        title: "claude".into(),
        conversation: Some(ConversationId::parse("cv_d4").expect("valid")),
        foreground_command: None,
    };
    let shell_tab = Tab {
        id: TabId::parse("tb_e5").expect("valid"),
        workspace_id: WorkspaceId::parse("ws_c3").expect("valid"),
        index: 2,
        title: "build".into(),
        conversation: None,
        foreground_command: Some("cargo".into()),
    };

    assert!(agent_tab.conversation.is_some());
    assert_eq!(shell_tab.foreground_command.as_deref(), Some("cargo"));

    for tab in [agent_tab, shell_tab] {
        let bytes = postcard::to_stdvec(&tab).expect("encode");

        assert_eq!(postcard::from_bytes::<Tab>(&bytes).expect("decode"), tab);
    }
}

// An index assigned by list position would renumber when a tab closes, and the
// person using the machine calls it `2:build`.
#[test]
fn a_tab_index_is_the_backends_own_ordinal() {
    let tab = Tab {
        id: TabId::parse("tb_e5").expect("valid"),
        workspace_id: WorkspaceId::parse("ws_c3").expect("valid"),
        index: 7,
        title: "build".into(),
        conversation: None,
        foreground_command: None,
    };

    assert_eq!(tab.index, 7);
}

#[test]
fn both_split_directions_round_trip() {
    for direction in [SplitDirection::Horizontal, SplitDirection::Vertical] {
        let bytes = postcard::to_stdvec(&direction).expect("encode");

        assert_eq!(
            postcard::from_bytes::<SplitDirection>(&bytes).expect("decode"),
            direction
        );
    }
}
