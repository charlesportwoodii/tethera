use std::time::Duration;

use tethera_common::protocol::terminal::{
    CloseReason, Key, Mods, TerminalFrame, TerminalInput,
};
use tethera_common::protocol::grid::TerminalGrid;
use tethera_common::structs::ids::PaneId;
use tethera_common::structs::terminal::Size;
use tethera_server_lib::protocol::live::PaneSession;
use tethera_server_lib::protocol::ports::TerminalSession;
use tethera_server_lib::terminal::{PaneEvent, PaneIo, PaneRegistry, PaneSource};

fn pane() -> PaneId {
    PaneId::parse("pn_live").expect("valid")
}

/// Every await in this file carries a deadline.
///
/// `cargo test` has no per-test timeout, so an unbounded await turns a regression
/// in the wakeup path — the highest-risk area in this module — into a run that
/// hangs with no output, instead of a failure with a name.
async fn deadline<F: std::future::Future>(future: F) -> Result<F::Output, tokio::time::error::Elapsed>
{
    tokio::time::timeout(Duration::from_secs(5), future).await
}

/// Opens a pane on a registry and keeps both ends of its plumbing.
///
/// No fake source and no fake emulator: the real emulator runs, and only the pty
/// is replaced by two channels a test writes to directly.
struct Harness {
    registry: PaneRegistry,
    writer: tokio::sync::mpsc::Sender<PaneEvent>,
    reader: tokio::sync::mpsc::Receiver<Vec<u8>>,
}

impl Harness {
    fn open(size: Size) -> Self {
        Self::open_from(size, PaneSource::Streamed)
    }

    /// A pane a shim is relaying, as `ShimRelay` adopts one.
    fn relayed(size: Size) -> Self {
        Self::open_from(size, PaneSource::Relayed)
    }

    fn open_from(size: Size, source: PaneSource) -> Self {
        let registry = PaneRegistry::new();
        let (io, writer, reader) = PaneIo::channel(size);
        registry.adopt(pane(), io, source);

        Self {
            registry,
            writer,
            reader,
        }
    }

    fn attach(&self) -> PaneSession {
        self.registry.attach(&pane()).expect("attach")
    }

    async fn write(&self, bytes: &[u8]) {
        self.writer
            .send(PaneEvent::Output(bytes.to_vec()))
            .await
            .expect("send");
    }

    /// Waits until the pump has fed everything written so far.
    ///
    /// The event channel is FIFO, so a frame produced by a trailing printable
    /// character proves every byte before it was fed. Polling `next_frame`
    /// without this races the pump: the opening snapshot returns immediately,
    /// before any output has been consumed.
    async fn settle(&self, session: &mut PaneSession) {
        // The opening snapshot is produced without waiting for the pump, so it
        // has to be taken out of the way before a frame means anything.
        let _ = tokio::time::timeout(Duration::from_millis(50), session.next_frame()).await;

        // A cursor move rather than a printable character. It still produces a
        // frame — the cursor moved — so the FIFO argument holds, and it leaves no
        // mark on the screen or in scrollback for a later assertion to trip over.
        self.write(b"\x1b[1;1H").await;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);

        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(100), session.next_frame()).await
            {
                Ok(Some(TerminalFrame::Snapshot { .. })) | Ok(Some(TerminalFrame::Damage { .. })) => {
                    return
                }
                Ok(Some(_)) => continue,
                Ok(None) => return,
                Err(_) => continue,
            }
        }

        panic!("the pump never produced a frame");
    }
}

// The end-to-end suite asserts this, and damage before a snapshot has nothing to
// apply to.
#[tokio::test]
async fn the_first_frame_of_an_attach_is_a_snapshot() {
    let harness = Harness::open(Size {
        cols: 20,
        rows: 4,
    });
    let mut session = harness.attach();

    harness.write(b"hello").await;

    let first = deadline(session.next_frame())
        .await
        .expect("an opening frame")
        .expect("a frame");

    assert!(
        matches!(first, TerminalFrame::Snapshot { .. }),
        "got {first:?}"
    );
}

// A pane nobody adopted has no byte stream on this machine. That is the honest
// answer for a herdr pane, which is real but unreadable.
#[tokio::test]
async fn attaching_a_pane_the_registry_does_not_hold_is_refused() {
    let registry = PaneRegistry::new();
    let other = PaneId::parse("pn_absent").expect("valid");

    assert!(registry.attach(&other).is_err());
}

