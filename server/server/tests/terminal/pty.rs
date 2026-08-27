use std::sync::Arc;
use std::time::Duration;

use tethera_common::protocol::grid::TerminalGrid;
use tethera_common::protocol::terminal::TerminalFrame;
use tethera_common::structs::terminal::{Size, SplitDirection};
use tethera_common::traits::TerminalBackendTrait;
use tethera_server_lib::protocol::ports::TerminalSession;
use tethera_server_lib::terminal::{PaneRegistry, PtyBackend};

/// Whether this environment can be asked to spawn a real shell in a real pty.
///
/// An opt-out, not an opt-in: these tests are the only proof that the emulator
/// reads real bytes, so they run wherever a console exists. A CI container that
/// has none sets `TETHERA_SKIP_PTY_TESTS=1`, and then the synthetic `drift` suite
/// is what still covers accumulation.
fn pty_available() -> bool {
    match std::env::var("TETHERA_SKIP_PTY_TESTS") {
        Ok(value) => value.is_empty() || value == "0",
        Err(_) => true,
    }
}

fn backend() -> (Arc<PaneRegistry>, PtyBackend) {
    let registry = PaneRegistry::new_shared();
    let backend = PtyBackend::new(
        registry.clone(),
        Size {
            cols: 80,
            rows: 24,
        },
        PtyBackend::default_shell(),
    );

    (registry, backend)
}

// The whole point of the widening: a pane tethera opened produces real bytes and
// the emulator reads them.
#[tokio::test]
async fn a_pty_pane_reaches_the_emulator() {
    if !pty_available() {
        return;
    }

    let (registry, backend) = backend();
    let pane = backend
        .open_pane(
            None,
            None,
            Size {
                cols: 80,
                rows: 24,
            },
        )
        .expect("open");

    assert!(registry.holds(&pane.id), "opening a pane must adopt it");

    let mut session = registry.attach(&pane.id).expect("attach");
    let first = tokio::time::timeout(Duration::from_secs(10), session.next_frame())
        .await
        .expect("a frame within the deadline")
        .expect("a frame");

    assert!(
        matches!(first, TerminalFrame::Snapshot { .. }),
        "got {first:?}"
    );

    backend.close(&pane.id).expect("close");
}

// The verification the handoff asks for, in a form CI can run: a real child
// process, real pty bytes, and the applied grid showing what the command printed.
//
// On Windows this is also the test that says whether panes work at all. ConPTY's
// first output is `CSI 6n` and the child makes no progress until it is answered,
// so a regression in the reply path hangs here rather than printing nothing.
#[tokio::test]
async fn a_pane_runs_a_command_and_its_output_reaches_the_grid() {
    if !pty_available() {
        return;
    }

    let (registry, backend) = backend();
    let pane = backend
        .open_pane(
            None,
            None,
            Size {
                cols: 80,
                rows: 24,
            },
        )
        .expect("open");
    let mut session = registry.attach(&pane.id).expect("attach");
    let mut grid = TerminalGrid::default();

    backend
        .send_text(&pane.id, "echo tethera-pty-marker\r")
        .expect("send");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut seen = false;

    while tokio::time::Instant::now() < deadline && !seen {
        if let Ok(Some(frame)) =
            tokio::time::timeout(Duration::from_millis(500), session.next_frame()).await
        {
            grid.apply(&frame);
            seen = (0..grid.rows()).any(|y| grid.line(y).contains("tethera-pty-marker"));
        }
    }

    backend.close(&pane.id).expect("close");

    assert!(seen, "the command's output never reached the grid");
}

// A pty pane's geometry is chosen by this backend and stays chosen, which is the
// one place spec 10.1's "stable for the pane's life" is true.
#[tokio::test]
async fn a_pty_pane_reports_the_size_it_was_given() {
    if !pty_available() {
        return;
    }

    let (_registry, backend) = backend();
    let size = Size {
        cols: 100,
        rows: 30,
    };
    let pane = backend.open_pane(None, None, size).expect("open");

    assert_eq!(pane.size, size);

    backend.close(&pane.id).expect("close");
}

