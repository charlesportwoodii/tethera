use std::sync::Mutex;

use tethera_common::protocol::watch::WatchEvent;
use tokio::sync::broadcast;

use crate::protocol::ports::TreeSnapshot;

/// Turns successive tree snapshots into `WatchEvent`s.
///
/// `MachinePort::tree_events` returns a receiver, and nothing sent on it. herdr
/// does have `events.subscribe` with 26 event kinds, but it is a socket-API
/// method with no CLI form — `herdr --help` has no `events` subcommand — and the
/// backend shells the CLI. Reaching it means speaking herdr's named pipe, which
/// is its own piece of work.
///
/// So this diffs instead, on every read of the tree. Reads come from a client
/// RPC and from the runtime's own heartbeat, which is what makes a watch fire for
/// a client that opens one and then waits — so the effective interval is the
/// heartbeat's, `Runtime::ADDRESS_HEARTBEAT`. That is not as good as a
/// subscription and is not pretending to be: it is bounded, it works for every
/// backend, and it degrades honestly, where the alternative is a watch that never
/// fires.
///
/// Diffing rather than emitting an event at each mutation site is deliberate. An
/// event built by hand at a call site can disagree with the tree the next reader
/// sees; a diff of two snapshots cannot.
pub struct TreeWatcher {
    events: broadcast::Sender<WatchEvent>,
    last: Mutex<Option<TreeSnapshot>>,
}

impl TreeWatcher {
    pub fn new(events: broadcast::Sender<WatchEvent>) -> Self {
        Self {
            events,
            last: Mutex::new(None),
        }
    }

    /// Every change between two snapshots, one event per change.
    ///
    /// A row present in both and unequal is `Changed`. Present only in the older
    /// is `Removed`. Present only in the newer is also `Changed`: the event set
    /// has no `Added`, and a client applying a change to an id it has not seen
    /// treats it as an insert.
    pub fn diff(previous: &TreeSnapshot, next: &TreeSnapshot) -> Vec<WatchEvent> {
        let mut events = Vec::new();

        Self::diff_rank(
            &previous.workspaces,
            &next.workspaces,
            |workspace| workspace.id.clone(),
            |workspace| WatchEvent::WorkspaceChanged(workspace.clone()),
            |id| WatchEvent::WorkspaceRemoved(id),
            &mut events,
        );
        Self::diff_rank(
            &previous.tabs,
            &next.tabs,
            |tab| tab.id.clone(),
            |tab| WatchEvent::TabChanged(tab.clone()),
            |id| WatchEvent::TabRemoved(id),
            &mut events,
        );
        Self::diff_rank(
            &previous.panes,
            &next.panes,
            |pane| pane.id.clone(),
            |pane| WatchEvent::PaneChanged(pane.clone()),
            |id| WatchEvent::PaneRemoved(id),
            &mut events,
        );
        Self::diff_rank(
            &previous.conversations,
            &next.conversations,
            |conversation| conversation.id.clone(),
            |conversation| WatchEvent::ConversationChanged(conversation.clone()),
            |id| WatchEvent::ConversationRemoved(id),
            &mut events,
        );

        events
    }

    /// Records a snapshot and sends one event per change against the last one.
    ///
    /// The first snapshot sends nothing: a watch opens with the whole tree in
    /// `WatchOpen`, so re-sending it as changes would make every client redraw
    /// what it has just drawn.
    pub fn observe(&self, next: TreeSnapshot) {
        let mut held = self.last.lock().unwrap_or_else(|p| p.into_inner());

        if let Some(previous) = held.as_ref() {
            for event in Self::diff(previous, &next) {
                // A send with no subscriber is not a failure: nothing is
                // watching, and the next watch opens with a fresh snapshot.
                let _ = self.events.send(event);
            }
        }

        *held = Some(next);
    }

    fn diff_rank<T, K, Id, Changed, Removed>(
        previous: &[T],
        next: &[T],
        id: Id,
        changed: Changed,
        removed: Removed,
        events: &mut Vec<WatchEvent>,
    ) where
        T: PartialEq,
        K: PartialEq,
        Id: Fn(&T) -> K,
        Changed: Fn(&T) -> WatchEvent,
        Removed: Fn(K) -> WatchEvent,
    {
        for item in next {
            let key = id(item);

            match previous.iter().find(|held| id(held) == key) {
                Some(held) if held == item => {}
                _ => events.push(changed(item)),
            }
        }

        for item in previous {
            let key = id(item);

            if !next.iter().any(|held| id(held) == key) {
                events.push(removed(key));
            }
        }
    }
}
