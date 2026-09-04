/// Where a pane's bytes come from.
///
/// Not a backend name. The distinction is what a client can be told honestly: a
/// streamed pane's bytes are the program's own, so the emulator's state *is* the
/// program's state. A sampled pane's screen is re-read on a timer and the
/// difference replayed, so the emulator's state is a reconstruction of it.
///
/// Both carry the program's own bytes, so the emulator's state is the program's
/// state and its cursor is the program's cursor. What differs is who is the
/// terminal, and therefore who answers when the pty asks a question.
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
}

impl PaneSource {
    /// Whether this process answers the pane's device queries.
    ///
    /// Exactly one thing may answer a `DSR` or a `DA`. For a pty this process
    /// opened, that is this process. For a relayed pane it is the shim, which
    /// has already replied by the time the bytes arrive here — so replying again
    /// sends a second answer down the same pipe, and the shell receives a
    /// literal `\x1b[1;1R` as though somebody had typed it.
    pub fn answers_queries(self) -> bool {
        matches!(self, Self::Streamed)
    }
}
