use tethera_common::protocol::terminal::{attrs, Color, Style, TerminalFrame};
use tethera_common::structs::terminal::Size;
use tethera_server_lib::terminal::{Cell, Emulator, StyleTable};

// The style table is the hot path: this is the frame type that flows continuously
// while a person watches a build, so a style used twice must cost one entry
// rather than one per cell.
#[test]
fn interning_the_same_style_twice_yields_one_entry() {
    let mut table = StyleTable::new();
    let bold = Style {
        fg: Color::Default,
        bg: Color::Default,
        attrs: attrs::BOLD,
    };

    assert_eq!(table.intern(Cell::PLAIN), 0);
    assert_eq!(table.intern(bold), 1);
    assert_eq!(table.intern(Cell::PLAIN), 0);
    assert_eq!(table.len(), 2);
}

// A terminal row is overwhelmingly one style. One span and a one-entry table is
// the difference between usable and not over a relayed connection to a phone.
#[test]
fn a_row_of_one_style_becomes_one_span_and_one_style_entry() {
    let mut emulator = Emulator::new(Size { cols: 20, rows: 1 });
    let _ = emulator.snapshot();
    emulator.feed(b"a plain row of text");

    match emulator.next_frame().expect("a frame") {
        TerminalFrame::Snapshot {
            styles, rows_data, ..
        } => {
            assert_eq!(styles.len(), 1);
            assert_eq!(rows_data.len(), 1);
            assert_eq!(rows_data[0].spans.len(), 1);
        }
        other => panic!("expected a snapshot, got {other:?}"),
    }
}

#[test]
fn a_row_of_two_styles_becomes_two_spans() {
    let mut emulator = Emulator::new(Size { cols: 20, rows: 8 });
    let _ = emulator.snapshot();
    emulator.feed(b"plain\x1b[1mbold\x1b[0m");

    match emulator.next_frame().expect("a frame") {
        TerminalFrame::Damage {
            styles, rows_data, ..
        } => {
            assert_eq!(styles.len(), 2);
            assert_eq!(rows_data[0].spans.len(), 2);
            assert_eq!(rows_data[0].spans[0].text, "plain");
            assert_eq!(rows_data[0].spans[1].text, "bold");
        }
        other => panic!("expected damage, got {other:?}"),
    }
}

// A repaint of most of the screen is smaller as a snapshot than as damage: a
// snapshot trims trailing blanks and drops blank rows, and damage may not.
#[test]
fn a_repaint_of_most_of_the_screen_arrives_as_a_snapshot() {
    let mut emulator = Emulator::new(Size { cols: 8, rows: 4 });
    let _ = emulator.snapshot();
    emulator.feed(b"\x1b[1;1Ha\x1b[2;1Hb\x1b[3;1Hc\x1b[4;1Hd");

    assert!(matches!(
        emulator.next_frame(),
        Some(TerminalFrame::Snapshot { .. })
    ));
}

#[test]
fn a_one_cell_change_arrives_as_damage_covering_one_column() {
    let mut emulator = Emulator::new(Size { cols: 8, rows: 8 });
    let _ = emulator.snapshot();
    emulator.feed(b"\x1b[4;5Hz");

    match emulator.next_frame().expect("a frame") {
        TerminalFrame::Damage { rows_data, .. } => {
            assert_eq!(rows_data.len(), 1);
            assert_eq!(rows_data[0].y, 3);
            assert_eq!(rows_data[0].from_x, 4);
            assert_eq!(rows_data[0].spans[0].text, "z");
        }
        other => panic!("expected damage, got {other:?}"),
    }
}

// A snapshot describes the whole grid, so a blank row is already blank and
// sending it would cost bytes for nothing.
#[test]
fn a_snapshot_omits_a_blank_row_and_trims_a_trailing_blank_run() {
    let mut emulator = Emulator::new(Size { cols: 10, rows: 3 });
    emulator.feed(b"hi");

    match emulator.snapshot() {
        TerminalFrame::Snapshot { rows_data, .. } => {
            assert_eq!(rows_data.len(), 1);
            assert_eq!(rows_data[0].y, 0);
            assert_eq!(rows_data[0].spans[0].text, "hi");
        }
        other => panic!("expected a snapshot, got {other:?}"),
    }
}

// A space carrying a background colour is visible, so trimming it would erase a
// highlight the emulator drew.
#[test]
fn a_snapshot_keeps_a_trailing_space_that_carries_a_background() {
    let mut emulator = Emulator::new(Size { cols: 6, rows: 1 });
    emulator.feed(b"a\x1b[41m  \x1b[0m");

    match emulator.snapshot() {
        TerminalFrame::Snapshot { rows_data, .. } => {
            let width: usize = rows_data[0]
                .spans
                .iter()
                .map(|span| span.text.chars().count())
                .sum();

            assert_eq!(width, 3, "the coloured spaces were trimmed away");
        }
        other => panic!("expected a snapshot, got {other:?}"),
    }
}

// A snapshot answers everything outstanding, so the damage it subsumed must not
// then arrive again as a second frame describing the same cells.
#[test]
fn a_snapshot_consumes_the_damage_it_covers() {
    let mut emulator = Emulator::new(Size { cols: 8, rows: 2 });
    emulator.feed(b"hello");

    let _ = emulator.snapshot();

    assert!(emulator.next_frame().is_none());
}

// Nothing changed means nothing owed. A frame per poll would make an idle pane
// cost the same as a busy one.
#[test]
fn an_idle_emulator_owes_no_frame() {
    let mut emulator = Emulator::new(Size { cols: 8, rows: 2 });
    let _ = emulator.snapshot();

    assert!(emulator.next_frame().is_none());
}