// There is no layout engine to ask, so refusing is the honest answer and
// `pane_split` is not advertised for this backend.
#[tokio::test]
async fn splitting_a_pty_pane_is_refused() {
    if !pty_available() {
        return;
    }

    let (_registry, backend) = backend();
    let pane = backend
        .open_pane(
            None,
            None,
            Size {
                cols: 80,
                rows: 24,
            },
        )
        .expect("open");

    assert!(backend.split(&pane.id, SplitDirection::Vertical).is_err());

    backend.close(&pane.id).expect("close");
}

// A closed pane leaves the registry, or the map grows with dead panes for the
// life of the process: nothing else removes them, because a detach is noticed
// only when a client's connection closes.
#[tokio::test]
async fn closing_a_pane_ends_its_emulation() {
    if !pty_available() {
        return;
    }

    let (registry, backend) = backend();
    let pane = backend
        .open_pane(
            None,
            None,
            Size {
                cols: 80,
                rows: 24,
            },
        )
        .expect("open");

    backend.close(&pane.id).expect("close");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);

    while registry.holds(&pane.id) && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    assert!(
        !registry.holds(&pane.id),
        "a closed pane stayed in the registry"
    );
}

// A pane the backend never opened is not attachable, and a herdr pane is exactly
// that case: real, and unreadable.
#[tokio::test]
async fn a_pane_this_backend_does_not_own_is_not_attachable() {
    if !pty_available() {
        return;
    }

    let (registry, _backend) = backend();
    let absent = tethera_common::structs::ids::PaneId::parse("pn_absent").expect("valid");

    assert!(registry.attach(&absent).is_err());
}

// The tree is flat but it is real: one workspace, one tab per pane.
#[tokio::test]
async fn the_tree_names_every_open_pane() {
    if !pty_available() {
        return;
    }

    let (_registry, backend) = backend();
    let first = backend
        .open_pane(
            None,
            None,
            Size {
                cols: 80,
                rows: 24,
            },
        )
        .expect("open");
    let second = backend
        .open_pane(
            None,
            None,
            Size {
                cols: 80,
                rows: 24,
            },
        )
        .expect("open");

    let tree = backend.tree().expect("tree");
    let (workspaces, tabs, panes) = (tree.workspaces, tree.tabs, tree.panes);

    assert_eq!(workspaces.len(), 1);
    assert_eq!(tabs.len(), 2);
    assert_eq!(panes.len(), 2);

    backend.close(&first.id).expect("close");
    backend.close(&second.id).expect("close");
}

