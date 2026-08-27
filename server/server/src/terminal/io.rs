use crate::terminal::event::PaneEvent;
use tethera_common::structs::terminal::Size;
use tokio::sync::mpsc;

/// The two halves of one pane's byte plumbing, plus the rect it starts at.
///
/// Channels rather than a trait returning a stream: a test constructs both ends
/// directly, so there is no fake byte source to keep in step with a real one. A
/// closed `input` sender is also how "the pane is gone" reaches `send_input`,
/// which is the error `Attach::serve` already answers with `Closed`.
pub struct PaneIo {
    pub events: mpsc::Receiver<PaneEvent>,
    pub input: mpsc::Sender<Vec<u8>>,
    /// The pane's rect as its backend observes it now.
    ///
    /// Carried here rather than chosen by the emulator. A program in an 89-column
    /// pty lays its output out for 89 columns, so an emulator asserting its own
    /// geometry would apply that output to the wrong grid.
    pub size: Size,
}

impl PaneIo {
    /// Bounded on purpose. An unbounded channel converts backpressure into
    /// memory growth, which surfaces as an out-of-memory kill far from the pane
    /// that caused it.
    pub const EVENT_CAPACITY: usize = 256;
    pub const INPUT_CAPACITY: usize = 64;

    /// The pair, plus the ends a backend or a test keeps.
    pub fn channel(size: Size) -> (Self, mpsc::Sender<PaneEvent>, mpsc::Receiver<Vec<u8>>) {
        let (event_tx, event_rx) = mpsc::channel(Self::EVENT_CAPACITY);
        let (input_tx, input_rx) = mpsc::channel(Self::INPUT_CAPACITY);

        (
            Self {
                events: event_rx,
                input: input_tx,
                size,
            },
            event_tx,
            input_rx,
        )
    }
}
