use std::io::{Read, Write};

use portable_pty::{Child, ChildKiller, CommandBuilder, MasterPty, PtySize};
use tethera_common::protocol::terminal::CloseReason;
use tethera_common::structs::terminal::Size;
use tokio::sync::mpsc;

use crate::terminal::event::PaneEvent;
use crate::terminal::io::PaneIo;

/// One pty, its child, and the threads moving bytes in and out of it.
///
/// `Box<dyn>` throughout because `portable_pty` returns its reader, writer and
/// child that way. That is the crate's own signature, the same exception
/// `MigratorTrait::migrations()` takes.
pub struct PtyPane {
    master: Box<dyn MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    input: mpsc::Sender<Vec<u8>>,
    size: Size,
}

impl PtyPane {
    /// Read in chunks this size. Large enough that a screenful of output is one
    /// or two reads, small enough that a chunk is never a latency spike.
    const READ_CHUNK: usize = 8192;

    pub fn size(&self) -> Size {
        self.size
    }

    /// Opens a pty, spawns the shell in it, and returns the plumbing the registry
    /// adopts.
    pub fn open(shell: &str, cwd: Option<&str>, size: Size) -> anyhow::Result<(Self, PaneIo)> {
        let system = portable_pty::native_pty_system();
        let pair = system.openpty(Self::pty_size(size))?;

        let mut command = CommandBuilder::new(shell);

        if let Some(cwd) = cwd {
            command.cwd(cwd);
        }

        // Declared so a program in the pane picks the escape sequences this
        // emulator actually implements, rather than probing for a terminal it
        // cannot ask about.
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");

        let child = pair.slave.spawn_command(command)?;

        // The slave handle has to go, or the pty never reports the child's exit:
        // this process would still be holding the other end open.
        drop(pair.slave);

        let killer = child.clone_killer();
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        let (io, events, input) = PaneIo::channel(size);
        let sink = io.input.clone();

        Self::spawn_reader(reader, events.clone());
        Self::spawn_writer(writer, input);
        Self::spawn_waiter(child, events);

        Ok((
            Self {
                master: pair.master,
                killer,
                input: sink,
                size,
            },
            io,
        ))
    }

    /// Queues bytes for the pty without blocking.
    ///
    /// `try_send` rather than `blocking_send`, because this runs on whatever
    /// thread a synchronous backend method was called from, and `blocking_send`
    /// panics on a runtime thread. A full input channel is a real `Busy`, not
    /// something to stall a thread for.
    pub fn write(&self, bytes: Vec<u8>) -> anyhow::Result<()> {
        self.input
            .try_send(bytes)
            .map_err(|error| anyhow::anyhow!("pane is not accepting input: {error}"))
    }

    pub fn resize(&mut self, size: Size) -> anyhow::Result<()> {
        self.master.resize(Self::pty_size(size))?;
        self.size = size;

        Ok(())
    }

    /// Ends the child.
    ///
    /// The reported error is logged rather than propagated, because on Windows
    /// `portable-pty` returns `Err` from a kill that worked: measured, a
    /// successful `TerminateProcess` came back as
    /// `Os { code: 0, message: "The operation completed successfully." }` while
    /// the waiter thread observed the real exit. There is nothing in the error to
    /// distinguish that from a genuine failure, so the proof a pane is gone is
    /// its registry entry disappearing when the waiter reports `Closed`, not this
    /// return value.
    pub fn kill(&mut self) -> anyhow::Result<()> {
        if let Err(error) = self.killer.kill() {
            tracing::debug!(%error, "the pty child reported an error while being killed");
        }

        Ok(())
    }

    fn pty_size(size: Size) -> PtySize {
        PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    /// Reads the pty until the channel closes.
    ///
    /// Never joined. On Windows the ConPTY reader does not reach EOF even after
    /// the child exits and the master is dropped, so joining this thread would
    /// hang shutdown. A pane's death is learned from the waiter instead, and the
    /// cost of the thread that stays blocked is why `PtyBackend` caps how many
    /// panes may be open at once.
    fn spawn_reader(mut reader: Box<dyn Read + Send>, events: mpsc::Sender<PaneEvent>) {
        std::thread::spawn(move || {
            let mut buffer = vec![0u8; Self::READ_CHUNK];

            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => return,
                    Ok(read) => {
                        // Blocking, and deliberately so: this is the one place
                        // backpressure from a full channel should reach the pty
                        // rather than becoming memory. Legal here because this is
                        // a plain OS thread, not a runtime thread.
                        if events
                            .blocking_send(PaneEvent::Output(buffer[..read].to_vec()))
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(error) => {
                        tracing::debug!(%error, "pty read ended");

                        return;
                    }
                }
            }
        });
    }

    fn spawn_writer(mut writer: Box<dyn Write + Send>, mut input: mpsc::Receiver<Vec<u8>>) {
        std::thread::spawn(move || {
            while let Some(bytes) = input.blocking_recv() {
                if writer.write_all(&bytes).is_err() || writer.flush().is_err() {
                    return;
                }
            }
        });
    }

    /// The only reliable report that a pane is gone.
    fn spawn_waiter(mut child: Box<dyn Child + Send + Sync>, events: mpsc::Sender<PaneEvent>) {
        std::thread::spawn(move || {
            let _ = child.wait();
            let _ = events.blocking_send(PaneEvent::Closed(CloseReason::Exited));
        });
    }
}
