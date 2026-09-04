use std::io::Write;
use std::sync::{Arc, Mutex};

use tethera_server_lib::terminal::{SharedWriter, Shim};

/// A pty writer that keeps what was written.
///
/// The real one is a pty master. What is under test is not the pty — it is that
/// `write_whole` takes the lock for a whole sequence — so an in-memory sink is
/// the honest instrument, and it needs nothing added to the shim to build one.
struct Tap(Arc<Mutex<Vec<u8>>>);

impl Write for Tap {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(bytes);

        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn tap() -> (SharedWriter, Arc<Mutex<Vec<u8>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let writer: SharedWriter = Arc::new(Mutex::new(Box::new(Tap(Arc::clone(&seen)))));

    (writer, seen)
}

// herdr does not exec `default_shell` bare — it invokes it the way it invokes a
// shell, with its own arguments. Measured: herdr launches a pane's shell as
// `powershell.exe -NoExit -Command "<prompt integration>"`, so a shim in that
// slot is handed `-NoExit -Command ...` and must pass it to the real shell.
//
// Getting this wrong killed every pane on the machine: clap rejected `-NoExit`,
// the process exited before the shell started, and new tabs, splits and agent
// starts all failed at once.
#[test]
#[cfg_attr(
    not(windows),
    ignore = "uses a Windows-only shell; the passthrough itself is portable"
)]
fn arguments_meant_for_the_shell_reach_the_shell() {
    let seen = Shim::run_capturing_with(
        "cmd.exe",
        &["/c".to_string(), "echo passthrough-ok".to_string()],
    )
    .expect("the shim to run a shell with arguments");

    let text = String::from_utf8_lossy(&seen);

    assert!(
        text.contains("passthrough-ok"),
        "the shell never saw its arguments: {text:?}"
    );
}

// A shell runs, produces its output and exits, and the shim ends with it.
//
// Not a test of the drain. The drain guards `process::exit` losing bytes, and
// `run_capturing` returns normally rather than exiting — proven by removing the
// drain and watching this still pass. What it does assert is the whole run: a
// pty opens, the emulator answers the cursor query the shell waits on, the shell
// starts, its output reaches the sink, and the shim returns.
#[test]
#[cfg_attr(
    not(windows),
    ignore = "uses a Windows-only shell; the run itself is portable"
)]
fn a_shell_that_exits_takes_the_shim_with_it_and_its_output_arrives() {
    let seen = Shim::run_capturing("hostname").expect("the shim to run a shell that exits");

    assert!(
        !seen.is_empty(),
        "the shell produced output and none of it reached the sink"
    );
}

// Whole writes, never interleaved halves. The desk types on stdin, a client types
// on the downlink, and the shim answers device queries — three writers into one
// pty, where a byte from one landing inside another's escape sequence arrives at
// the shell as neither key.
//
// Two six-byte sequences with no shared prefix, so any interleaving shows up as a
// chunk that is neither.
#[test]
fn concurrent_writers_never_split_a_sequence() {
    const ROUNDS: usize = 500;
    const UP: &[u8] = b"\x1b[1;5A";
    const DOWN: &[u8] = b"\x1b[1;5B";

    let (writer, seen) = tap();

    let left = std::thread::spawn({
        let writer = Arc::clone(&writer);
        move || {
            for _ in 0..ROUNDS {
                assert!(Shim::write_whole(&writer, UP));
            }
        }
    });
    let right = std::thread::spawn({
        let writer = Arc::clone(&writer);
        move || {
            for _ in 0..ROUNDS {
                assert!(Shim::write_whole(&writer, DOWN));
            }
        }
    });

    left.join().expect("left");
    right.join().expect("right");

    let written = seen.lock().expect("lock").clone();

    assert_eq!(written.len(), ROUNDS * UP.len() * 2);

    for chunk in written.chunks(UP.len()) {
        assert!(
            chunk == UP || chunk == DOWN,
            "a sequence was split: {chunk:?}"
        );
    }
}

// An empty write is a no-op that succeeds. The reply path calls `write_whole` on
// every chunk from the pty and most chunks ask nothing, so a false there would
// tear down the output thread on ordinary output.
#[test]
fn an_empty_write_succeeds_without_touching_the_sink() {
    let (writer, seen) = tap();

    assert!(Shim::write_whole(&writer, b""));
    assert!(seen.lock().expect("lock").is_empty());
}