// Thousands of small writes must merge into tens of frames. The emulator's own
// state is the buffer, so exceeding the budget merges rather than accumulates.
#[tokio::test]
async fn many_small_writes_produce_bounded_frames() {
    let harness = Harness::open(Size {
        cols: 40,
        rows: 10,
    });
    let mut session = harness.attach();

    for index in 0..500u32 {
        harness.write(format!("\r{index:>6}").as_bytes()).await;
    }

    let mut frames = 0;

    while tokio::time::timeout(Duration::from_millis(120), session.next_frame())
        .await
        .ok()
        .flatten()
        .is_some()
    {
        frames += 1;

        if frames > 40 {
            break;
        }
    }

    assert!(frames > 0, "no frames at all");
    assert!(
        frames <= 40,
        "{frames} frames for 500 writes is close to one per write"
    );
}

#[tokio::test]
async fn a_closed_pane_sends_a_reason_and_then_ends_the_stream() {
    let harness = Harness::open(Size { cols: 8, rows: 2 });
    let mut session = harness.attach();

    let _ = deadline(session.next_frame()).await;

    harness
        .writer
        .send(PaneEvent::Closed(CloseReason::Exited))
        .await
        .expect("send");

    let mut last = None;

    for _ in 0..8 {
        match deadline(session.next_frame()).await.expect("a frame or an end") {
            Some(frame) => last = Some(frame),
            None => break,
        }
    }

    assert!(
        matches!(
            last,
            Some(TerminalFrame::Closed {
                reason: CloseReason::Exited
            })
        ),
        "got {last:?}"
    );
    assert!(deadline(session.next_frame())
        .await
        .expect("the stream to end")
        .is_none());
}

// A pane that stopped producing output has not stopped existing, so an idle
// session must stay pending rather than end the stream.
#[tokio::test]
async fn an_idle_pane_does_not_end_the_stream() {
    let harness = Harness::open(Size { cols: 8, rows: 2 });
    let mut session = harness.attach();

    let _ = deadline(session.next_frame()).await;

    assert!(
        tokio::time::timeout(Duration::from_millis(150), session.next_frame())
            .await
            .is_err(),
        "an idle pane ended the stream"
    );
}

#[tokio::test]
async fn input_reaches_the_pane_as_encoded_bytes() {
    let mut harness = Harness::open(Size { cols: 8, rows: 2 });
    let mut session = harness.attach();

    session
        .send_input(TerminalInput::Key {
            key: Key::Char('c'),
            mods: Mods::CTRL,
        })
        .await
        .expect("send");

    let sent = deadline(harness.reader.recv()).await.expect("input to arrive");

    assert_eq!(sent, Some(vec![0x03]));
}

// A program's query has to be answered or it hangs, and ConPTY asks before it
// will run anything. The answer goes back up the same channel as a key press.
#[tokio::test]
async fn a_device_query_is_answered_back_to_the_pane() {
    let mut harness = Harness::open(Size { cols: 8, rows: 4 });
    let mut session = harness.attach();

    harness.write(b"\x1b[6n").await;
    harness.settle(&mut session).await;

    // Wrapped in a deadline so a regression fails instead of hanging the suite:
    // `cargo test` has no per-test timeout.
    let reply = tokio::time::timeout(Duration::from_secs(5), harness.reader.recv())
        .await
        .expect("a reply within the deadline");

    assert_eq!(reply, Some(b"\x1b[1;1R".to_vec()));
}

// An application that set DECCKM expects SS3 arrows, and it set that mode on the
// stream this emulator parsed. The session has to read it from there.
#[tokio::test]
async fn arrow_encoding_follows_the_mode_the_pane_set() {
    let mut harness = Harness::open(Size { cols: 8, rows: 4 });
    let mut session = harness.attach();

    harness.write(b"\x1b[?1h").await;
    harness.settle(&mut session).await;

    session
        .send_input(TerminalInput::Key {
            key: Key::Up,
            mods: Mods::NONE,
        })
        .await
        .expect("send");

    let sent = deadline(harness.reader.recv()).await.expect("input to arrive");

    assert_eq!(sent, Some(b"\x1bOA".to_vec()));
}

// Scrollback outliving an attach is what makes paging it honest: the emulator
// belongs to the pane, not to the connection looking at it.
#[tokio::test]
async fn scrollback_survives_a_detach() {
    let harness = Harness::open(Size { cols: 8, rows: 2 });

    {
        let mut session = harness.attach();
        harness.write(b"a\r\nb\r\nc\r\nd\r\ne\r\nf").await;
        harness.settle(&mut session).await;
    }

    let (_styles, rows, _next, has_earlier) = harness
        .registry
        .scrollback(&pane(), None, 10)
        .expect("scrollback");

    assert!(!rows.is_empty(), "no scrollback after a detach");
    assert!(
        !has_earlier,
        "a page larger than the whole history should report nothing earlier"
    );
}