// Watching a real pane, in the form the handoff asks for and CI can repeat.
//
// A real interactive shell in a real pty, driven for long enough to scroll its
// own prompt off the top, with every frame applied to the reference grid. The
// escape traffic is whatever the shell and ConPTY actually emit rather than
// anything this test authored, which is the point: drift that accumulates is the
// failure mode here, and a single-frame test cannot find it.
//
// The check at the end is the one that matters. A second attach opens with a
// fresh full snapshot of the emulator's current screen, so a grid built only from
// the incremental stream and a grid built from that snapshot must agree cell for
// cell. They can only disagree if a frame described something the emulator did
// not have.
#[tokio::test]
async fn a_real_shell_driven_over_time_leaves_no_drift() {
    if !pty_available() {
        return;
    }

    let (registry, backend) = backend();
    let size = Size {
        cols: 80,
        rows: 24,
    };
    let pane = backend.open_pane(None, None, size).expect("open");
    let mut session = registry.attach(&pane.id).expect("attach");
    let mut incremental = TerminalGrid::default();

    // Enough commands to scroll the screen several times over, so lines really
    // leave the top and the prompt is redrawn repeatedly.
    for index in 0..60 {
        backend
            .send_text(&pane.id, &format!("echo tethera-line-{index}\r"))
            .expect("send");

        // Drain whatever the shell produced before typing again, so this drives
        // the pane over time rather than dumping 60 lines into one frame.
        while let Ok(Some(frame)) =
            tokio::time::timeout(Duration::from_millis(40), session.next_frame()).await
        {
            incremental.apply(&frame);
        }
    }

    // Let the last command finish and settle.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);

    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(300), session.next_frame()).await {
            Ok(Some(frame)) => incremental.apply(&frame),
            Ok(None) => break,
            Err(_) => break,
        }
    }

    assert!(
        incremental.rows() == size.rows && incremental.cols() == size.cols,
        "the grid was never sized: {}x{}",
        incremental.cols(),
        incremental.rows()
    );

    // Real output has to have reached the screen, or this test proves nothing
    // about drift.
    let scrollback = registry
        .scrollback(&pane.id, None, 500)
        .expect("scrollback")
        .1;
    let seen_on_screen = (0..size.rows).any(|y| incremental.line(y).contains("tethera-line-"));
    let seen_in_history = scrollback.iter().any(|row| {
        row.spans
            .iter()
            .any(|span| span.text.contains("tethera-line-"))
    });

    assert!(
        seen_on_screen || seen_in_history,
        "no command output reached the pane at all"
    );

    // A second attach opens with a fresh snapshot of the same emulator.
    let mut fresh_session = registry.attach(&pane.id).expect("second attach");
    let snapshot = tokio::time::timeout(Duration::from_secs(5), fresh_session.next_frame())
        .await
        .expect("a snapshot within the deadline")
        .expect("a snapshot");
    let mut fresh = TerminalGrid::default();
    fresh.apply(&snapshot);

    for y in 0..size.rows {
        assert_eq!(
            incremental.line(y),
            fresh.line(y),
            "row {y} drifted between the incremental stream and a fresh snapshot"
        );
    }

    for y in 0..size.rows {
        for x in 0..size.cols {
            assert_eq!(
                incremental.cell(x, y).expect("cell").style,
                fresh.cell(x, y).expect("cell").style,
                "the style at {x},{y} drifted"
            );
        }
    }

    backend.close(&pane.id).expect("close");
}

// A pane whose shell exits on its own has to leave the backend's map too, not
// only the registry.
//
// Measured, and the reason this test exists: a pty reader thread stays blocked in
// `read` for as long as the master is alive, and exits once it is dropped. The
// entry in this map owns the master. So an unreaped dead pane leaks an OS thread
// and a ConPTY for the life of the process, counts against `MAX_PANES`, and makes
// `tree` report a pane that is gone.
#[tokio::test]
async fn a_pane_whose_shell_exits_leaves_the_backend_map() {
    if !pty_available() {
        return;
    }

    let (registry, backend) = backend();
    let size = Size {
        cols: 80,
        rows: 24,
    };
    let pane = backend.open_pane(None, None, size).expect("open");

    // The shell exits on its own. Nothing calls `close`.
    backend.send_text(&pane.id, "exit\r").expect("send");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);

    while registry.holds(&pane.id) && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    assert!(
        !registry.holds(&pane.id),
        "the shell exited but the registry still holds the pane"
    );

    // The reap happens on the next read of the map, which is what `tree` does.
    let tree = backend.tree().expect("tree");
    let (tabs, panes) = (tree.tabs, tree.panes);

    assert!(
        panes.is_empty(),
        "a dead pane is still in the backend map: {panes:?}"
    );
    assert!(tabs.is_empty(), "a dead pane's tab survived: {tabs:?}");
}

