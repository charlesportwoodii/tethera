use std::sync::Arc;
use std::time::Duration;

use tethera_common::protocol::capability::{self, HasCapability};
use tethera_common::structs::terminal::Size;
use tethera_server_lib::backend::TerminalBackend;
use tethera_server_lib::protocol::live::LiveTerminals;
use tethera_server_lib::terminal::{
    Advance, HerdrSource, OutputDelta, PaneRegistry, PtyBackend,
};

/// The ordinary case: the window did not slide, so the new read extends the old.
#[test]
fn successive_reads_yield_only_the_new_tail() {
    let mut delta = OutputDelta::new();

    assert_eq!(
        delta.advance("one\ntwo\n"),
        Advance::Appended("one\ntwo\n".to_string())
    );
    assert_eq!(
        delta.advance("one\ntwo\nthree\n"),
        Advance::Appended("three\n".to_string())
    );
}

/// The window slid: the oldest lines fell out of the read, and the overlap has
/// to be found rather than assumed to be the whole of the previous read.
#[test]
fn a_slid_window_is_rejoined_at_its_overlap() {
    let mut delta = OutputDelta::new();

    delta.advance("a\nb\nc\nd\ne\nf\ng\nh\ni\n");

    assert_eq!(
        delta.advance("c\nd\ne\nf\ng\nh\ni\nj\n"),
        Advance::Appended("j\n".to_string())
    );
}

#[test]
fn an_unchanged_read_appends_nothing() {
    let mut delta = OutputDelta::new();

    delta.advance("steady\n");

    assert_eq!(delta.advance("steady\n"), Advance::Appended(String::new()));
}

/// More arrived between two reads than the window holds, so the previous tail is
/// gone. Splicing here would produce output that reads correctly and is wrong,
/// which is worse than a visible gap.
#[test]
fn a_lost_overlap_is_reported_rather_than_spliced() {
    let mut delta = OutputDelta::new();

    delta.advance("one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\n");

    assert_eq!(
        delta.advance("ninety\nninety-one\nninety-two\nninety-three\n"),
        Advance::Jumped
    );
}

/// A cleared pane reads as a jump rather than as an append of everything, which
/// is the same answer and reached the honest way: nothing of what was held is
/// still there.
#[test]
fn a_cleared_pane_is_a_jump() {
    let mut delta = OutputDelta::new();

    delta.advance("something\nthat was here\n");

    assert_eq!(delta.advance("$ \n"), Advance::Jumped);
}

/// After a jump the delta resynchronises on the text it was given, so the read
/// after it appends normally instead of jumping for ever.
#[test]
fn a_jump_resynchronises_on_what_it_was_handed() {
    let mut delta = OutputDelta::new();

    delta.advance("old\n");
    assert_eq!(delta.advance("wholly new\n"), Advance::Jumped);

    assert_eq!(
        delta.advance("wholly new\nand more\n"),
        Advance::Appended("and more\n".to_string())
    );
}

/// A terminal repeats itself constantly - blank lines, repeated prompts - so the
/// rejoin must not be fooled by an earlier identical run.
#[test]
fn a_repeated_tail_rejoins_at_the_latest_occurrence() {
    let mut delta = OutputDelta::new();

    delta.advance("$ \n$ \n$ \n");

    assert_eq!(delta.advance("$ \n$ \n$ \nls\n"), Advance::Appended("ls\n".to_string()));
}

/// What each backend advertises.
///
/// Pinned because the advertised set is what a client draws controls from, and
/// the two questions it answers used to be one. While only a pty could attach,
/// "can this be attached to" and "can this be split" were the same flag read two
/// ways; they are opposite answers on the two backends now, and reading one off
/// the other silently stops advertising a split that works.
///
/// Neither case needs herdr installed: the answer comes from which backend is
/// configured, not from whether it is reachable.
#[test]
fn each_backend_advertises_what_it_can_actually_do() {
    let panes = PaneRegistry::new_shared();
    let herdr = LiveTerminals::new_shared(
        Arc::new(TerminalBackend::herdr(
            "herdr-that-is-not-installed".to_string(),
            Size { cols: 120, rows: 40 },
        )),
        Arc::clone(&panes),
    );

    let advertised = herdr.capabilities();

    assert!(advertised.has(capability::TERMINAL_ATTACH));
    assert!(advertised.has(capability::TERMINAL_INPUT));
    // herdr has a layout engine, so a split from a phone is a real pane at the
    // desk. This is half the handoff requirement.
    assert!(advertised.has(capability::PANE_SPLIT));
    // Only herdr can return output with its wrapping removed, which is what the
    // Lines view is laid out from.
    assert!(advertised.has(capability::TERMINAL_LINES_VIEW));

    let registry = PaneRegistry::new_shared();
    let pty = LiveTerminals::new_shared(
        Arc::new(TerminalBackend::pty(
            Arc::clone(&registry),
            Size { cols: 120, rows: 40 },
            PtyBackend::default_shell(),
        )),
        registry,
    );

    let advertised = pty.capabilities();

    assert!(advertised.has(capability::TERMINAL_ATTACH));
    assert!(advertised.has(capability::TERMINAL_INPUT));
    // No layout engine to ask, so `split` refuses and must not be offered.
    assert!(!advertised.has(capability::PANE_SPLIT));
    // A pty's bytes are already wrapped to the pty's width, and nothing records
    // which breaks were autowrap, so there is nothing to un-wrap from.
    assert!(!advertised.has(capability::TERMINAL_LINES_VIEW));
}

/// A workspace this suite made, closed when the guard drops so a failing
/// assertion cannot leave one behind on the operator's desk.
struct Scratch {
    workspace: Option<String>,
}