// Two phones on one pane share the emulator but not the first frame: each needs
// its own snapshot before any damage.
#[tokio::test]
async fn two_attaches_each_open_with_their_own_snapshot() {
    let harness = Harness::open(Size { cols: 8, rows: 2 });
    let mut first = harness.attach();
    let mut second = harness.attach();

    harness.write(b"hi").await;

    assert!(matches!(
        deadline(first.next_frame()).await.expect("a frame"),
        Some(TerminalFrame::Snapshot { .. })
    ));
    assert!(matches!(
        deadline(second.next_frame()).await.expect("a frame"),
        Some(TerminalFrame::Snapshot { .. })
    ));
}

// A second attach must not take the damage the first one was owed.
//
// Both laggard-serving paths build a snapshot, and a snapshot is the whole
// screen, so neither needs the pending damage. Draining it there took it from
// whichever session was in sync, which then received nothing and froze on a
// screen missing that change — and a second overlapping attach is the *reconnect*
// case, because a detach is only noticed at the QUIC idle timeout.
#[tokio::test]
async fn a_second_attach_does_not_steal_the_first_ones_damage() {
    let harness = Harness::open(Size {
        cols: 20,
        rows: 8,
    });
    let mut first = harness.attach();

    // Short enough to fit one 20-column row: a marker that wrapped would be
    // split across two lines and `contains` would never match it.
    let opening = deadline(first.next_frame())
        .await
        .expect("an opening frame")
        .expect("a frame");
    assert!(matches!(opening, TerminalFrame::Snapshot { .. }));

    harness.write(b"second-marker").await;

    // A second session attaches and takes its own opening snapshot, which
    // describes that write.
    let mut second = harness.attach();
    let _ = tokio::time::timeout(Duration::from_secs(5), second.next_frame())
        .await
        .expect("the second session's opening frame");

    // The first session must still learn about the write.
    let frame = tokio::time::timeout(Duration::from_secs(5), first.next_frame())
        .await
        .expect("the first session was never told about the write")
        .expect("a frame");

    let mut grid = TerminalGrid::default();
    grid.apply(&opening);
    grid.apply(&frame);

    assert!(
        (0..8).any(|y| grid.line(y).contains("second-marker")),
        "the first session's screen is missing the write"
    );
}



// The other half of the contract: a pty owns its bytes, so its cursor is the
// program's own and withholding it would break every full-screen program.
#[tokio::test]
async fn a_streamed_pane_still_reports_its_cursor() {
    let harness = Harness::open(Size { cols: 20, rows: 4 });
    let mut session = harness.attach();

    harness.write(b"hello").await;

    let frame = deadline(session.next_frame())
        .await
        .expect("a frame")
        .expect("a frame");

    match frame {
        TerminalFrame::Snapshot { cursor, .. } => {
            assert!(cursor.is_some(), "a streamed pane must report its cursor");
        }
        other => panic!("expected a snapshot, got {other:?}"),
    }
}

// The shim answers this pty's device queries, so the server must not. A second
// reply travels the same downlink and arrives at the shell as though somebody
// had typed it - `[1;1R` appearing at a prompt out of nowhere.
#[tokio::test]
async fn a_relayed_pane_sends_no_device_replies_back_to_its_pane() {
    let mut harness = Harness::relayed(Size { cols: 20, rows: 4 });
    let mut session = harness.attach();

    let _ = deadline(session.next_frame()).await;

    harness.write(b"[6n").await;
    harness.settle(&mut session).await;

    assert!(
        harness.reader.try_recv().is_err(),
        "the server answered a query the shim had already answered"
    );
}

// A pty this process owns is the opposite case: nothing else is positioned to
// answer, and a program that asks and is never told hangs. ConPTY asks before it
// will run anything at all.
#[tokio::test]
async fn a_streamed_pane_answers_a_device_query_itself() {
    let mut harness = Harness::open(Size { cols: 20, rows: 4 });
    let mut session = harness.attach();

    let _ = deadline(session.next_frame()).await;

    harness.write(b"[6n").await;
    harness.settle(&mut session).await;

    let replied = harness.reader.try_recv().expect("a device reply");

    assert!(
        replied.starts_with(b"["),
        "expected a report, got {replied:?}"
    );
}

// Both remaining sources carry the program's own bytes, so both report a cursor.
// The variant that could not was the polled one, and polling is gone.
#[tokio::test]
async fn a_relayed_pane_reports_its_cursor() {
    let harness = Harness::relayed(Size { cols: 20, rows: 4 });
    let mut session = harness.attach();

    harness.write(b"hello").await;

    let frame = deadline(session.next_frame())
        .await
        .expect("a frame")
        .expect("a frame");

    match frame {
        TerminalFrame::Snapshot { cursor, .. } => {
            assert!(cursor.is_some(), "a relayed pane must report its cursor");
        }
        other => panic!("expected a snapshot, got {other:?}"),
    }
}
