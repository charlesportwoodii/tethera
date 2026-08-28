use std::sync::Arc;
use std::time::{Duration, Instant};

use tethera_common::protocol::terminal::CloseReason;
use tethera_common::protocol::view::PaneView;
use tethera_common::structs::ids::PaneId;
use tethera_common::structs::terminal::Size;
use tokio::sync::Semaphore;

use crate::backend::{BackendError, TerminalBackend};
use crate::terminal::delta::{Advance, OutputDelta};
use crate::terminal::event::PaneEvent;
use crate::terminal::io::PaneIo;
use crate::terminal::registry::PaneRegistry;
use crate::terminal::source::PaneSource;

/// A herdr pane, read on a timer and fed to an emulator as if it were a stream.
///
/// herdr publishes no per-pane bytes and announces nothing when a pane produces
/// output - measured, in `docs/findings/phone-terminal/task-00-subscription.md`,
/// against every event and counter its socket API offers. So the only way to
/// watch a pane is to read it and compare.
///
/// Nothing above `PaneRegistry::adopt` can tell this apart from a pty. That is
/// the point of the seam: the emulator, the damage tracking, the frame budget
/// and the attach stream are all shared, and only the pusher differs.
pub struct HerdrSource;

impl HerdrSource {
    /// How often an attached pane is re-read.
    ///
    /// Four times a second is faster than a person reading output on a phone
    /// perceives as steps, and the frame budget downstream caps emission at
    /// thirty a second anyway, so a shorter interval would buy nothing it could
    /// deliver. A read costs about 30ms of subprocess, measured, so this leaves
    /// the machine idle most of the time.
    pub const READ_INTERVAL: Duration = Duration::from_millis(250);

    /// How long a pane with no readers is kept before the loop gives up.
    ///
    /// Not immediate: a phone that locks its screen and comes back, or a view
    /// switched from Lines to Screen, both detach and re-attach within a second.
    /// Tearing the emulator down in between would throw away the scrollback they
    /// are about to ask for again.
    pub const IDLE_GRACE: Duration = Duration::from_secs(10);

    /// How much history each read asks for.
    ///
    /// This is the overlap window `OutputDelta` rejoins on, not the height of
    /// the grid. It has to survive a burst between two reads, because losing the
    /// overlap costs a visible gap.
    pub const READ_LINES: u16 = 500;

    /// Starts emulating a pane, unless something already is.
    ///
    /// Called from the attach path rather than when the pane is created: a pane
    /// nobody is looking at is never read, which is what keeps this from being a
    /// poller over the whole tree.
    /// The grid a feed emulates into, for one view.
    ///
    /// In `Lines` the emulator lays logical lines out to what the client can
    /// draw, which is what stops a phone scrolling sideways through a pane laid
    /// out for a desk. In `Screen` the pane's own geometry is the only correct
    /// one, and the client refits.
    pub fn shape(backend: &TerminalBackend, view: PaneView, viewport: Size) -> Size {
        match view {
            PaneView::Lines => viewport,
            PaneView::Screen => backend.default_size(),
        }
    }

    /// Starts emulating a pane, unless something already is with this shape.
    ///
    /// Returning early for a pane already held is what stops a burst of attaches
    /// spawning a poller each. It must not swallow a *changed* shape, though:
    /// the view and the viewport are read once, at spawn, and a feed started in
    /// `Screen` keeps answering `visible` for as long as it lives. That is what
    /// made the view toggle re-open the stream and change nothing — the wire
    /// carried the new view and the thing producing frames never saw it.
    pub fn ensure(
        backend: Arc<TerminalBackend>,
        registry: Arc<PaneRegistry>,
        gate: Arc<Semaphore>,
        pane: PaneId,
        view: PaneView,
        viewport: Size,
    ) {
        let wanted = Self::shape(&backend, view, viewport);

        if registry.holds(&pane) {
            if registry.feed_matches(&pane, view, wanted) {
                return;
            }

            // Dropped rather than reshaped. The two views come from different
            // herdr sources, so the delta this feed has been rejoining on does
            // not carry over, and the next attach opens with a snapshot anyway.
            registry.forget(&pane);
        }

        let (io, events, input) = PaneIo::channel(wanted);

        // Dropped deliberately. The emulator's replies to device queries go
        // nowhere for a herdr pane, because the program on the far end is
        // herdr's to answer and has already been answered. The pump uses
        // `try_send` for those, so a closed channel costs a dropped reply and
        // never a stall.
        drop(input);

        registry.adopt(pane.clone(), io, PaneSource::Sampled);
        registry.record_feed(&pane, view, wanted);

        tokio::spawn(Self::pump(backend, registry, gate, pane, view, events));
    }

