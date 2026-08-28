/// Where a pane's bytes come from.
///
/// Not a backend name. The distinction is what a client can be told honestly: a
/// streamed pane's bytes are the program's own, so the emulator's state *is* the
/// program's state. A sampled pane's screen is re-read on a timer and the
/// difference replayed, so the emulator's state is a reconstruction of it.
///
/// A reconstruction recovers the cells and nothing else. The cursor is the part
/// that matters, because the emulator always has one — it sits wherever the
/// replay last wrote — and reporting that as the program's cursor is a specific
/// position, drawn on a phone, that no read ever observed. Absent is the honest
/// answer, and `CursorState` is already optional on the wire to carry it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneSource {
    /// The pane publishes its own bytes and this process owns the far end.
    Streamed,
    /// The pane's screen is polled and the difference between two reads replayed.
    Sampled,
}

impl PaneSource {
    /// Whether a frame from this source may carry a cursor.
    pub fn observes_cursor(self) -> bool {
        matches!(self, Self::Streamed)
    }
}
