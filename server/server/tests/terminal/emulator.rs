use tethera_common::protocol::grid::TerminalGrid;
use tethera_common::protocol::terminal::{attrs, Color, TerminalFrame};
use tethera_common::structs::terminal::Size;
use tethera_server_lib::terminal::Emulator;

/// Feeds bytes and applies every frame the emulator offers, in order.
///
/// The assertions are on the *applied* grid rather than the emulator's own,
/// because that is the property that matters: asserting on the emulator's grid
/// would only prove it agrees with itself.
struct Harness {
    emulator: Emulator,
    grid: TerminalGrid,
}

impl Harness {
    fn new(cols: u16, rows: u16) -> Self {
        let mut emulator = Emulator::new(Size { cols, rows });
        let mut grid = TerminalGrid::default();
        grid.apply(&emulator.snapshot());

        Self { emulator, grid }
    }

    fn feed(&mut self, bytes: &[u8]) -> &mut Self {
        self.emulator.feed(bytes);

        while let Some(frame) = self.emulator.next_frame() {
            self.grid.apply(&frame);
        }

        self
    }

    fn line(&self, y: u16) -> String {
        self.grid.line(y)
    }
}

#[test]
fn plain_text_lands_on_the_first_row() {
    let mut harness = Harness::new(10, 2);
    harness.feed(b"hello");

    assert_eq!(harness.line(0), "hello     ");
}

#[test]
fn a_carriage_return_then_a_write_overwrites_from_the_left_edge() {
    let mut harness = Harness::new(10, 2);
    harness.feed(b"hello\rby");

    assert_eq!(harness.line(0), "byllo     ");
}

// A bare line feed moves down without returning to column zero, which is what
// makes a program that emits "\n" without "\r" draw a staircase.
#[test]
fn a_line_feed_moves_down_without_returning_to_column_zero() {
    let mut harness = Harness::new(6, 3);
    harness.feed(b"ab\ncd");

    assert_eq!(harness.line(0), "ab    ");
    assert_eq!(harness.line(1), "  cd  ");
}

#[test]
fn cursor_addressing_writes_where_it_was_sent() {
    let mut harness = Harness::new(8, 3);
    harness.feed(b"\x1b[2;4Hxy");

    assert_eq!(harness.line(1), "   xy   ");
}

// Damage never clears implicitly, so a blanked region has to reach the client as
// spans of spaces. Applying the frames to the reference grid is what proves the
// spaces were actually on the wire rather than inferred.
#[test]
fn erasing_to_the_end_of_a_line_blanks_it_through_a_damage_frame() {
    let mut harness = Harness::new(8, 1);
    harness.feed(b"abcdefgh");
    harness.feed(b"\x1b[1;4H\x1b[K");

    assert_eq!(harness.line(0), "abc     ");
}

#[test]
fn erasing_to_the_start_of_a_line_blanks_through_the_cursor() {
    let mut harness = Harness::new(8, 1);
    harness.feed(b"abcdefgh");
    harness.feed(b"\x1b[1;4H\x1b[1K");

    assert_eq!(harness.line(0), "    efgh");
}

#[test]
fn erasing_the_display_blanks_every_row() {
    let mut harness = Harness::new(4, 2);
    harness.feed(b"ab\r\ncd");
    harness.feed(b"\x1b[2J");

    assert_eq!(harness.line(0), "    ");
    assert_eq!(harness.line(1), "    ");
}

#[test]
fn a_bold_indexed_colour_reaches_the_applied_cell() {
    let mut harness = Harness::new(4, 1);
    harness.feed(b"\x1b[1;31mR\x1b[0mp");

    let red = harness.grid.cell(0, 0).expect("cell");
    assert_eq!(red.style.fg, Color::Indexed(1));
    assert_eq!(red.style.attrs & attrs::BOLD, attrs::BOLD);

    let plain = harness.grid.cell(1, 0).expect("cell");
    assert_eq!(plain.style.fg, Color::Default);
    assert_eq!(plain.style.attrs, attrs::NONE);
}

#[test]
fn a_truecolour_foreground_reaches_the_applied_cell() {
    let mut harness = Harness::new(2, 1);
    harness.feed(b"\x1b[38;2;10;20;30mX");

    assert_eq!(
        harness.grid.cell(0, 0).expect("cell").style.fg,
        Color::Rgb(10, 20, 30)
    );
}

