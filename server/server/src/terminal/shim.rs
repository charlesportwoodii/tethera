use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize};

use crate::terminal::link::{Downlink, ShimLink};

type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;
type SharedMaster = Arc<Mutex<Box<dyn MasterPty + Send>>>;

/// A shell wrapped in a pty, inside somebody else's terminal.
///
/// This is the one place tethera runs *as* a pane's program rather than owning
/// the pane. It opens a pty, spawns the shell in it, and copies both ways
/// between that pty and its own stdio — so the terminal it is running in
/// (herdr's pane) sees exactly what the shell produced, and the shell receives
/// exactly what was typed.
///
/// The point of the arrangement is the byte stream in the middle. herdr owns the
/// pane, the tab, the split and the lifetime, and publishes no per-pane stream;
/// a shim in the pane has the stream by construction, and tethera can read it
/// without taking any of herdr's job away.
///
/// **Nothing here may fail closed.** Once a herdr `default_shell` points at this
/// binary, every new pane on the machine runs it, including panes opened for
/// work that has nothing to do with tethera and panes opened while the tethera
/// server is stopped. A shim that exits on an error it could have survived is a
/// machine with no working terminal, so every failure below either degrades to a
/// plain shell or is ignored.
pub struct Shim;

impl Shim {
    /// Set for the shell, so a shim launched inside a shim runs the shell
    /// directly instead of nesting a second pty.
    pub const MARKER: &'static str = "TETHERA_SHIM";

    /// How often the shim compares its own terminal size against the pty's.
    ///
    /// Polled rather than signalled. `SIGWINCH` does not exist on Windows, and
    /// the console API reports a resize only by being asked, so a poll is the
    /// only form that is the same on both. Four times a second is below what
    /// anybody notices dragging a window and costs one console call.
    const RESIZE_INTERVAL: Duration = Duration::from_millis(250);

    /// Read in chunks this size, matching `PtyPane`.
    const READ_CHUNK: usize = 8192;

