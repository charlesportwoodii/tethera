use tethera_common::protocol::watch::WatchEvent;
use tethera_common::structs::ids::{PaneId, TabId, WorkspaceId};
use tethera_common::structs::terminal::{Pane, Size, Tab, Workspace};
use tethera_server_lib::protocol::live::TreeWatcher;
use tethera_server_lib::protocol::ports::TreeSnapshot;

fn workspace(name: &str) -> Workspace {
    Workspace::new(WorkspaceId::mint(name), name.to_string())
}

fn tab(name: &str, title: &str) -> Tab {
    Tab {
        id: TabId::mint(name),
        workspace_id: WorkspaceId::mint("ws"),
        index: 1,
        title: title.to_string(),
        conversation: None,
        foreground_command: None,
    }
}

fn pane(name: &str) -> Pane {
    Pane {
        id: PaneId::mint(name),
        tab_id: TabId::mint("tb"),
        workspace_id: WorkspaceId::mint("ws"),
        label: name.to_string(),
        title: None,
        cwd: None,
        size: Size {
            cols: 80,
            rows: 24,
        },
        focused: false,
        foreground_command: None,
        conversation: None,
        agent: None,
    }
}

fn snapshot(workspaces: Vec<Workspace>, tabs: Vec<Tab>, panes: Vec<Pane>) -> TreeSnapshot {
    TreeSnapshot {
        workspaces,
        tabs,
        panes,
        conversations: Vec::new(),
    }
}

// A diff of two snapshots cannot disagree with the tree a later reader sees. An
// event built by hand at each mutation site can.
#[test]
fn a_new_pane_appears_as_one_changed_event() {
    let before = snapshot(Vec::new(), Vec::new(), Vec::new());
    let after = snapshot(Vec::new(), Vec::new(), vec![pane("one")]);

    let events = TreeWatcher::diff(&before, &after);

    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], WatchEvent::PaneChanged(_)));
}

#[test]
fn a_pane_that_vanished_appears_as_removed() {
    let before = snapshot(Vec::new(), Vec::new(), vec![pane("one")]);
    let after = snapshot(Vec::new(), Vec::new(), Vec::new());

    let events = TreeWatcher::diff(&before, &after);

    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], WatchEvent::PaneRemoved(_)));
}

// Otherwise a rename costs the client a remove and an insert, and it loses
// whatever state it was holding against that row.
#[test]
fn a_renamed_tab_appears_once_as_changed_and_not_as_removed_and_added() {
    let before = snapshot(Vec::new(), vec![tab("one", "build")], Vec::new());
    let after = snapshot(Vec::new(), vec![tab("one", "test")], Vec::new());

    let events = TreeWatcher::diff(&before, &after);

    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], WatchEvent::TabChanged(_)));
}

// A watcher that re-sends the world on every tick is worse than no watcher,
// because the client redraws on each event.
#[test]
fn an_unchanged_tree_produces_no_events() {
    let tree = snapshot(
        vec![workspace("ws")],
        vec![tab("one", "build")],
        vec![pane("one")],
    );

    assert!(TreeWatcher::diff(&tree, &tree).is_empty());
}

#[test]
fn every_rank_is_diffed_and_not_only_panes() {
    let before = snapshot(Vec::new(), Vec::new(), Vec::new());
    let after = snapshot(
        vec![workspace("ws")],
        vec![tab("one", "build")],
        vec![pane("one")],
    );

    let events = TreeWatcher::diff(&before, &after);

    assert_eq!(events.len(), 3);
    assert!(events
        .iter()
        .any(|event| matches!(event, WatchEvent::WorkspaceChanged(_))));
    assert!(events
        .iter()
        .any(|event| matches!(event, WatchEvent::TabChanged(_))));
    assert!(events
        .iter()
        .any(|event| matches!(event, WatchEvent::PaneChanged(_))));
}

// The first snapshot is not a change. A watch opens with the whole tree in
// `WatchOpen`, so re-sending it as events would make a client redraw what it has
// just drawn.
#[test]
fn the_first_observation_sends_nothing() {
    let (events, mut receiver) = tokio::sync::broadcast::channel(16);
    let watcher = TreeWatcher::new(events);

    watcher.observe(snapshot(Vec::new(), Vec::new(), vec![pane("one")]));

    assert!(receiver.try_recv().is_err());
}

#[test]
fn a_later_observation_sends_what_changed() {
    let (events, mut receiver) = tokio::sync::broadcast::channel(16);
    let watcher = TreeWatcher::new(events);

    watcher.observe(snapshot(Vec::new(), Vec::new(), Vec::new()));
    watcher.observe(snapshot(Vec::new(), Vec::new(), vec![pane("one")]));

    assert!(matches!(
        receiver.try_recv(),
        Ok(WatchEvent::PaneChanged(_))
    ));
}