// The cap must bound panes that are open, not panes that have ever existed, or a
// long session refuses to open anything while holding nothing.
//
// More cycles than `MAX_PANES`, so the last open is the assertion: a version that
// counted panes ever created rather than panes still open fails here. Asserting
// on the map size instead would pass even with the reap deleted, because `close`
// empties the map synchronously.
#[tokio::test]
async fn cycling_more_panes_than_the_cap_still_leaves_room_to_open_one() {
    if !pty_available() {
        return;
    }

    let (registry, backend) = backend();
    let size = Size {
        cols: 40,
        rows: 10,
    };

    for _ in 0..(PtyBackend::MAX_PANES + 4) {
        let pane = backend.open_pane(None, None, size).expect("open");
        backend.close(&pane.id).expect("close");

        assert!(
            !registry.holds(&pane.id),
            "close did not stop the pane being emulated"
        );
    }

    let last = backend
        .open_pane(None, None, size)
        .expect("the cap was exhausted by panes that are no longer open");

    backend.close(&last.id).expect("close");
}

// `Tab.index` is the backend's own ordinal, and an index taken from list position
// renumbers when a tab closes — somebody's `2:build` silently becomes `1:build`.
// Over a `HashMap` it is worse: the order is arbitrary and shifts on rehash.
#[tokio::test]
async fn a_tab_keeps_its_ordinal_when_an_earlier_pane_closes() {
    if !pty_available() {
        return;
    }

    let (_registry, backend) = backend();
    let size = Size {
        cols: 40,
        rows: 10,
    };

    let first = backend.open_pane(None, None, size).expect("open");
    let second = backend.open_pane(None, None, size).expect("open");
    let third = backend.open_pane(None, None, size).expect("open");

    let tabs = backend.tree().expect("tree").tabs;
    let before: Vec<u16> = tabs.iter().map(|tab| tab.index).collect();

    assert_eq!(before, vec![1, 2, 3], "ordinals are not assigned in order");

    backend.close(&first.id).expect("close");

    let tabs = backend.tree().expect("tree").tabs;
    let after: Vec<u16> = tabs.iter().map(|tab| tab.index).collect();

    assert_eq!(
        after,
        vec![2, 3],
        "the surviving tabs were renumbered when an earlier one closed"
    );

    backend.close(&second.id).expect("close");
    backend.close(&third.id).expect("close");
}

// Two reads of an unchanged tree must be equal, or the watcher reports every tab
// as changed on every read and a client redraws its whole tab row.
#[tokio::test]
async fn two_reads_of_an_unchanged_tree_are_identical() {
    if !pty_available() {
        return;
    }

    let (_registry, backend) = backend();
    let size = Size {
        cols: 40,
        rows: 10,
    };

    let panes: Vec<_> = (0..3)
        .map(|_| backend.open_pane(None, None, size).expect("open"))
        .collect();

    let first = backend.tree().expect("tree");
    let second = backend.tree().expect("tree");

    assert_eq!(first.tabs, second.tabs, "the tab list is not stable across reads");
    assert_eq!(first.panes, second.panes, "the pane list is not stable across reads");

    for pane in panes {
        backend.close(&pane.id).expect("close");
    }
}


// A zero dimension reaches `PtySize` unclamped while `Buffer` clamps to one, so
// an unclamped open would report a geometry no part of the stack has.
#[tokio::test]
async fn a_degenerate_requested_size_is_clamped_consistently() {
    if !pty_available() {
        return;
    }

    let (registry, backend) = backend();
    let pane = backend
        .open_pane(None, None, Size { cols: 0, rows: 0 })
        .expect("open");

    assert!(pane.size.cols >= 1 && pane.size.rows >= 1, "{:?}", pane.size);

    let mut session = registry.attach(&pane.id).expect("attach");
    let frame = tokio::time::timeout(Duration::from_secs(10), session.next_frame())
        .await
        .expect("a frame within the deadline")
        .expect("a frame");

    match frame {
        TerminalFrame::Snapshot { cols, rows, .. } => {
            assert_eq!(
                (cols, rows),
                (pane.size.cols, pane.size.rows),
                "the emulator and the reported pane size disagree"
            );
        }
        other => panic!("expected a snapshot, got {other:?}"),
    }

    backend.close(&pane.id).expect("close");
}
