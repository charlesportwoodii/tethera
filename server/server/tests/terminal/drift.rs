use tethera_common::protocol::grid::TerminalGrid;
use tethera_common::structs::terminal::Size;
use tethera_server_lib::terminal::Emulator;

/// A deterministic stand-in for a long-running pane.
///
/// A progress bar rewriting one row, a full-screen repaint, SGR churn, an
/// alternate-screen visit, and enough scrolling to evict history. Deterministic
/// because a drift failure has to be reproducible to be fixable.
struct Traffic {
    step: u32,
}

impl Traffic {
    fn new() -> Self {
        Self { step: 0 }
    }

    fn next(&mut self) -> Vec<u8> {
        self.step += 1;
        let step = self.step;

        match step % 10 {
            0 => format!(
                "\x1b[{};1H\x1b[K[{:>3}%] building",
                (step % 20) + 1,
                step % 100
            )
            .into_bytes(),
            1 => format!("\rcompiling crate-{step} \x1b[32mok\x1b[0m").into_bytes(),
            2 => b"\x1b[2J\x1b[H".to_vec(),
            3 => format!("\x1b[38;2;{};7;9mcolour\x1b[0m\r\n", step % 256).into_bytes(),
            4 => format!("line {step}\r\n").into_bytes(),
            5 => b"\x1b[?1049h\x1b[Halt screen\x1b[?1049l".to_vec(),
            6 => format!("\x1b[1;1H\x1b[7mheader\x1b[0m\x1b[{};1H", (step % 20) + 1)
                .into_bytes(),
            // Wide glyphs, and narrow writes landing on top of them. Emoji are
            // width 2 and appear constantly in agent output, and a run of ASCII
            // alone cannot catch an emitter that orphans half a glyph.
            // Deliberately the same row as arm 8 below. An earlier version of
            // these two used `step % 20` and `step % 20` one apart, so the wide
            // glyphs and the narrow writes never met and the whole class of
            // half-glyph bugs went uncovered while the test stayed green.
            7 => format!("\x1b[{};1H\u{1f600}ok\u{1f600}", (step % 18) + 1).into_bytes(),
            // A narrow write and an erase, both landing inside the pair arm 7
            // just drew.
            8 => format!("\x1b[{};2Hz\x1b[{};2H\x1b[K", (step % 18) + 1, (step % 18) + 1)
                .into_bytes(),
            _ => format!("\x1b[{};{}Hx", (step % 24) + 1, (step % 80) + 1).into_bytes(),
        }
    }
}

/// The emulator's own row, spelled the way the applier will spell it.
///
/// A continuation column is a space, because that is what the applier writes into
/// the second half of a double-width glyph. Skipping it here would make the two
/// strings differ by one column per wide glyph and hide real drift behind a
/// bookkeeping difference.
fn emulator_row(emulator: &Emulator, y: u16, cols: u16) -> String {
    (0..cols)
        .map(|x| {
            let cell = emulator.screen().active().cell(x, y);

            if cell.is_continuation() {
                ' '
            } else {
                cell.ch
            }
        })
        .collect()
}

// Drift that accumulates is the failure mode here, and a single-frame test will
// not find it. Applying every frame in order and then comparing cell for cell is
// what proves the emitter and the client's applier stay in agreement over time.
#[test]
fn a_long_stream_leaves_the_applier_agreeing_with_the_emulator() {
    let size = Size {
        cols: 80,
        rows: 24,
    };
    let mut emulator = Emulator::new(size);
    let mut grid = TerminalGrid::default();
    grid.apply(&emulator.snapshot());

    let mut traffic = Traffic::new();
    let mut bytes = 0usize;

    while bytes < 100_000 {
        let chunk = traffic.next();
        bytes += chunk.len();
        emulator.feed(&chunk);

        while let Some(frame) = emulator.next_frame() {
            grid.apply(&frame);
        }
    }

    assert_eq!(grid.cols(), size.cols);
    assert_eq!(grid.rows(), size.rows);

    for y in 0..size.rows {
        let applied = grid.line(y);
        let emulated = emulator_row(&emulator, y, size.cols);

        assert_eq!(
            applied, emulated,
            "row {y} drifted after {bytes} bytes"
        );

        // Styles too: a colour the applier never received is drift the character
        // comparison above cannot see.
        for x in 0..size.cols {
            assert_eq!(
                grid.cell(x, y).expect("cell").style,
                emulator.screen().active().cell(x, y).style,
                "style at {x},{y} drifted after {bytes} bytes"
            );
        }
    }
}

// The same stream reaching the same screen through damage alone. This is where a
// missing space in a blanked region shows up: a snapshot would hide it by
// clearing implicitly, and damage never clears implicitly.
#[test]
fn incremental_frames_reach_the_same_screen_as_a_fresh_snapshot() {
    let size = Size {
        cols: 80,
        rows: 24,
    };
    let mut emulator = Emulator::new(size);
    let mut incremental = TerminalGrid::default();
    incremental.apply(&emulator.snapshot());

    let mut traffic = Traffic::new();

    for _ in 0..600 {
        let chunk = traffic.next();
        emulator.feed(&chunk);

        // `damage_only`, not `next_frame`. The resnapshot rule turns roughly one
        // chunk in ten into a snapshot — `Traffic` arm 2 is `ESC[2J ESC[H`, which
        // dirties every row — and a snapshot clears implicitly, so any damage
        // error would be wiped within ten steps. That is the opposite of what
        // this test claims to prove.
        while let Some(frame) = emulator.damage_only() {
            incremental.apply(&frame);
        }
    }

    let mut fresh = TerminalGrid::default();
    fresh.apply(&emulator.snapshot());

    for y in 0..size.rows {
        assert_eq!(
            incremental.line(y),
            fresh.line(y),
            "row {y} differs between the incremental path and a fresh snapshot"
        );
    }
}

// A pane that scrolls for a long time must not grow without bound, and its
// history must stay readable at the cap rather than becoming empty.
#[test]
fn a_long_scrolling_stream_holds_its_scrollback_bound() {
    let mut emulator = Emulator::new(Size {
        cols: 80,
        rows: 24,
    });

    for step in 0..5_000 {
        emulator.feed(format!("line {step}\r\n").as_bytes());

        while emulator.next_frame().is_some() {}
    }

    let length = emulator.screen().scrollback_len().expect("a length");

    assert_eq!(length, 2000);

    let (page, next, has_earlier) = emulator.screen().scrollback_page(None, 50);

    assert_eq!(page.len(), 50);
    assert!(has_earlier);
    assert_eq!(next, Some(1950));
}
