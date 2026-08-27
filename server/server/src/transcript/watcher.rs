use super::TranscriptReader;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tethera_common::protocol::watch::WatchEvent;
use tethera_common::structs::agent::Agent;
use tethera_common::structs::ids::{ConversationId, TurnId};
use tethera_common::structs::primitives::Cursor;
use tethera_common::structs::transcript::Turn;
use tokio::sync::broadcast;

/// The live tail of every conversation something is watching.
///
/// One poller per conversation, reused and reference counted. `subscribe` is
/// called more than once per stream - the watch handler calls it again on a
/// lagged receiver, wanting only the cursor - so without reuse every lag and
/// every reconnect would leak a task reading a file for the life of the process.
pub struct TranscriptWatcher {
    agent: Agent,
    live: Arc<Mutex<HashMap<ConversationId, broadcast::Sender<WatchEvent>>>>,
}

impl TranscriptWatcher {
    /// How often a watched file is asked whether it grew.
    ///
    /// A transcript one interval stale is indistinguishable from one that is
    /// not, and the cost is a `metadata` call per watched conversation.
    pub const POLL: Duration = Duration::from_millis(250);

    /// Enough that a burst of turns does not lag a client that is keeping up.
    pub const CAPACITY: usize = 256;

    /// How many turns at the tail are watched for change after they were sent.
    ///
    /// A tool result lands within a turn or two of the call it answers, so this
    /// is the window in which a turn can still be revised. Beyond it a turn is
    /// settled and re-reading it would be work for nothing.
    const REVISABLE: u16 = 8;

    pub fn new(agent: Agent) -> Self {
        Self {
            agent,
            live: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// A receiver for one conversation, starting a poller if none is running.
    pub fn subscribe(
        &self,
        id: &ConversationId,
        path: PathBuf,
        from: Cursor,
    ) -> broadcast::Receiver<WatchEvent> {
        let mut live = self.live.lock().expect("lock");

        if let Some(sender) = live.get(id) {
            if sender.receiver_count() > 0 {
                return sender.subscribe();
            }
        }

        let (sender, receiver) = broadcast::channel(Self::CAPACITY);
        live.insert(id.clone(), sender.clone());

        Self::poll(self.agent, path, from, sender, self.live.clone(), id.clone());

        receiver
    }

    /// How many conversations have a poller running.
    ///
    /// A behavioural accessor rather than a window into the map: one poller per
    /// watched conversation however many clients are watching it is the property
    /// that matters, and there is no other way to observe it.
    pub fn watching(&self) -> usize {
        self.live
            .lock()
            .expect("lock")
            .values()
            .filter(|sender| sender.receiver_count() > 0)
            .count()
    }

    fn poll(
        agent: Agent,
        path: PathBuf,
        from: Cursor,
        sender: broadcast::Sender<WatchEvent>,
        live: Arc<Mutex<HashMap<ConversationId, broadcast::Sender<WatchEvent>>>>,
        id: ConversationId,
    ) {
        tokio::spawn(async move {
            let reader = Arc::new(Mutex::new(TranscriptReader::open(path, agent)));
            let mut watch = TailWatch::new(from);

            loop {
                tokio::time::sleep(Self::POLL).await;

                // Nobody is listening any more. A poller outliving its last
                // receiver is a file read per interval for the life of the
                // process.
                if sender.receiver_count() == 0 {
                    live.lock().expect("lock").remove(&id);

                    return;
                }

                let handle = reader.clone();

                let tail = tokio::task::spawn_blocking(move || {
                    handle.lock().expect("lock").page(None, Self::REVISABLE)
                })
                .await;

                let Ok(Ok(page)) = tail else {
                    continue;
                };

                for event in watch.absorb(page.items) {
                    let _ = sender.send(event);
                }
            }
        });
    }
}

/// What the tail of a conversation has already told its subscribers.
///
/// Held so the poller can tell a turn nobody has seen from a turn whose tool
/// call has since been answered - both are things a client must hear about, and
/// they are different events.
///
/// Turns only. Whether a person is being *asked* something is `BlockWatch`'s,
/// because half of what an agent asks is drawn on screen and never written here
/// — and two watchers each emitting from their own source would send two
/// `Blocked` events for the one question a harness draws both ways.
struct TailWatch {
    sent: Cursor,
    /// The revisable window as it was last broadcast.
    drawn: HashMap<TurnId, Turn>,
}

impl TailWatch {
    fn new(from: Cursor) -> Self {
        Self {
            sent: from,
            drawn: HashMap::new(),
        }
    }

    /// The events a freshly read tail implies.
    fn absorb(&mut self, tail: Vec<Turn>) -> Vec<WatchEvent> {
        let mut events = Vec::new();
        let newest = self.sent.clone();
        let mut furthest = self.sent.clone();

        for turn in tail {
            let fresh = Self::after(&turn.cursor, &newest);

            // A turn already sent, re-read because a result may have landed on
            // it. `Turn.id` is stable across reads and is documented as the
            // dedupe key, so re-sending the same id is the defined update path
            // rather than a second turn.
            let revised = !fresh && self.drawn.get(&turn.id).is_some_and(|was| was != &turn);

            if !fresh && !revised {
                continue;
            }

            if fresh && Self::after(&turn.cursor, &furthest) {
                furthest = turn.cursor.clone();
            }

            self.drawn.insert(turn.id.clone(), turn.clone());
            events.push(WatchEvent::Turn(turn));
        }

        self.sent = furthest;

        events
    }

    /// Cursors are byte offsets behind an opaque prefix, so later means further
    /// into the file.
    fn after(cursor: &Cursor, than: &Cursor) -> bool {
        match (
            TranscriptReader::offset_of(cursor),
            TranscriptReader::offset_of(than),
        ) {
            (Some(left), Some(right)) => left > right,
            _ => false,
        }
    }
}