// ConPTY opens a pane with a bare `ESC[m`. Treating an empty parameter list as a
// no-op rather than as SGR 0 would leave the pen wherever it happened to be.
#[test]
fn an_empty_sgr_parameter_list_resets_the_pen() {
    let mut harness = Harness::new(4, 1);
    harness.feed(b"\x1b[31mr\x1b[mp");

    assert_eq!(
        harness.grid.cell(1, 0).expect("cell").style.fg,
        Color::Default
    );
}

// A pane owning the alternate screen genuinely has no scrollback. Reporting zero
// there is the "absent is not zero" failure the predecessor made.
#[test]
fn entering_the_alternate_screen_reports_no_scrollback() {
    let mut emulator = Emulator::new(Size { cols: 8, rows: 2 });
    emulator.feed(b"\x1b[?1049h");

    match emulator.snapshot() {
        TerminalFrame::Snapshot {
            alt_screen,
            scrollback_len,
            ..
        } => {
            assert!(alt_screen);
            assert_eq!(scrollback_len, None);
        }
        other => panic!("expected a snapshot, got {other:?}"),
    }
}

#[test]
fn leaving_the_alternate_screen_restores_the_primary_screen() {
    let mut emulator = Emulator::new(Size { cols: 8, rows: 2 });
    emulator.feed(b"kept\r\n\r\n\x1b[?1049h\x1b[2Jgone\x1b[?1049l");

    let mut grid = TerminalGrid::default();
    grid.apply(&emulator.snapshot());

    match emulator.snapshot() {
        TerminalFrame::Snapshot {
            alt_screen,
            scrollback_len,
            ..
        } => {
            assert!(!alt_screen);
            assert_eq!(scrollback_len, Some(1));
        }
        other => panic!("expected a snapshot, got {other:?}"),
    }

    assert!(
        !grid.line(0).contains("gone"),
        "the alternate screen's content survived into the primary buffer"
    );
}

#[test]
fn scrolling_past_the_top_grows_scrollback() {
    let mut emulator = Emulator::new(Size { cols: 4, rows: 2 });
    emulator.feed(b"a\r\nb\r\nc\r\nd");

    assert_eq!(emulator.screen().scrollback_len(), Some(2));
}

// Bounded on purpose: an unbounded scrollback is a leak with a lookup method.
#[test]
fn scrollback_stops_growing_at_its_cap() {
    let mut emulator = Emulator::new(Size { cols: 4, rows: 2 });

    for _ in 0..2500 {
        emulator.feed(b"x\r\n");
    }

    assert_eq!(emulator.screen().scrollback_len(), Some(2000));
}

// Columns are cell columns and the emitter sends no spacer, so the character
// after a double-width glyph has to arrive two columns right of it.
#[test]
fn a_double_width_glyph_pushes_the_next_character_two_columns() {
    let mut harness = Harness::new(6, 1);
    harness.feed("\u{1f600}ab".as_bytes());

    assert_eq!(harness.grid.cell(0, 0).expect("cell").ch, '\u{1f600}');
    assert_eq!(harness.grid.cell(2, 0).expect("cell").ch, 'a');
    assert_eq!(harness.grid.cell(3, 0).expect("cell").ch, 'b');
}

// A damage run that begins on a wide glyph's second column has to be widened
// back to the glyph, or the applier starts one column late and shifts the rest.
#[test]
fn damage_starting_on_a_continuation_column_still_agrees_with_the_applier() {
    let mut harness = Harness::new(6, 1);
    harness.feed("\u{1f600}ab".as_bytes());
    harness.feed(b"\x1b[1;2Hz");

    assert_eq!(harness.grid.cell(1, 0).expect("cell").ch, 'z');
}

#[test]
fn a_bell_arrives_as_its_own_frame() {
    let mut emulator = Emulator::new(Size { cols: 4, rows: 1 });
    emulator.feed(b"\x07");

    assert!(matches!(emulator.next_frame(), Some(TerminalFrame::Bell)));
}