impl Scratch {
    fn new() -> Self {
        Self { workspace: None }
    }

    fn hold(&mut self, native: String) {
        self.workspace = Some(native);
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Some(workspace) = self.workspace.take() {
            let _ = std::process::Command::new("herdr")
                .args(["workspace", "close", &workspace])
                .output();
        }
    }
}

/// The read loop against a real herdr pane, proved through the client's applier.
///
/// `#[ignore]`: it needs herdr installed with a server running, and it creates
/// and closes a workspace on the operator's own desk. Run explicitly:
///
/// ```text
/// cargo test -j4 --test integration terminal::herdr::a_real -- --ignored --nocapture
/// ```
///
/// This is the only thing that proves the loop works. herdr announces nothing
/// when a pane produces output, so every part of this path - the read, the
/// overlap rejoin, the line endings, the emulator, the frames - is exercised
/// only by watching a real pane change.
#[tokio::test]
#[ignore = "needs a running herdr"]
async fn a_real_herdr_pane_reaches_the_clients_grid() {
    use tethera_common::protocol::grid::TerminalGrid;
    use tethera_common::protocol::terminal::{Key, Mods, TerminalFrame};
    use tethera_common::protocol::view::PaneView;
    use tethera_common::traits::TerminalBackendTrait;
    use tethera_server_lib::backend::herdr::HerdrIds;
    use tethera_server_lib::protocol::ports::TerminalSession;
    use tokio::sync::Semaphore;

    const MARKER: &str = "tethera-herdr-marker";

    let mut scratch = Scratch::new();
    let size = Size { cols: 120, rows: 40 };
    let backend = Arc::new(TerminalBackend::herdr("herdr".to_string(), size));

    let workspace = backend
        .create_workspace("tethera-terminal-verify")
        .expect("herdr created a workspace");
    scratch.hold(
        HerdrIds::native_workspace(&workspace.id)
            .expect("its own id is native")
            .to_string(),
    );

    let pane = backend
        .open_pane(Some(&workspace.id), None, size)
        .expect("herdr opened a pane");

    let registry = PaneRegistry::new_shared();
    let gate = Arc::new(Semaphore::new(4));

    HerdrSource::ensure(
        Arc::clone(&backend),
        Arc::clone(&registry),
        gate,
        pane.id.clone(),
        PaneView::Lines,
        Size { cols: 60, rows: 200 },
    );

    let mut session = registry.attach(&pane.id).expect("a session over an adopted pane");

    // The shell needs a moment to reach a prompt, and the loop needs one read
    // before there is anything to append to.
    tokio::time::sleep(Duration::from_millis(1500)).await;

    backend
        .send_text(&pane.id, &format!("echo {MARKER}"))
        .expect("herdr took the text");
    backend
        .send_key(&pane.id, Key::Enter, Mods::NONE)
        .expect("herdr took the key");

    // Applied with the client's own applier, not the emulator's grid. Asserting
    // on the emulator would only prove it agrees with itself.
    let mut grid = TerminalGrid::default();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let mut seen = false;

    while tokio::time::Instant::now() < deadline && !seen {
        let Ok(Some(frame)) =
            tokio::time::timeout(Duration::from_secs(2), session.next_frame()).await
        else {
            continue;
        };

        if matches!(frame, TerminalFrame::Closed { .. }) {
            break;
        }

        grid.apply(&frame);

        seen = (0..grid.rows()).any(|row| grid.line(row).contains(MARKER));
    }

    backend.close(&pane.id).ok();

    assert!(
        seen,
        "the marker never reached the grid; the read loop is not feeding the emulator"
    );
}

/// A dense, heavily-styled screen must still fit in one frame.
///
/// `FrameCodec::DEFAULT_MAX_FRAME_BYTES` is 64 KiB, and a write past it fails
/// outright and takes the attach with it. So the failure is not a dropped
/// frame: it is a terminal that shows one screen and then goes silent for the
/// life of the session, which is indistinguishable from a pane that stopped.
///
/// The sample is shaped like the output that provoked this - a server log where
/// every line carries four style changes, so four spans a line rather than one -
/// on a grid tall enough to hold the history a phone scrolls.
#[test]
fn a_dense_styled_screen_fits_in_one_frame() {
    use tethera_common::protocol::terminal::TerminalFrame;
    use tethera_server_lib::terminal::{Emulator, FrameBuilder};
    use tethera_transport::frame::FrameCodec;

    let cols = 58;
    let rows = 200;
    let mut emulator = Emulator::new(Size { cols, rows });

    for line in 0..rows * 2 {
        let painted = format!(
            "\x1b[90m[04:36:03.{line:03}]\x1b[0m\x1b[90m[bvc_server_lib::stream::quic]\x1b[0m\x1b[32m[INFO]\x1b[0m \x1b[37mQUIC server started on port {line}\x1b[0m\r\n"
        );

        emulator.feed(painted.as_bytes());
    }

    let frame = FrameBuilder::snapshot(emulator.screen());
    let encoded = postcard::to_allocvec(&frame).expect("a snapshot encodes");

    println!("encoded snapshot: {} bytes of {}", encoded.len(), FrameCodec::DEFAULT_MAX_FRAME_BYTES);

    assert!(
        matches!(frame, TerminalFrame::Snapshot { .. }),
        "expected a snapshot"
    );

    assert!(
        encoded.len() <= FrameCodec::DEFAULT_MAX_FRAME_BYTES,
        "a {cols}x{rows} styled screen encodes to {} bytes, over the {} byte cap; the write fails and the attach dies with it",
        encoded.len(),
        FrameCodec::DEFAULT_MAX_FRAME_BYTES
    );
}