    /// Spike instrumentation. Writes to the path in TETHERA_SHIM_TRACE, because
    /// the pane is the one place a message cannot go: stderr there is the thing
    /// being measured.
    fn trace(line: &str) {
        if let Ok(path) = std::env::var("TETHERA_SHIM_TRACE") {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(file, "{line}");
            }
        }
    }

    /// The shell to wrap when nothing named one.
    pub fn default_shell() -> String {
        for named in ["TETHERA_SHIM_SHELL", "SHELL"] {
            if let Ok(shell) = std::env::var(named) {
                if !shell.is_empty() {
                    return shell;
                }
            }
        }

        if cfg!(windows) {
            "powershell.exe".to_string()
        } else {
            "/bin/sh".to_string()
        }
    }

    /// The pane this shim is running in, as herdr told it.
    ///
    /// herdr exports `HERDR_PANE_ID` into every pane it opens, so a shim knows
    /// its own identity with no discovery and no cooperation from whoever
    /// created the pane. That is what lets a pane split by hand at the desk be
    /// as readable as one tethera opened itself.
    pub fn pane_id() -> Option<String> {
        std::env::var("HERDR_PANE_ID").ok().filter(|id| !id.is_empty())
    }

    /// Runs the shell to completion and returns its exit code.
    pub fn run(shell: &str, address: Option<&str>) -> anyhow::Result<i32> {
        let size = Self::own_size();

        let system = portable_pty::native_pty_system();
        let pair = system.openpty(Self::pty_size(size))?;

        let mut command = CommandBuilder::new(shell);

        // Inherited by the shell and by everything it launches, so a shim
        // started inside this shell sees it and declines to nest.
        command.env(Self::MARKER, "1");

        let child = pair.slave.spawn_command(command)?;

        // The slave has to go or the pty never reports the child's exit: this
        // process would still be holding the other end open.
        drop(pair.slave);

        let reader = pair.master.try_clone_reader()?;

        // One writer, shared. The desk's keystrokes, a client's keystrokes and
        // this process's own replies to device queries all go through it, and an
        // escape sequence interleaved with a keystroke would reach the shell as
        // neither.
        let writer: SharedWriter = Arc::new(Mutex::new(pair.master.take_writer()?));
        let master: SharedMaster = Arc::new(Mutex::new(pair.master));

        // Raw mode is what makes the shell interactive. Without it this
        // process's own stdin is line buffered and echoed, so a keystroke does
        // not reach the pty until Enter and arrives twice on screen.
        //
        // Failure is not fatal. A shim whose stdin is a pipe rather than a
        // console has nothing to set, and refusing there would take out the
        // non-interactive case for the sake of tidiness.
        let raw = crossterm::terminal::enable_raw_mode();
        Self::trace(&format!("size={size:?} raw={raw:?}"));
        let raw = raw.is_ok();

        let done = Arc::new(AtomicBool::new(false));

        // Who the pty's geometry belongs to.
        //
        // A handoff, not a shared view. A phone that attaches claims the size
        // and keeps it after it goes away, because somebody who locked their
        // screen mid-command still cares what width that command was laid out
        // for. Typing at the desk takes it back — that *is* the act of retaking
        // the session, and it is distinguishable here because the desk's
        // keystrokes arrive on stdin while a client's arrive on the downlink.
        let claimed = Arc::new(AtomicBool::new(false));

        // Dialled before the copy loops start, so the first bytes the shell
        // produces are already being relayed. A pane whose stream began one
        // prompt late would open on a screen with no prompt on it.
        //
        // Failure is not fatal and is not logged loudly: a stopped server is the
        // ordinary case, and this pane is a working terminal either way.
        //
        // Two channels, in this order. The uplink is what makes the server adopt
        // the pane, so a downlink opened first has nothing to attach to.
        let uplink = match address {
            Some(address) => Self::dial(address, size, "up").map(|(write, _)| write),
            None => None,
        };

        if let (Some(address), true) = (address, uplink.is_some()) {
            if let Some((_, read)) = Self::dial(address, size, "down") {
                Self::spawn_downlink(
                    read,
                    Arc::clone(&writer),
                    Arc::clone(&master),
                    Arc::clone(&claimed),
                    Arc::clone(&done),
                );
            }
        }

        Self::spawn_output(reader, Arc::clone(&writer), uplink, Arc::clone(&done));
        Self::spawn_input(Arc::clone(&writer), Arc::clone(&claimed), Arc::clone(&done));
        Self::spawn_resize(master, Arc::clone(&claimed), Arc::clone(&done), size);

        Self::trace("threads up, waiting on child");

        let code = Self::wait(child);

        Self::trace(&format!("child exited {code}"));

        done.store(true, Ordering::SeqCst);

        if raw {
            let _ = crossterm::terminal::disable_raw_mode();
        }

        Ok(code)
    }

    fn pty_size(size: (u16, u16)) -> PtySize {
        PtySize {
            rows: size.1,
            cols: size.0,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    /// This process's own terminal size, or a conventional default.
    ///
    /// The default matters: a pty opened at 0x0 makes a shell draw nothing at
    /// all, which reads as the shim having hung.
    fn own_size() -> (u16, u16) {
        match crossterm::terminal::size() {
            Ok((cols, rows)) if cols > 0 && rows > 0 => (cols, rows),
            _ => (80, 24),
        }
    }

    /// Opens the channel and announces this pane.
    ///
    /// The size goes in the greeting because this side is the only one that can
    /// read it: the server never sees the pane's terminal, only the bytes laid
    /// out for it.
    fn dial(
        address: &str,
        size: (u16, u16),
        half: &str,
    ) -> Option<(Box<dyn Write + Send>, Box<dyn Read + Send>)> {
        let pane = Self::pane_id()?;

        let (read, mut write) = match ShimLink::dial(address) {
            Ok(pair) => pair,
            Err(error) => {
                Self::trace(&format!("dial failed {error}"));

                return None;
            }
        };

        let hello = format!("{pane} {} {} {half}\n", size.0, size.1);

        if write.write_all(hello.as_bytes()).is_err() || write.flush().is_err() {
            Self::trace("hello failed");

            return None;
        }

        Self::trace(&format!("dialled {address} as {pane} {half}"));

        Some((write, read))
    }

    /// Messages arriving from the server.
    ///
    /// Framed, unlike the uplink, because this direction carries two kinds of
    /// thing. Read header then body rather than by chunk: a stream has no
    /// message boundaries, and treating an arrival as one splits a resize in
    /// half the first time the pipe delivers it in two pieces.
    fn spawn_downlink(
        mut downlink: Box<dyn Read + Send>,
        writer: SharedWriter,
        master: SharedMaster,
        claimed: Arc<AtomicBool>,
        done: Arc<AtomicBool>,
    ) {
        std::thread::spawn(move || {
            let mut header = [0u8; Downlink::HEADER_BYTES];

            while !done.load(Ordering::SeqCst) {
                if downlink.read_exact(&mut header).is_err() {
                    return;
                }

                let Some(length) = Downlink::payload_length(header) else {
                    Self::trace("downlink named an oversize payload");

                    return;
                };

                let mut payload = vec![0u8; length];

                if downlink.read_exact(&mut payload).is_err() {
                    return;
                }

                match Downlink::decode(header[0], &payload) {
                    Some(Downlink::Input(bytes)) => {
                        let Ok(mut held) = writer.lock() else {
                            return;
                        };

                        if held.write_all(&bytes).is_err() || held.flush().is_err() {
                            return;
                        }
                    }
                    Some(Downlink::Resize { cols, rows }) => {
                        claimed.store(true, Ordering::SeqCst);

                        if let Ok(master) = master.lock() {
                            let applied = master.resize(Self::pty_size((cols, rows)));

                            Self::trace(&format!("claimed {cols}x{rows} applied={applied:?}"));
                        }
                    }
                    // Skipped, not fatal. A shim that outlived an upgrade should
                    // ignore a message it does not know rather than tear down a
                    // working pane.
                    None => Self::trace(&format!("downlink tag {} ignored", header[0])),
                }
            }
        });
    }

    /// pty to this process's stdout, which is the pane, and on to the server.
    ///
    /// Never joined, and flushed per chunk. On Windows the ConPTY reader does
    /// not reach EOF when the child exits, so this thread stays blocked for the
    /// life of the process and the child's own exit is what ends the shim.
    fn spawn_output(
        mut reader: Box<dyn Read + Send>,
        writer: SharedWriter,
        mut uplink: Option<Box<dyn Write + Send>>,
        done: Arc<AtomicBool>,
    ) {
        std::thread::spawn(move || {
            let mut buffer = vec![0u8; Self::READ_CHUNK];
            let mut out = std::io::stdout();

            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => return,
                    Ok(read) => {
                        // Answered here rather than forwarded. A ConPTY will not
                        // run its child until something tells it where the cursor
                        // is, and the terminal this shim runs in does not answer
                        // through two nested ConPTYs — measured: the query goes
                        // out and no reply comes back, so the shell never starts.
                        // The shim owns this pty, so the shim answers.
                        if Self::answers(&buffer[..read]) {
                            if let Ok(mut writer) = writer.lock() {
                                let _ = writer.write_all(b"\x1b[1;1R");
                                let _ = writer.flush();
                            }
                        }

                        // The pane first, the server second. The desk is a
                        // display somebody may be looking at, and a blocked or
                        // dead channel must never delay it.
                        if out.write_all(&buffer[..read]).is_err() || out.flush().is_err() {
                            return;
                        }

                        if let Some(channel) = uplink.as_mut() {
                            if channel.write_all(&buffer[..read]).is_err()
                                || channel.flush().is_err()
                            {
                                Self::trace("uplink ended");

                                uplink = None;
                            }
                        }
                    }
                    Err(_) => return,
                }

                if done.load(Ordering::SeqCst) {
                    return;
                }
            }
        });
    }

    /// Whether a chunk from the pty holds a cursor position request.
    fn answers(chunk: &[u8]) -> bool {
        chunk.windows(4).any(|window| window == b"\x1b[6n")
    }

    /// This process's stdin to the pty, which is the desk typing.
    ///
    /// Typing here releases a claim. Only keystrokes do: output from a program
    /// in the pane arrives on the pty rather than here, so an agent working away
    /// for an hour never takes the session back from the phone watching it.
    fn spawn_input(writer: SharedWriter, claimed: Arc<AtomicBool>, done: Arc<AtomicBool>) {
        std::thread::spawn(move || {
            let mut buffer = vec![0u8; Self::READ_CHUNK];
            let mut input = std::io::stdin();

            loop {
                match input.read(&mut buffer) {
                    Ok(0) => return,
                    Ok(read) => {
                        Self::trace(&format!("stdin {read} claimed_was={}", claimed.load(Ordering::SeqCst)));

                        if claimed.swap(false, Ordering::SeqCst) {
                            Self::trace("the desk took the session back");
                        }

                        let Ok(mut held) = writer.lock() else {
                            return;
                        };

                        if held.write_all(&buffer[..read]).is_err() || held.flush().is_err() {
                            return;
                        }
                    }
                    Err(_) => return,
                }

                if done.load(Ordering::SeqCst) {
                    return;
                }
            }
        });
    }

    /// Keeps the pty at the size of whoever owns the session.
    ///
    /// Tracks what was last *applied* rather than what the terminal last
    /// reported. After a claim is released the desk's size has usually not
    /// changed, so comparing against the terminal would find nothing to do and
    /// leave the pty at the phone's width until somebody dragged a window.
    fn spawn_resize(
        master: SharedMaster,
        claimed: Arc<AtomicBool>,
        done: Arc<AtomicBool>,
        initial: (u16, u16),
    ) {
        std::thread::spawn(move || {
            // The last size this loop trusts as the desk's.
            //
            // Remembered rather than re-read on release, because resizing the
            // inner pty changes what this process observes as its *own*
            // terminal size — measured: after claiming 58x30 the shim read its
            // own console as 58x30 while herdr still reported the pane as
            // 66x46. The inner ConPTY's resize reaches the outer console
            // through the forwarded byte stream, so the reading is contaminated
            // for exactly as long as a claim is held.
            let mut desk = initial;
            let mut applied = initial;
            let mut was_claimed = false;

            while !done.load(Ordering::SeqCst) {
                std::thread::sleep(Self::RESIZE_INTERVAL);

                if claimed.load(Ordering::SeqCst) {
                    was_claimed = true;

                    continue;
                }

                // Released. Restore what the desk was before the claim and skip
                // this tick's reading, which is still the claim's geometry.
                if std::mem::replace(&mut was_claimed, false) {
                    let Ok(master) = master.lock() else {
                        return;
                    };

                    if master.resize(Self::pty_size(desk)).is_ok() {
                        Self::trace(&format!("desk took back {}x{}", desk.0, desk.1));

                        applied = desk;
                    }

                    continue;
                }

                let now = Self::own_size();

                if now == applied {
                    continue;
                }

                let Ok(master) = master.lock() else {
                    return;
                };

                if master.resize(Self::pty_size(now)).is_ok() {
                    Self::trace(&format!("desk size {}x{}", now.0, now.1));

                    applied = now;
                    desk = now;
                }
            }
        });
    }

    fn wait(mut child: Box<dyn Child + Send + Sync>) -> i32 {
        match child.wait() {
            Ok(status) => i32::try_from(status.exit_code()).unwrap_or(0),
            Err(_) => 0,
        }
    }
}