// ConPTY sends this before it will run anything, and a child that is never
// answered makes no progress at all. On Windows this is whether a pane starts.
#[test]
fn a_cursor_position_query_is_answered() {
    let mut emulator = Emulator::new(Size { cols: 8, rows: 4 });
    emulator.feed(b"\x1b[3;5H\x1b[6n");

    assert_eq!(emulator.take_replies(), b"\x1b[3;5R".to_vec());
}

#[test]
fn a_device_attributes_query_is_answered() {
    let mut emulator = Emulator::new(Size { cols: 8, rows: 4 });
    emulator.feed(b"\x1b[c");

    assert_eq!(emulator.take_replies(), b"\x1b[?62;22c".to_vec());
}

#[test]
fn deleting_and_inserting_characters_shifts_the_row() {
    let mut harness = Harness::new(6, 1);
    harness.feed(b"abcdef");
    harness.feed(b"\x1b[1;2H\x1b[2P");

    assert_eq!(harness.line(0), "adef  ");

    harness.feed(b"\x1b[1;2H\x1b[2@");

    assert_eq!(harness.line(0), "a  def");
}

#[test]
fn a_repeat_sequence_draws_the_previous_character_again() {
    let mut harness = Harness::new(6, 1);
    harness.feed(b"x\x1b[3b");

    assert_eq!(harness.line(0), "xxxx  ");
}

// Autowrap holds the cursor at the last column until the next character arrives.
// Moving off the row early makes every wrapped line lose its first character.
#[test]
fn a_row_filled_to_its_last_column_wraps_only_on_the_next_character() {
    let mut harness = Harness::new(4, 2);
    harness.feed(b"abcd");

    assert_eq!(harness.line(0), "abcd");
    assert_eq!(harness.line(1), "    ");

    harness.feed(b"e");

    assert_eq!(harness.line(0), "abcd");
    assert_eq!(harness.line(1), "e   ");
}

// A hostile or malformed sequence must not panic a server. Every one of these is
// out of range for its parameter.
#[test]
fn out_of_range_sequences_do_not_panic() {
    let mut harness = Harness::new(8, 3);
    harness.feed(b"\x1b[999;999H");
    harness.feed(b"\x1b[99J\x1b[99K");
    harness.feed(b"\x1b[65535P\x1b[65535@\x1b[65535X");
    harness.feed(b"\x1b[65535S\x1b[65535T\x1b[65535L\x1b[65535M");
    harness.feed(b"\x1b[0;0r\x1b[99;1r");
    harness.feed(b"\x1b[38;2;999m\x1b[38;5m\x1b[999m");
    harness.feed(b"\x1b[?99999h\x1b[?99999l");
    harness.feed(b"ok");

    assert_eq!(harness.grid.rows(), 3);
}

// A scroll region confines a line feed to its own band, which is how a
// full-screen program keeps a header and a status line still.
#[test]
fn a_scroll_region_confines_a_line_feed() {
    let mut harness = Harness::new(4, 4);
    harness.feed(b"\x1b[1;1Htop");
    harness.feed(b"\x1b[2;3r");
    harness.feed(b"\x1b[3;1Ha\nb");

    assert_eq!(harness.line(0), "top ");
}

// DECSCUSR. `CursorState` carries a shape, so a program that asks for a bar and
// gets a block is a visible difference the client cannot correct.
#[test]
fn a_cursor_shape_request_reaches_the_frame() {
    use tethera_common::protocol::terminal::CursorShape;

    let mut emulator = Emulator::new(Size { cols: 8, rows: 2 });

    for (parameter, expected) in [
        (b"\x1b[0 q".as_slice(), CursorShape::Block),
        (b"\x1b[4 q".as_slice(), CursorShape::Underline),
        (b"\x1b[6 q".as_slice(), CursorShape::Bar),
    ] {
        emulator.feed(parameter);

        match emulator.snapshot() {
            TerminalFrame::Snapshot { cursor, .. } => {
                assert_eq!(cursor.expect("a cursor").shape, expected);
            }
            other => panic!("expected a snapshot, got {other:?}"),
        }
    }
}

