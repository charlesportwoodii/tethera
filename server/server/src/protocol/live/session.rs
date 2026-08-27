use std::sync::Arc;

use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::protocol::terminal::{TerminalFrame, TerminalInput};
use tokio::sync::{mpsc, watch};

use crate::protocol::ports::TerminalSession;
use crate::terminal::{FrameBudget, KeyEncoder, PaneEmulator};

/// One attach: frames out of a shared emulator, input into its pane.
///
/// The opening snapshot and the frame budget belong to the session rather than to
/// the emulator, because two phones looking at one pane each need their own first
/// frame and their own rate. The epoch belongs here for the same reason: damage
/// is drained from shared state, so a session has to know whether the damage it
/// is about to be handed is the damage that follows the screen it last saw.
pub struct PaneSession {
    shared: Arc<PaneEmulator>,
    revision: watch::Receiver<u64>,
    input: mpsc::Sender<Vec<u8>>,
    budget: FrameBudget,
    epoch: Option<u64>,
    farewell_sent: bool,
}

impl PaneSession {
    pub fn new(shared: Arc<PaneEmulator>) -> Self {
        let revision = shared.subscribe();
        let input = shared.input();

        Self {
            shared,
            revision,
            input,
            budget: FrameBudget::new(),
            epoch: None,
            farewell_sent: false,
        }
    }
}

impl TerminalSession for PaneSession {
    async fn next_frame(&mut self) -> Option<TerminalFrame> {
        if self.farewell_sent {
            return None;
        }

        // The first frame on an attach is always a snapshot. Damage before one
        // has nothing to apply to.
        let Some(mut epoch) = self.epoch else {
            let (frame, epoch) = self.shared.open();
            self.epoch = Some(epoch);
            self.budget.spent();

            return Some(frame);
        };

        loop {
            self.budget.ready().await;

            let (frame, next) = self.shared.next(epoch);
            epoch = next;
            self.epoch = Some(next);

            if let Some(frame) = frame {
                self.budget.spent();

                return Some(frame);
            }

            if let Some(reason) = self.shared.closed() {
                self.farewell_sent = true;

                return Some(TerminalFrame::Closed { reason });
            }

            // A `watch` receiver remembers the version it has seen, so a change
            // that lands between the check above and this await is still
            // observed, and a cancelled poll does not lose one either. The
            // sender lives in the emulator this session holds, so the only way
            // this errors is a bug.
            if self.revision.changed().await.is_err() {
                return None;
            }
        }
    }

    async fn send_input(&mut self, input: TerminalInput) -> Result<(), WireError> {
        let (application_cursor_keys, bracketed_paste) = self.shared.modes();
        let bytes = KeyEncoder::encode(&input, application_cursor_keys, bracketed_paste);

        // A key this table has no sequence for is dropped rather than sent as an
        // empty write, which the pty would see as nothing anyway.
        if bytes.is_empty() {
            return Ok(());
        }

        self.input
            .send(bytes)
            .await
            .map_err(|_| WireError::NotFound {
                kind: EntityKind::Pane,
            })
    }
}
