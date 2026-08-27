use std::sync::Arc;

use tethera_common::protocol::error::WireError;
use tethera_common::protocol::terminal::{TerminalFrame, TerminalInput};
use tethera_common::structs::ids::PaneId;
use tokio::sync::Semaphore;

use crate::backend::{BackendError, TerminalBackend};
use crate::protocol::live::PaneSession;
use crate::protocol::ports::TerminalSession;

/// One attach, over whichever kind of pane it turned out to be.
///
/// Frames come from the same place either way - an emulator in the registry -
/// and only input differs, because a pty takes bytes and herdr takes key names.
/// An enum rather than a trait object, so adding a third backend touches this
/// file and the compiler finds every arm.
pub enum LiveSession {
    /// A pane whose bytes this process owns, so encoded input reaches it
    /// directly.
    Direct(PaneSession),
    /// A pane herdr owns. Frames are emulated from reads; input goes back
    /// through herdr, which takes names.
    Herdr(HerdrSession),
}

impl TerminalSession for LiveSession {
    async fn next_frame(&mut self) -> Option<TerminalFrame> {
        match self {
            Self::Direct(session) => session.next_frame().await,
            Self::Herdr(session) => session.frames.next_frame().await,
        }
    }

    async fn send_input(&mut self, input: TerminalInput) -> Result<(), WireError> {
        match self {
            Self::Direct(session) => session.send_input(input).await,
            Self::Herdr(session) => session.send_input(input).await,
        }
    }
}

/// An attach to a pane herdr owns.
///
/// Reads and writes take different routes on purpose. Frames come from the
/// emulator the read loop feeds, which is shared with every other attach to the
/// same pane; input goes straight to herdr, because the emulator is a reader of
/// this pane rather than its owner and nothing it wrote would reach the program.
pub struct HerdrSession {
    pub(super) frames: PaneSession,
    backend: Arc<TerminalBackend>,
    gate: Arc<Semaphore>,
    pane: PaneId,
}

impl HerdrSession {
    pub fn new(
        frames: PaneSession,
        backend: Arc<TerminalBackend>,
        gate: Arc<Semaphore>,
        pane: PaneId,
    ) -> Self {
        Self {
            frames,
            backend,
            gate,
            pane,
        }
    }

    /// Hands one keypress or one piece of text to herdr.
    ///
    /// `Text` is already sanitised by the time it arrives - printable plus tab,
    /// newlines folded - so it cannot carry an escape sequence for the program
    /// on the far end to interpret. That is a security property of the wire
    /// type and not something this restates.
    ///
    /// A key herdr will not accept is refused by name rather than dropped.
    /// `delete`, `insert`, `home`, `end`, `pageup` and `pagedown` are each
    /// rejected by herdr as an unsupported key, and a keystroke that silently
    /// did nothing is indistinguishable from a machine that has stopped
    /// answering.
    async fn send_input(&mut self, input: TerminalInput) -> Result<(), WireError> {
        let backend = Arc::clone(&self.backend);
        let pane = self.pane.clone();

        let permit = Arc::clone(&self.gate)
            .acquire_owned()
            .await
            .map_err(|_| WireError::Backend {
                message: "the terminal backend is shutting down".to_string(),
            })?;

        let handle = tokio::task::spawn_blocking(move || {
            use tethera_common::traits::TerminalBackendTrait;

            let outcome = match input {
                TerminalInput::Text(text) => backend.send_text(&pane, &text),
                TerminalInput::Key { key, mods } => backend.send_key(&pane, key, mods),
            };

            drop(permit);

            outcome
        });

        match handle.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(BackendError::classify(error)),
            Err(error) => Err(WireError::Backend {
                message: format!("terminal backend task failed: {error}"),
            }),
        }
    }
}
