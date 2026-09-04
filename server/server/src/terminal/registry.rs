use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::protocol::terminal::{CloseReason, RowUpdate, TerminalFrame};
use tethera_common::structs::ids::PaneId;
use tokio::sync::{mpsc, watch};

use crate::protocol::live::PaneSession;
use crate::protocol::ports::ScrollbackPage;
use crate::terminal::emulator::Emulator;
use crate::terminal::event::PaneEvent;
use crate::terminal::frames::FrameBuilder;
use crate::terminal::io::PaneIo;
use crate::terminal::source::PaneSource;
use crate::terminal::styles::StyleTable;

type Live = Arc<Mutex<HashMap<PaneId, Arc<PaneEmulator>>>>;

struct PaneState {
    emulator: Emulator,
    /// Bumped whenever a session drains the shared damage.
    ///
    /// Damage lives on one emulator and draining it consumes it, so only the
    /// session that drained can apply it. A session whose epoch is behind missed
    /// a drain and is sent a snapshot instead - which is what §4.3 already
    /// prescribes for a client that has fallen behind.
    epoch: u64,
    closed: Option<CloseReason>,
}

/// One pane's emulator, and the wakeup any session attached to it waits on.
///
/// Shared rather than owned by a session, because a pane outlives an attach:
/// scrollback that vanished when a phone locked its screen would make paging it a
/// lie.
pub struct PaneEmulator {
    state: Mutex<PaneState>,
    /// Bumped by the pump on every change.
    ///
    /// A `watch` rather than a `Notify`, because `notify_waiters` stores no
    /// permit: a wakeup landing between a session's check and its await would be
    /// lost, and `Attach::serve` polls `next_frame` inside a `select!`, so every
    /// client input frame cancels that future and drops its registration. A
    /// `watch` receiver remembers the version it has seen, so neither window can
    /// swallow a change.
    revision: watch::Sender<u64>,
    input: mpsc::Sender<Vec<u8>>,
}

impl PaneEmulator {
    /// Recovers a poisoned lock rather than propagating the panic.
    ///
    /// One panicked frame build would otherwise wedge this pane and every later
    /// attach to it for the life of the process. The emulator's own state
    /// self-heals on the next snapshot, so continuing is strictly better than
    /// refusing forever.
    fn state(&self) -> MutexGuard<'_, PaneState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.revision.subscribe()
    }

    /// How many sessions are attached to this pane right now.
    ///
    /// A `PaneSession` holds a revision receiver for its whole life and drops it
    /// when the attach ends, so the receiver count *is* the number of readers.
    /// Counted rather than tracked separately, because a second tally would be a
    /// second thing to get wrong on a path where being wrong means a read loop
    /// that never stops.
    pub fn watchers(&self) -> usize {
        self.revision.receiver_count()
    }

    pub fn input(&self) -> mpsc::Sender<Vec<u8>> {
        self.input.clone()
    }

    pub fn closed(&self) -> Option<CloseReason> {
        self.state().closed
    }

    /// What this pane currently has on screen, as text.
    pub fn screen_text(&self) -> String {
        self.state().emulator.screen().text()
    }

    /// Application cursor keys, and bracketed paste, as the pane last set them.
    pub fn modes(&self) -> (bool, bool) {
        let state = self.state();
        let screen = state.emulator.screen();

        (screen.application_cursor_keys(), screen.bracketed_paste())
    }

    /// The opening frame of an attach, and the epoch it starts from.
    ///
    /// Built without draining. A snapshot is the whole screen and does not need
    /// the pending damage, and draining it here would take it from whichever
    /// session is actually in sync — which then receives nothing and freezes on a
    /// screen missing that change. Re-applying damage a snapshot already covered
    /// is idempotent, so the one redundant frame this costs the new session is
    /// harmless.
    pub fn open(&self) -> (TerminalFrame, u64) {
        let state = self.state();
        let frame = FrameBuilder::snapshot(state.emulator.screen());

        (frame, state.epoch)
    }

    /// The next frame a session at `seen` is owed, and the epoch it moves to.
    pub fn next(&self, seen: u64) -> (Option<TerminalFrame>, u64) {
        let mut state = self.state();

        // Another session drained the damage this one would have needed, so
        // there is nothing left that would apply to the screen it last saw.
        //
        // Non-draining, and not bumping the epoch, for the same reason as `open`.
        // Bumping it here instead would make two sessions alternate mismatches
        // forever, each sending a full snapshot every budget tick with no output
        // at all.
        if state.epoch != seen {
            let frame = FrameBuilder::snapshot(state.emulator.screen());

            return (Some(frame), state.epoch);
        }

        match state.emulator.next_frame() {
            Some(frame) => {
                // A bell changes no cells, so it does not put any other session
                // behind for *damage* purposes. It is also consumed here, so with
                // two attaches only the session that polled first hears it. A
                // missed bell is cosmetic, which is why this is accepted rather
                // than given a third counter alongside the epoch.
                if !matches!(frame, TerminalFrame::Bell) {
                    state.epoch += 1;
                }

                let epoch = state.epoch;

                (Some(frame), epoch)
            }
            None => (None, state.epoch),
        }
    }
}

