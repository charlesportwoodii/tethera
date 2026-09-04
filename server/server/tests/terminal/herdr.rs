use std::sync::Arc;

use tethera_common::protocol::capability::{self, HasCapability};
use tethera_common::structs::terminal::Size;
use tethera_server_lib::backend::TerminalBackend;
use tethera_server_lib::protocol::live::LiveTerminals;
use tethera_server_lib::terminal::{PaneRegistry, PtyBackend};

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