    async fn pump(
        backend: Arc<TerminalBackend>,
        registry: Arc<PaneRegistry>,
        gate: Arc<Semaphore>,
        pane: PaneId,
        view: PaneView,
        events: tokio::sync::mpsc::Sender<PaneEvent>,
    ) {
        let mut delta = OutputDelta::new();
        let mut idle_since: Option<Instant> = None;

        loop {
            tokio::time::sleep(Self::READ_INTERVAL).await;

            // The registry dropped the pane, so its emulator is gone and there is
            // nothing left to feed.
            let Some(watchers) = registry.watchers(&pane) else {
                return;
            };

            if watchers == 0 {
                let since = idle_since.get_or_insert_with(Instant::now);

                if since.elapsed() >= Self::IDLE_GRACE {
                    registry.forget(&pane);

                    return;
                }

                continue;
            }

            idle_since = None;

            let text = match Self::read(&backend, &gate, &pane, view).await {
                Ok(text) => text,
                // The pane is gone, which is not a read that failed. Without
                // this the loop polls a destroyed pane at four hertz for the
                // life of the process - a warning line and a subprocess call
                // every 250ms - and the client never learns, so it sits on a
                // frozen grid believing it is still attached.
                Err(error) if Self::vanished(&error) => {
                    let _ = events.send(PaneEvent::Closed(CloseReason::PaneGone)).await;
                    registry.forget(&pane);

                    return;
                }
                Err(error) => {
                    tracing::warn!(pane = pane.as_str(), %error, "could not read a pane");

                    continue;
                }
            };

            let bytes = match delta.advance(&text) {
                Advance::Appended(added) if added.is_empty() => continue,
                Advance::Appended(added) => Self::as_stream(&added),
                // Said on screen rather than spliced. Joining two pieces of
                // output that were never adjacent reads correctly and is wrong,
                // which on a terminal is invisible.
                Advance::Jumped => Self::as_stream(&format!(
                    "\r\n\x1b[2m--- output was missed here ---\x1b[0m\r\n{text}"
                )),
            };

            // The receiver is the pump inside the registry, which drains
            // continuously. A full channel means the emulator is behind, and
            // waiting is the right shape for output backpressure.
            if events.send(PaneEvent::Output(bytes)).await.is_err() {
                return;
            }
        }
    }

    /// One read, through the same admission gate every other backend call uses.
    ///
    /// Without the gate a four-hertz loop per attached pane would contend with
    /// the operator's own requests unbounded, and a tree read would start losing
    /// to a terminal nobody is looking at.
    async fn read(
        backend: &Arc<TerminalBackend>,
        gate: &Arc<Semaphore>,
        pane: &PaneId,
        view: PaneView,
    ) -> anyhow::Result<String> {
        let permit = Arc::clone(gate)
            .acquire_owned()
            .await
            .map_err(|_| anyhow::anyhow!("the terminal backend is shutting down"))?;

        let backend = Arc::clone(backend);
        let pane = pane.clone();

        let read = tokio::task::spawn_blocking(move || {
            let outcome = backend.read_screen(&pane, view, Self::READ_LINES);
            drop(permit);

            outcome
        })
        .await??;

        Ok(read)
    }

    /// Whether a failed read means the backend no longer has this pane.
    ///
    /// The distinction the loop above turns on: every other error is "could not
    /// read just now" and is worth retrying, and this one is "there is nothing
    /// left to read" and never will be.
    fn vanished(error: &anyhow::Error) -> bool {
        matches!(
            error.downcast_ref::<BackendError>(),
            Some(BackendError::NotFound { .. })
        )
    }

    /// Line endings a terminal understands.
    ///
    /// herdr returns text with bare newlines. An emulator moves down on `LF` and
    /// only returns to column one on `CR`, so feeding it unchanged draws a
    /// staircase running off the right edge.
    fn as_stream(text: &str) -> Vec<u8> {
        text.replace("\r\n", "\n").replace('\n', "\r\n").into_bytes()
    }
}
