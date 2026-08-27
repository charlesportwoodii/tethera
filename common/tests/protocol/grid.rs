use tethera_common::protocol::grid::TerminalGrid;
use tethera_common::protocol::terminal::{
    attrs, CloseReason, Color, CursorShape, CursorState, RowUpdate, Span, Style, TerminalFrame,
};

fn plain() -> Style {
    Style {
        fg: Color::Default,
        bg: Color::Default,
        attrs: attrs::NONE,
    }
}

fn row(y: u16, from_x: u16, text: &str) -> RowUpdate {
    RowUpdate {
        y,
        from_x,
        spans: vec![Span {
            style: 0,
            text: text.into(),
        }],
    }
}

fn snapshot(cols: u16, rows: u16, rows_data: Vec<RowUpdate>) -> TerminalFrame {
    TerminalFrame::Snapshot {
        cols,
        rows,
        styles: vec![plain()],
        rows_data,
        cursor: Some(CursorState {
            x: 0,
            y: 0,
            visible: true,
            shape: CursorShape::Block,
        }),
        alt_screen: false,
        scrollback_len: None,
    }
}

#[test]
fn a_snapshot_sizes_the_grid_and_writes_its_rows() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(10, 2, vec![row(0, 0, "hello")]));

    assert_eq!(grid.cols(), 10);
    assert_eq!(grid.rows(), 2);
    assert_eq!(grid.line(0), "hello     ");
}

// A snapshot describes the whole grid, so a row absent from it is blank for its
// full width. This is the only frame that clears anything implicitly.
#[test]
fn a_row_absent_from_a_snapshot_is_blank() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(5, 2, vec![row(0, 0, "abc")]));

    assert_eq!(grid.line(1), "     ");
}

// A second snapshot must clear what the first drew, or a reconnect leaves
// stale cells the server no longer believes are on screen.
#[test]
fn a_later_snapshot_clears_what_an_earlier_one_drew() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(6, 1, vec![row(0, 0, "abcdef")]));
    grid.apply(&snapshot(6, 1, vec![row(0, 0, "xy")]));

    assert_eq!(grid.line(0), "xy    ");
}

// A damage frame replaces only the run it names. Cells outside it are unchanged,
// which is what makes a damage frame small.
#[test]
fn damage_replaces_only_the_run_it_names() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(10, 1, vec![row(0, 0, "abcdefghij")]));
    grid.apply(&TerminalFrame::Damage {
        styles: vec![plain()],
        rows_data: vec![row(0, 3, "XY")],
        cursor: None,
    });

    assert_eq!(grid.line(0), "abcXYfghij");
}

// Damage never clears implicitly. To blank a region the server sends spans of
// spaces, so a client that inferred "clear to end of line" would erase content
// the server still believes is on screen.
#[test]
fn damage_does_not_clear_to_the_end_of_the_line() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(10, 1, vec![row(0, 0, "abcdefghij")]));
    grid.apply(&TerminalFrame::Damage {
        styles: vec![plain()],
        rows_data: vec![row(0, 0, "Z")],
        cursor: None,
    });

    assert_eq!(grid.line(0), "Zbcdefghij");
}

// A damage frame carrying no cursor leaves the cursor where it was: absent means
// unchanged, not "move it to the origin".
#[test]
fn damage_without_a_cursor_leaves_the_cursor_alone() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(10, 1, vec![row(0, 0, "abcdefghij")]));
    let before = grid.cursor();

    grid.apply(&TerminalFrame::Damage {
        styles: vec![plain()],
        rows_data: vec![row(0, 0, "Z")],
        cursor: None,
    });

    assert_eq!(grid.cursor(), before);
}

#[test]
fn a_resize_reshapes_the_grid_and_keeps_what_still_fits() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(10, 1, vec![row(0, 0, "abcdefghij")]));
    grid.apply(&TerminalFrame::Resized { cols: 4, rows: 2 });

    assert_eq!(grid.cols(), 4);
    assert_eq!(grid.rows(), 2);
    assert_eq!(grid.line(0), "abcd");
}

#[test]
fn a_style_index_resolves_against_the_frame_it_arrived_in() {
    let mut grid = TerminalGrid::default();
    grid.apply(&TerminalFrame::Snapshot {
        cols: 3,
        rows: 1,
        styles: vec![
            plain(),
            Style {
                fg: Color::Indexed(9),
                bg: Color::Default,
                attrs: attrs::BOLD,
            },
        ],
        rows_data: vec![RowUpdate {
            y: 0,
            from_x: 0,
            spans: vec![Span {
                style: 1,
                text: "abc".into(),
            }],
        }],
        cursor: None,
        alt_screen: false,
        scrollback_len: None,
    });

    let cell = grid.cell(0, 0).expect("cell");

    assert_eq!(cell.style.fg, Color::Indexed(9));
    assert_eq!(cell.style.attrs, attrs::BOLD);
}