/// One emulator per pane, any number of attaches.
///
/// No source trait: a backend that owns a pane's bytes pushes it here when it
/// opens the pane, and a backend that owns none never pushes. Pulling at the
/// first attach would leave output produced between opening a pane and attaching
/// to it with nowhere to go.
pub struct PaneRegistry {
    /// Behind an `Arc` so a pump can hold a `Weak` to it and drop its own entry
    /// when its pane dies, without keeping the registry alive.
    live: Live,
}

impl PaneRegistry {
    pub fn new() -> Self {
        Self {
            live: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn new_shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    fn live(&self) -> MutexGuard<'_, HashMap<PaneId, Arc<PaneEmulator>>> {
        self.live
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A pane this machine just opened.
    ///
    /// Emulation starts now rather than at the first attach, so a pane's
    /// scrollback is complete from its first byte and nothing has to be buffered
    /// waiting for a client that may never arrive.
    pub fn adopt(&self, pane: PaneId, io: PaneIo, source: PaneSource) {
        let (revision, _) = watch::channel(0);
        let shared = Arc::new(PaneEmulator {
            state: Mutex::new(PaneState {
                emulator: Emulator::new(io.size),
                epoch: 0,
                closed: None,
            }),
            revision,
            input: io.input.clone(),
        });

        // Inserted before the pump starts, so a pane can never be removed by its
        // own pump before it was ever in the map.
        self.live().insert(pane.clone(), shared.clone());

        Self::spawn_pump(pane, shared, Arc::downgrade(&self.live), io.events, io.input, source);
    }

    /// Whether this registry is emulating the named pane.
    pub fn holds(&self, pane: &PaneId) -> bool {
        self.live().contains_key(pane)
    }

    pub fn attach(&self, pane: &PaneId) -> Result<PaneSession, WireError> {
        Ok(PaneSession::new(self.entry(pane)?))
    }

    /// What a pane has on screen, for anything that has to read it rather than
    /// be told. `None` for a pane this registry is not emulating.
    pub fn screen_of(&self, pane: &PaneId) -> Option<String> {
        self.live().get(pane).map(|shared| shared.screen_text())
    }

    /// Application cursor keys, and bracketed paste, as this pane last set them.
    ///
    /// Read here rather than guessed at, because they are the two modes that
    /// change what a keystroke means and the program on the far end set them on
    /// the stream this registry is already parsing. `None` for a pane it is not
    /// emulating, which is every pane on a backend that publishes no byte stream.
    pub fn modes_of(&self, pane: &PaneId) -> Option<(bool, bool)> {
        self.live().get(pane).map(|shared| shared.modes())
    }

    /// Stops emulating a pane, without waiting for its child to die.
    ///
    /// The pump removes its own entry when the waiter reports an exit, but that
    /// needs the child to actually exit, and a kill cannot report whether it did.
    /// So a caller that has decided a pane is gone says so here. A session
    /// already attached keeps its own `Arc` and still receives its farewell
    /// frame, and the pump's later removal is a no-op because it checks identity.
    pub fn forget(&self, pane: &PaneId) {
        self.live().remove(pane);
    }

    /// One page of a pane's own history, with the styles it was drawn in.
    pub fn scrollback(
        &self,
        pane: &PaneId,
        before_line: Option<u32>,
        limit: u16,
    ) -> Result<ScrollbackPage, WireError> {
        let shared = self.entry(pane)?;

        // Scoped, so the span build below runs with the emulator unlocked. Holding
        // it across up to 500 rows of cell copies would stall the pump - and the
        // pty reader behind it - for the length of a scrollback request.
        let (lines, next, has_earlier) = {
            let state = shared.state();

            state.emulator.screen().scrollback_page(before_line, limit)
        };

        let mut styles = StyleTable::new();
        let mut rows = Vec::new();

        for (index, line) in lines.iter().enumerate() {
            // The same builder the live frames use, not a copy of it. Scrollback
            // is where half-glyph orphans collect, because a row is evicted from
            // the screen without being repaired, so this is the path that needs
            // the orphan handling most.
            let spans = FrameBuilder::spans(line, &mut styles);

            rows.push(RowUpdate {
                y: u16::try_from(index).unwrap_or(u16::MAX),
                from_x: 0,
                spans,
            });
        }

        Ok((styles.into_vec(), rows, next, has_earlier))
    }

    fn entry(&self, pane: &PaneId) -> Result<Arc<PaneEmulator>, WireError> {
        // A pane nobody adopted has no byte stream on this machine. That is the
        // honest answer for a herdr pane, which is real but unreadable.
        self.live()
            .get(pane)
            .cloned()
            .ok_or(WireError::NotFound {
                kind: EntityKind::Pane,
            })
    }

    /// Feeds the emulator from its own task.
    ///
    /// If a session fed it instead, a phone on a slow link would stop it being
    /// fed, the bounded event channel would fill, and the backend's pty reader
    /// would block - the build would slow down because the phone is slow.
    fn spawn_pump(
        pane: PaneId,
        shared: Arc<PaneEmulator>,
        live: Weak<Mutex<HashMap<PaneId, Arc<PaneEmulator>>>>,
        mut events: mpsc::Receiver<PaneEvent>,
        input: mpsc::Sender<Vec<u8>>,
        source: PaneSource,
    ) {
        tokio::spawn(async move {
            loop {
                let reason = match events.recv().await {
                    Some(PaneEvent::Output(bytes)) => {
                        let replies = {
                            let mut state = shared.state();
                            state.emulator.feed(&bytes);

                            // Taken either way, so an unanswered query cannot
                            // accumulate in the emulator for the life of a
                            // relayed pane.
                            let replies = state.emulator.take_replies();

                            if source.answers_queries() {
                                replies
                            } else {
                                Vec::new()
                            }
                        };

                        // A program that asks for its cursor position and is
                        // never answered hangs. ConPTY asks before it will run
                        // anything at all, so this is what makes a pane start.
                        //
                        // `try_send`, never `send`. Awaiting here would make the
                        // pump - the sole consumer of the event channel - block on
                        // the input channel, so a program emitting queries faster
                        // than it drains its own input would stall the pump, fill
                        // the event channel and stop its own pty reader. Blocking
                        // is the right shape for output backpressure and the wrong
                        // shape for the server's own traffic: a dropped reply
                        // hangs one program, a stalled pump hangs the pane.
                        match Self::reply(&input, replies) {
                            Ok(()) => {
                                Self::bump(&shared);

                                None
                            }
                            Err(reason) => reason,
                        }
                    }
                    Some(PaneEvent::Resized(size)) => {
                        shared.state().emulator.resize(size);
                        Self::bump(&shared);

                        None
                    }
                    Some(PaneEvent::Closed(reason)) => Some(reason),
                    None => Some(CloseReason::PaneGone),
                };

                if let Some(reason) = reason {
                    shared.state().closed = Some(reason);
                    Self::bump(&shared);

                    // The entry goes with the pane. A session already attached
                    // keeps its own `Arc` and still receives its farewell frame,
                    // but a dead pane must not stay in the map: nothing else
                    // would remove it, because a detach is noticed only when a
                    // client's connection closes, and a phone that backgrounds
                    // mid-attach is not noticed until the QUIC idle timeout.
                    //
                    // Removed by identity, so a pane id reopened while this pump
                    // was dying does not take the new entry with the old one.
                    if let Some(live) = live.upgrade() {
                        let mut held = live
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());

                        if held
                            .get(&pane)
                            .map(|entry| Arc::ptr_eq(entry, &shared))
                            .unwrap_or(false)
                        {
                            held.remove(&pane);
                        }
                    }

                    return;
                }
            }
        });
    }

    fn bump(shared: &PaneEmulator) {
        shared.revision.send_modify(|revision| *revision += 1);
    }

    /// Answers a pane's own query without ever blocking the pump.
    ///
    /// `Err(Some(reason))` means the pane is gone. `Err(None)` means the reply was
    /// dropped and the pane is still alive.
    fn reply(
        input: &mpsc::Sender<Vec<u8>>,
        replies: Vec<u8>,
    ) -> Result<(), Option<CloseReason>> {
        if replies.is_empty() {
            return Ok(());
        }

        match input.try_send(replies) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Closed(_)) => Err(Some(CloseReason::PaneGone)),
            Err(mpsc::error::TrySendError::Full(dropped)) => {
                tracing::warn!(
                    bytes = dropped.len(),
                    "dropped a device reply: the pane is not draining its input"
                );

                Err(None)
            }
        }
    }
}

impl Default for PaneRegistry {
    fn default() -> Self {
        Self::new()
    }
}