// A hidden cursor is `visible: false`, not an absent cursor: absent means
// "unchanged" on a damage frame, and confusing the two makes a full-screen
// program's cursor reappear.
#[test]
fn hiding_the_cursor_reports_it_invisible_rather_than_absent() {
    let mut emulator = Emulator::new(Size { cols: 8, rows: 2 });
    emulator.feed(b"\x1b[?25l");

    match emulator.snapshot() {
        TerminalFrame::Snapshot { cursor, .. } => {
            assert!(!cursor.expect("a cursor").visible);
        }
        other => panic!("expected a snapshot, got {other:?}"),
    }
}

// A shrink that left the scroll region past the last row would make
// `line_feed`'s equality test unsatisfiable, and the cursor would stick on the
// bottom row forever: every later line overwrites one row and nothing scrolls.
// That is a pane that looks alive and shows one line.
#[test]
fn a_resize_leaves_the_pane_still_scrolling() {
    let mut emulator = Emulator::new(Size { cols: 20, rows: 24 });
    emulator.feed(b"\x1b[1;24r");
    emulator.feed(b"\x1b[24;1Hbottom");

    emulator.resize(Size { cols: 10, rows: 4 });

    let mut grid = TerminalGrid::default();
    grid.apply(&emulator.snapshot());

    for line in 0..12 {
        emulator.feed(format!("line{line}\r\n").as_bytes());

        while let Some(frame) = emulator.next_frame() {
            grid.apply(&frame);
        }
    }

    assert_eq!(grid.rows(), 4);
    assert!(
        emulator.screen().scrollback_len().unwrap_or_default() > 0,
        "nothing reached scrollback, so the pane stopped scrolling"
    );
    assert!(
        grid.line(2).contains("line11") || grid.line(3).contains("line11"),
        "the last line written is not near the bottom: {:?} / {:?}",
        grid.line(2),
        grid.line(3)
    );
}

// A resize must not leave the cursor outside the grid: `Buffer::set` drops an
// out-of-bounds write silently, so the pane would go blank with no error
// anywhere. The cursor is homed first, because a cursor legitimately left at the
// right edge wraps rather than lands, and that is a different behaviour.
#[test]
fn a_write_after_a_shrink_still_lands() {
    let mut emulator = Emulator::new(Size { cols: 40, rows: 10 });
    emulator.feed(b"\x1b[10;40Hx");

    emulator.resize(Size { cols: 8, rows: 2 });
    emulator.feed(b"\x1b[1;1Hafter");

    let mut grid = TerminalGrid::default();
    grid.apply(&emulator.snapshot());

    assert_eq!(grid.line(0), "after   ");
}

// A cursor left past the new right edge has to be reported inside the grid, or a
// client draws it off-screen.
#[test]
fn a_resize_reports_the_cursor_inside_the_new_grid() {
    let mut emulator = Emulator::new(Size { cols: 40, rows: 10 });
    emulator.feed(b"\x1b[10;40Hx");

    emulator.resize(Size { cols: 8, rows: 2 });

    match emulator.snapshot() {
        TerminalFrame::Snapshot { cursor, cols, rows, .. } => {
            let cursor = cursor.expect("a cursor");

            assert!(cursor.x < cols, "cursor x {} is outside {cols}", cursor.x);
            assert!(cursor.y < rows, "cursor y {} is outside {rows}", cursor.y);
        }
        other => panic!("expected a snapshot, got {other:?}"),
    }
}

// An erase whose run starts inside a double-width pair blanks the continuation
// and used to leave the lead behind with nothing after it. The emitter then sent
// one character for a cell claiming two columns, the applier advanced two, and
// every later cell on the row was one column apart — permanently, and invisibly.
//
// `\x1b[K` after a cursor position that lands mid-glyph is ordinary traffic: it
// is what a shell does redrawing a line that contains an emoji.
#[test]
fn an_erase_starting_inside_a_wide_glyph_leaves_no_orphan_lead() {
    let mut harness = Harness::new(6, 4);
    harness.feed("\x1b[1;1H\u{1f600}".as_bytes());
    harness.feed(b"\x1b[1;2H\x1b[K");
    harness.feed(b"\x1b[1;2Hx");

    assert_eq!(
        harness.grid.cell(1, 0).expect("cell").ch,
        'x',
        "the applier put the character somewhere else: {:?}",
        harness.line(0)
    );
}

