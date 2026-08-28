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
    /// The program's own bytes, relayed by a shim in a pane another process owns.
    ///
    /// As faithful as `Streamed` — these are the real bytes, in order — and
    /// different in exactly one way: this process is not the terminal. The shim
    /// is, because the pane it sits in has a ConPTY that will not start a child
    /// until somebody answers its cursor query, and the shim is the only thing
    /// positioned to answer. So the emulator here reads the stream and must not
    /// reply to it.
    Relayed,
    /// The pane's screen is polled and the difference between two reads replayed.
    Sampled,
}

impl PaneSource {
    /// Whether a frame from this source may carry a cursor.
    pub fn observes_cursor(self) -> bool {
        matches!(self, Self::Streamed | Self::Relayed)
    }

    /// Whether this process answers the pane's device queries.
    ///
    /// Exactly one thing may answer a `DSR` or a `DA`. For a pty this process
    /// opened, that is this process. For a relayed pane it is the shim, which
    /// has already replied by the time the bytes arrive here — so replying again
    /// sends a second answer down the same pipe, and the shell receives a
    /// literal `[1;1R` as though somebody had typed it.
    pub fn answers_queries(self) -> bool {
        matches!(self, Self::Streamed)
    }
}