// The bytes come from a peer, so a malformed frame must not be able to crash a
// renderer.
#[test]
fn a_run_past_the_right_edge_is_clipped() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(4, 1, vec![row(0, 2, "abcdef")]));

    assert_eq!(grid.line(0), "  ab");
}

#[test]
fn a_row_past_the_bottom_edge_is_ignored() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(4, 1, vec![row(0, 0, "abcd"), row(9, 0, "zzzz")]));

    assert_eq!(grid.line(0), "abcd");
    assert_eq!(grid.rows(), 1);
}

// A frame that indexes past its own style table is malformed. Drawing it plainly
// is better than dropping the row.
#[test]
fn a_style_index_the_frame_did_not_define_falls_back_to_the_default() {
    let mut grid = TerminalGrid::default();
    grid.apply(&TerminalFrame::Snapshot {
        cols: 3,
        rows: 1,
        styles: Vec::new(),
        rows_data: vec![RowUpdate {
            y: 0,
            from_x: 0,
            spans: vec![Span {
                style: 7,
                text: "abc".into(),
            }],
        }],
        cursor: None,
        alt_screen: false,
        scrollback_len: None,
    });

    assert_eq!(grid.line(0), "abc");
    assert_eq!(grid.cell(0, 0).expect("cell").style.fg, Color::Default);
}

// Bell and Closed carry no cells, so applying one must not disturb the grid.
#[test]
fn a_bell_or_a_close_leaves_the_grid_alone() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(4, 1, vec![row(0, 0, "abcd")]));

    grid.apply(&TerminalFrame::Bell);
    grid.apply(&TerminalFrame::Closed {
        reason: CloseReason::Exited,
    });

    assert_eq!(grid.line(0), "abcd");
}

// A cell outside the grid is None rather than a panic or a wrapped index, so a
// renderer can probe freely.
#[test]
fn a_cell_outside_the_grid_is_absent() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(2, 1, vec![row(0, 0, "ab")]));

    assert!(grid.cell(2, 0).is_none());
    assert!(grid.cell(0, 1).is_none());
}

// Columns are cell columns: a double-width glyph occupies two and the sender
// emits no spacer, so the applier has to account for the second column itself.
// Advancing one column per char would put every later cell on the row one column
// left of where the sender has it, and the result reads as an emulator bug.
#[test]
fn a_double_width_glyph_occupies_two_columns() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(6, 1, vec![row(0, 0, "\u{1f600}ab")]));

    assert_eq!(grid.cell(0, 0).expect("cell").ch, '\u{1f600}');
    assert_eq!(grid.cell(2, 0).expect("cell").ch, 'a');
    assert_eq!(grid.cell(3, 0).expect("cell").ch, 'b');
}

// The continuation column carries the glyph's own style, so a renderer drawing
// cell by cell cannot show a stale character through the second half.
#[test]
fn the_continuation_column_of_a_wide_glyph_is_blanked() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(4, 1, vec![row(0, 0, "xxxx")]));
    grid.apply(&TerminalFrame::Damage {
        styles: vec![plain()],
        rows_data: vec![row(0, 0, "\u{1f600}")],
        cursor: None,
    });

    assert_eq!(grid.cell(1, 0).expect("cell").ch, ' ');
    assert_eq!(grid.line(0), "\u{1f600} xx");
}

// A wide glyph with only one column left has nowhere to put its second half. It
// is dropped rather than half-drawn, because half a glyph in the last column
// would shift nothing but would render as a different character.
#[test]
fn a_wide_glyph_straddling_the_right_edge_is_not_written() {
    let mut grid = TerminalGrid::default();
    grid.apply(&snapshot(3, 1, vec![row(0, 0, "ab")]));
    grid.apply(&TerminalFrame::Damage {
        styles: vec![plain()],
        rows_data: vec![RowUpdate {
            y: 0,
            from_x: 2,
            spans: vec![Span {
                style: 0,
                text: "\u{1f600}".into(),
            }],
        }],
        cursor: None,
    });

    assert_eq!(grid.cell(2, 0).expect("cell").ch, ' ');
}
