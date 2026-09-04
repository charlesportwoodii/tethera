use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize};
use tethera_common::structs::terminal::Size;

use crate::terminal::link::{Downlink, ShimLink, Uplink};
use crate::terminal::shim_screen::ShimScreen;

/// One pty writer, shared by everything that types into this pane.
///
/// `pub` because the property that matters — a write lands whole or not at all —
/// is asserted against `Shim::write_whole`, and a test cannot construct the sink
/// without the type.
pub type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;
type SharedMaster = Arc<Mutex<Box<dyn MasterPty + Send>>>;
/// The shim's emulator, shared by the thread reading the pty and the thread that
/// applies a claim: both change what the tracked geometry is.
type SharedScreen = Arc<Mutex<ShimScreen>>;
/// The channel to the server, shared by the output thread and the resize thread.
///
/// Both write framed messages, and a resize landing inside a chunk of output
/// would be read as neither. `None` once it has failed or was never dialled.
type SharedUplink = Arc<Mutex<Option<Box<dyn Write + Send>>>>;

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

    /// The name this binary has to be called to run as a shell.
    ///
    /// One build, two names: the dispatch happens before any argument is parsed,
    /// so the file name is the whole of it. Everything that installs the shim or
    /// judges an installed path reads it from here — a second spelling anywhere
    /// is a `default_shell` that dispatches into the CLI and kills every pane.
    pub const ARGV0: &'static str = "tethera-shim";

    /// How often the shim compares its own terminal size against the pty's.
    ///
    /// Polled rather than signalled. `SIGWINCH` does not exist on Windows, and
    /// the console API reports a resize only by being asked, so a poll is the
    /// only form that is the same on both. Four times a second is below what
    /// anybody notices dragging a window and costs one console call.
    const RESIZE_INTERVAL: Duration = Duration::from_millis(250);

    /// Read in chunks this size, matching `PtyPane`.
    const READ_CHUNK: usize = 8192;

    /// How long the shim waits for its output thread to go quiet after the child
    /// exits.
    ///
    /// `run` ends in `process::exit`, which does not wait for threads. Anything
    /// the pty produced but the output thread has not yet written is lost at that
    /// point, and the last thing a shell writes is usually the thing somebody
    /// wanted to read.
    ///
    /// Waited on rather than joined. On Windows the ConPTY reader does not reach
    /// EOF when the child exits, so joining that thread never returns — the same
    /// reason `PtyPane::spawn_reader` is documented as never joined.
    ///
    /// Insurance rather than a fix for an observed loss: the run that appeared to
    /// lose output was a shell that never started, because nothing had answered
    /// its cursor query. Kept because the race is real and a quarter second at
    /// process exit costs nothing.
    pub const DRAIN_GRACE: Duration = Duration::from_millis(250);
    const DRAIN_POLL: Duration = Duration::from_millis(25);

    /// Writes bytes to the pty under one lock, and says whether it worked.
    ///
    /// The only way anything reaches this pty. The desk types on stdin, a client
    /// types on the downlink, and the shim itself answers device queries — three
    /// writers into one pty, where a byte from one landing inside another's
    /// escape sequence arrives at the shell as neither key.
    ///
    /// An empty write succeeds without taking the lock: the reply path calls
    /// this on every chunk and most chunks ask nothing.
    pub fn write_whole(writer: &SharedWriter, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return true;
        }

        let Ok(mut held) = writer.lock() else {
            return false;
        };

        held.write_all(bytes).is_ok() && held.flush().is_ok()
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
    ///
    /// The pane's own stdout is the sink. `run_capturing` is the same run
    /// against a buffer.
    ///
    /// `args` are the shell's, not this process's.
    ///
    /// herdr does not exec `default_shell` bare — it invokes it the way it
    /// invokes a shell, as `<shell> -NoExit -Command "<prompt integration>"`.
    /// Anything in that slot is handed those arguments and has to give them to
    /// the real shell. Swallowing them means a pane whose prompt integration
    /// never runs; rejecting them means a pane that never starts at all, which
    /// took out every new tab, split and agent start on the machine at once.
    pub fn run(shell: &str, args: &[String], address: Option<&str>) -> anyhow::Result<i32> {
        let out: SharedWriter = Arc::new(Mutex::new(Box::new(std::io::stdout())));

        Self::execute(shell, args, address, out)
    }

    /// `run`, with the pane's output captured and no channel dialled.
    ///
    /// A behavioural seam rather than a test hook: the drain is the behaviour
    /// worth asserting, and `run` reaches it only on the way to `process::exit`,
    /// which a test cannot observe.
    pub fn run_capturing(shell: &str) -> anyhow::Result<Vec<u8>> {
        Self::run_capturing_with(shell, &[])
    }

    /// `run_capturing`, with arguments for the shell.
    pub fn run_capturing_with(shell: &str, args: &[String]) -> anyhow::Result<Vec<u8>> {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let out: SharedWriter = Arc::new(Mutex::new(Box::new(Captured(Arc::clone(&seen)))));

        Self::execute(shell, args, None, out)?;

        let held = seen.lock().map_err(|_| anyhow::anyhow!("the capture lock was poisoned"))?;

        Ok(held.clone())
    }

    fn execute(
        shell: &str,
        args: &[String],
        address: Option<&str>,
        out: SharedWriter,
    ) -> anyhow::Result<i32> {
        let size = Self::own_size();

        let system = portable_pty::native_pty_system();
        let pair = system.openpty(Self::pty_size(size))?;

        let mut command = CommandBuilder::new(shell);

        for arg in args {
            command.arg(arg);
        }

        // Inherited by the shell and by everything it launches, so a shim
        // started inside this shell sees it and declines to nest.
        command.env(Self::MARKER, "1");

        // Set explicitly, because `CommandBuilder` does not inherit it: with no
        // cwd it starts the child in the user's home directory. The pane was
        // opened in a project — herdr's `new_cwd` decided where, and this
        // process is already there — so an unset cwd silently moves every
        // shimmed pane to `~` and the operator's `git status` reports on their
        // home directory.
        if let Ok(cwd) = std::env::current_dir() {
            command.cwd(cwd);
        }

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
        let screen: SharedScreen = Arc::new(Mutex::new(ShimScreen::new(Size {
            cols: size.0,
            rows: size.1,
        })));

        // Raw mode is what makes the shell interactive. Without it this
        // process's own stdin is line buffered and echoed, so a keystroke does
        // not reach the pty until Enter and arrives twice on screen.
        //
        // Failure is not fatal. A shim whose stdin is a pipe rather than a
        // console has nothing to set, and refusing there would take out the
        // non-interactive case for the sake of tidiness.
        let raw = crossterm::terminal::enable_raw_mode();
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

        // Empty until the dialler fills it, and emptied again whenever a write
        // fails. The pane is a working terminal throughout.
        let uplink: SharedUplink = Arc::new(Mutex::new(None));

        if let Some(address) = address {
            Self::spawn_dialler(
                address.to_string(),
                size,
                Arc::clone(&writer),
                Arc::clone(&master),
                Arc::clone(&screen),
                Arc::clone(&uplink),
                Arc::clone(&claimed),
                Arc::clone(&done),
            );
        }

        // Bumped by the output thread on every chunk it writes. The drain below
        // watches it rather than joining the thread, which on Windows would
        // never return.
        let wrote = Arc::new(AtomicU64::new(0));

        Self::spawn_output(
            reader,
            Arc::clone(&writer),
            Arc::clone(&screen),
            out,
            Arc::clone(&wrote),
            Arc::clone(&uplink),
            Arc::clone(&done),
        );
        Self::spawn_input(Arc::clone(&writer), Arc::clone(&claimed), Arc::clone(&done));
        Self::spawn_resize(
            master,
            Arc::clone(&claimed),
            Arc::clone(&uplink),
            Arc::clone(&done),
            size,
        );


        let code = Self::wait(child);


        Self::drain(&wrote);

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

    /// Keeps the pane's channel to the server open, redialling as needed.
    ///
    /// The dial fails routinely rather than exceptionally: the server is often
    /// stopped when a pane opens, and it restarts under panes that are already
    /// running. A one-shot dial would leave such a pane unreadable for its whole
    /// life, which for an agent's pane means the phone can never see the work.
    ///
    /// Both channels or neither. The uplink is what makes the server adopt the
    /// pane, so an uplink without its downlink is a pane that can be watched and
    /// not typed into — worse than one that is simply not there yet, because
    /// nothing about it looks wrong.
    fn spawn_dialler(
        address: String,
        size: (u16, u16),
        writer: SharedWriter,
        master: SharedMaster,
        screen: SharedScreen,
        uplink: SharedUplink,
        claimed: Arc<AtomicBool>,
        done: Arc<AtomicBool>,
    ) {
        std::thread::spawn(move || {
            // A pane with no id cannot announce itself, so there is nothing to
            // retry. `HERDR_PANE_ID` is absent exactly when this is not a herdr
            // pane at all.
            if Self::pane_id().is_none() {
                return;
            }

            loop {
                if done.load(Ordering::SeqCst) {
                    return;
                }

                let empty = uplink.lock().map(|held| held.is_none()).unwrap_or(false);

                if empty {
                    if let Some((write, _)) = Self::dial(&address, size, "up") {
                        match Self::dial(&address, size, "down") {
                            Some((_, read)) => {
                                if let Ok(mut held) = uplink.lock() {
                                    *held = Some(write);
                                }

                                Self::spawn_downlink(
                                    read,
                                    Arc::clone(&writer),
                                    Arc::clone(&master),
                                    Arc::clone(&screen),
                                    Arc::clone(&claimed),
                                    Arc::clone(&done),
                                );
                            }
                            // The uplink is dropped rather than kept. A server
                            // that adopted this pane will forget it when the
                            // channel closes, and the next attempt starts clean.
                            // The uplink is dropped rather than kept. A server that adopted
                            // this pane forgets it when the channel closes, so the
                            // next attempt starts clean.
                            None => {}
                        }
                    }
                }

                std::thread::sleep(ShimLink::REDIAL_INTERVAL);
            }
        });
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
            // A stopped server is the ordinary case. `spawn_dialler` retries, and
            // the pane is a working terminal in the meantime.
            Err(_) => {
                return None;
            }
        };

        let hello = format!("{pane} {} {} {half}\n", size.0, size.1);

        if write.write_all(hello.as_bytes()).is_err() || write.flush().is_err() {

            return None;
        }


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
        screen: SharedScreen,
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

                    return;
                };

                let mut payload = vec![0u8; length];

                if downlink.read_exact(&mut payload).is_err() {
                    return;
                }

                match Downlink::decode(header[0], &payload) {
                    Some(Downlink::Input(bytes)) => {
                        if !Self::write_whole(&writer, &bytes) {
                            return;
                        }
                    }
                    Some(Downlink::Resize { cols, rows }) => {
                        claimed.store(true, Ordering::SeqCst);

                        // A refused resize leaves the pty where it was and the
                        // claim standing. The next frame is laid out for the old
                        // geometry, which the client refits, rather than for one
                        // the pty never took.
                        if let Ok(master) = master.lock() {
                            let _ = master.resize(Self::pty_size((cols, rows)));
                        }

                        // The tracked geometry follows the pty's, or the next
                        // cursor query is answered against the old grid and the
                        // program is told it is somewhere it is not.
                        if let Ok(mut screen) = screen.lock() {
                            screen.resize(Size { cols, rows });
                        }
                    }
                    // Skipped, not fatal. A shim that outlived an upgrade should
                    // ignore a message it does not know rather than tear down a
                    // working pane.
                    None => {}
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
        screen: SharedScreen,
        out: SharedWriter,
        wrote: Arc<AtomicU64>,
        uplink: SharedUplink,
        done: Arc<AtomicBool>,
    ) {
        std::thread::spawn(move || {
            let mut buffer = vec![0u8; Self::READ_CHUNK];

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
                        //
                        // One lock for both answers, because both read and
                        // change the same tracked screen.
                        let (replies, visible) = match screen.lock() {
                            Ok(mut screen) => (
                                screen.observe(&buffer[..read]),
                                screen.forward(&buffer[..read]),
                            ),
                            // A poisoned lock must not silence the pane. The
                            // bytes go out unfiltered, which risks the console
                            // leak this filter exists to stop and is strictly
                            // better than a terminal that stops drawing.
                            Err(_) => (Vec::new(), buffer[..read].to_vec()),
                        };

                        if !Self::write_whole(&writer, &replies) {
                            return;
                        }

                        // The pane first, the server second. The desk is a
                        // display somebody may be looking at, and a blocked or
                        // dead channel must never delay it.
                        //
                        // Filtered here and raw on the uplink: the server is not
                        // a guest in anybody's terminal, and it resizes its
                        // emulator from the claim rather than from the stream.
                        if !Self::write_whole(&out, &visible) {
                            return;
                        }

                        wrote.fetch_add(1, Ordering::SeqCst);

                        Self::report(&uplink, &Uplink::Output(buffer[..read].to_vec()));
                    }
                    Err(_) => return,
                }

                if done.load(Ordering::SeqCst) {
                    return;
                }
            }
        });
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

                        if claimed.swap(false, Ordering::SeqCst) {
                        }

                        if !Self::write_whole(&writer, &buffer[..read]) {
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
    /// Sends one framed message to the server, and forgets the channel if it
    /// has gone.
    ///
    /// A dead channel is not an error here. The pane is a working terminal
    /// whether or not the server is listening, and `spawn_dialler` is what brings
    /// the channel back.
    fn report(uplink: &SharedUplink, message: &Uplink) {
        let Ok(mut held) = uplink.lock() else {
            return;
        };

        let Some(channel) = held.as_mut() else {
            return;
        };

        let encoded = message.encode();

        if channel.write_all(&encoded).is_err() || channel.flush().is_err() {

            *held = None;
        }
    }

    fn spawn_resize(
        master: SharedMaster,
        claimed: Arc<AtomicBool>,
        uplink: SharedUplink,
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

                        applied = desk;
                        Self::report(
                            &uplink,
                            &Uplink::Resized {
                                cols: desk.0,
                                rows: desk.1,
                            },
                        );
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

                    applied = now;
                    desk = now;
                    Self::report(
                        &uplink,
                        &Uplink::Resized {
                            cols: now.0,
                            rows: now.1,
                        },
                    );
                }
            }
        });
    }

    /// Waits for the output thread to stop making progress.
    ///
    /// Two consecutive polls with an unchanged count mean the last chunk has
    /// been written; `DRAIN_GRACE` bounds the wait for a pane still producing
    /// output as it dies, so a wedged pty cannot hold the process open.
    fn drain(wrote: &Arc<AtomicU64>) {
        let deadline = std::time::Instant::now() + Self::DRAIN_GRACE;
        let mut last = wrote.load(Ordering::SeqCst);

        while std::time::Instant::now() < deadline {
            std::thread::sleep(Self::DRAIN_POLL);

            let now = wrote.load(Ordering::SeqCst);

            if now == last {
                return;
            }

            last = now;
        }
    }

    fn wait(mut child: Box<dyn Child + Send + Sync>) -> i32 {
        match child.wait() {
            Ok(status) => i32::try_from(status.exit_code()).unwrap_or(0),
            Err(_) => 0,
        }
    }
}

/// A sink that keeps what was written, for `Shim::run_capturing`.
///
/// The pane's real sink is this process's stdout, which a test cannot read back.
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match self.0.lock() {
            Ok(mut held) => {
                held.extend_from_slice(bytes);

                Ok(bytes.len())
            }
            Err(_) => Err(std::io::Error::other("the capture lock was poisoned")),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