// The same shape through `ECH`, which erases a count of cells rather than to the
// end of the line.
#[test]
fn erasing_characters_from_inside_a_wide_glyph_leaves_no_orphan_lead() {
    let mut harness = Harness::new(6, 4);
    harness.feed("\x1b[1;1H\u{1f600}ab".as_bytes());
    harness.feed(b"\x1b[1;2H\x1b[1X");
    harness.feed(b"\x1b[1;2Hz");

    assert_eq!(harness.grid.cell(1, 0).expect("cell").ch, 'z');
    assert_eq!(harness.grid.cell(2, 0).expect("cell").ch, 'a');
    assert_eq!(harness.grid.cell(3, 0).expect("cell").ch, 'b');
}

// Erasing the lead half rather than the continuation, which orphans in the other
// direction.
#[test]
fn an_erase_ending_inside_a_wide_glyph_leaves_no_orphan_continuation() {
    let mut harness = Harness::new(8, 4);
    harness.feed("\x1b[1;1Hab\u{1f600}cd".as_bytes());
    harness.feed(b"\x1b[1;1H\x1b[3X");

    assert_eq!(harness.grid.cell(4, 0).expect("cell").ch, 'c');
    assert_eq!(harness.grid.cell(5, 0).expect("cell").ch, 'd');
}


// `1049` clears the alternate screen on entry; `47` and `1047` switch to whatever
// it already held. That is the historical difference between them, and it is the
// reason all three are handled rather than aliased — a program that switches away
// with `47` and back expects its content to survive.
#[test]
fn switching_with_47_preserves_the_alternate_screen() {
    let mut emulator = Emulator::new(Size { cols: 8, rows: 2 });

    emulator.feed(b"\x1b[?47halt-kept");
    emulator.feed(b"\x1b[?47l");
    emulator.feed(b"\x1b[?47h");

    let mut grid = TerminalGrid::default();
    grid.apply(&emulator.snapshot());

    assert!(
        grid.line(0).contains("alt-kept"),
        "the alternate screen was cleared on re-entry: {:?}",
        grid.line(0)
    );
}

#[test]
fn switching_with_1049_clears_the_alternate_screen() {
    let mut emulator = Emulator::new(Size { cols: 8, rows: 2 });

    emulator.feed(b"\x1b[?1049halt-gone");
    emulator.feed(b"\x1b[?1049l");
    emulator.feed(b"\x1b[?1049h");

    let mut grid = TerminalGrid::default();
    grid.apply(&emulator.snapshot());

    assert!(
        !grid.line(0).contains("alt-gone"),
        "1049 did not clear on entry: {:?}",
        grid.line(0)
    );
}

// `IRM`. A program that sets insert mode and then types would otherwise overwrite,
// which corrupts the line it is editing rather than failing visibly.
#[test]
fn insert_mode_shifts_rather_than_overwrites() {
    let mut harness = Harness::new(8, 1);
    harness.feed(b"abcd");
    harness.feed(b"\x1b[1;2H\x1b[4hXY");

    assert_eq!(harness.line(0), "aXYbcd  ");
}

#[test]
fn resetting_insert_mode_restores_overwriting() {
    let mut harness = Harness::new(8, 1);
    harness.feed(b"abcd");
    harness.feed(b"\x1b[1;2H\x1b[4hX\x1b[4lY");

    assert_eq!(harness.line(0), "aXYcd   ");
}

// A column shrink can cut between a double-width glyph's halves. The applier drops
// a glyph with too few columns left, along with the rest of the row's spans, so an
// unrepaired lead loses everything after it.
#[test]
fn a_shrink_that_cuts_a_wide_glyph_does_not_lose_the_row() {
    let mut emulator = Emulator::new(Size { cols: 8, rows: 1 });
    emulator.feed("ab\u{1f600}cd".as_bytes());

    // Cuts between the glyph's two columns, which sat at 2 and 3.
    emulator.resize(Size { cols: 3, rows: 1 });

    let mut grid = TerminalGrid::default();
    grid.apply(&emulator.snapshot());

    assert_eq!(grid.line(0).chars().next(), Some('a'));
    assert_eq!(grid.line(0).chars().nth(1), Some('b'));
}
