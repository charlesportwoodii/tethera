use tethera_common::structs::terminal::Size;

use crate::terminal::Emulator;

/// The shim's own emulator, and the only thing that answers its pty.
///
/// The shim is the terminal for the pty it opened. That is not a design
/// preference: a ConPTY will not start its child until something answers its
/// cursor query, and the answer does not come back through the terminal the shim
/// is itself running in — measured, the query goes out and nothing returns, so
/// the shell never starts.
///
/// It answers from a tracked position rather than a constant. A fixed reply is
/// enough to boot a shell and wrong for every query after that: a program that
/// asks mid-draw would be told the cursor is at the origin.
pub struct ShimScreen {
    emulator: Emulator,
    /// A sequence that arrived split across two reads.
    ///
    /// The pipe chooses where a chunk ends, so a filter that only matched inside
    /// one chunk would leak a resize whenever the split landed mid-sequence.
    held: Vec<u8>,
}

impl ShimScreen {
    pub fn new(size: Size) -> Self {
        Self {
            emulator: Emulator::new(size),
            held: Vec::new(),
        }
    }

    /// Bytes safe to hand to the terminal the shim is a guest in.
    ///
    /// Only geometry is removed. `CSI ... t` is XTWINOPS, which asks the
    /// *terminal* to resize itself — correct for a program talking to its own
    /// terminal and wrong here, because the shim's terminal is somebody else's
    /// pane. Measured: a claim of 58x30 left the pane's console at 58x30 after
    /// the shim exited, while herdr went on reporting the pane as 66x46, so the
    /// pane and its console were left disagreeing for whatever ran there next.
    ///
    /// Everything else passes byte for byte. The desk is a display somebody may
    /// be looking at, and dropping a colour or a cursor move would corrupt it.
    pub fn forward(&mut self, chunk: &[u8]) -> Vec<u8> {
        let mut pending = std::mem::take(&mut self.held);
        pending.extend_from_slice(chunk);

        let mut out = Vec::with_capacity(pending.len());
        let mut index = 0;

        while index < pending.len() {
            let byte = pending[index];

            if byte != 0x1b {
                out.push(byte);
                index += 1;

                continue;
            }

            match Self::sequence_end(&pending[index..]) {
                // A whole sequence. Dropped when it is a window operation,
                // copied otherwise.
                Some(length) => {
                    let sequence = &pending[index..index + length];

                    if !Self::is_window_operation(sequence) {
                        out.extend_from_slice(sequence);
                    }

                    index += length;
                }
                // Incomplete. Held rather than emitted, so the rest of it can be
                // judged when it arrives.
                None => {
                    self.held = pending[index..].to_vec();

                    return out;
                }
            }
        }

        out
    }

    /// The length of the escape sequence starting at `bytes[0]`, or `None` when
    /// it is not all here yet.
    ///
    /// `CSI` runs to its first byte in `0x40..=0x7e`. `OSC` runs to `BEL` or
    /// `ESC \`. Anything else is a two-byte escape.
    fn sequence_end(bytes: &[u8]) -> Option<usize> {
        if bytes.len() < 2 {
            return None;
        }

        match bytes[1] {
            b'[' => bytes
                .iter()
                .enumerate()
                .skip(2)
                .find(|(_, byte)| (0x40..=0x7e).contains(*byte))
                .map(|(at, _)| at + 1),
            b']' => {
                for (at, byte) in bytes.iter().enumerate().skip(2) {
                    if *byte == 0x07 {
                        return Some(at + 1);
                    }

                    if *byte == b'\\' && bytes[at - 1] == 0x1b {
                        return Some(at + 1);
                    }
                }

                None
            }
            _ => Some(2),
        }
    }

    fn is_window_operation(sequence: &[u8]) -> bool {
        matches!(sequence, [0x1b, b'[', .., b't'])
    }

    /// Feeds a chunk and returns what must be written back into the pty.
    ///
    /// The replies come from `Screen`, which already implements the `DSR` and
    /// `DA` answers this needs — the same code that makes a pty pane start on
    /// Windows one layer in.
    pub fn observe(&mut self, chunk: &[u8]) -> Vec<u8> {
        self.emulator.feed(chunk);

        self.emulator.take_replies()
    }

    pub fn resize(&mut self, size: Size) {
        self.emulator.resize(size);
    }
}
