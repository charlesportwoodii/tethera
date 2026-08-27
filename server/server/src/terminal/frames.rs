use tethera_common::protocol::terminal::{RowUpdate, Span, TerminalFrame};

use crate::terminal::cell::Cell;
use crate::terminal::screen::Screen;
use crate::terminal::styles::StyleTable;

/// Turns screen rows into frames.
///
/// A unit struct: a frame is derived from a screen passed in, and there is no
/// state worth holding between frames.
pub struct FrameBuilder;

impl FrameBuilder {
    pub fn snapshot(screen: &Screen) -> TerminalFrame {
        let buffer = screen.active();
        let mut styles = StyleTable::new();
        let mut rows_data = Vec::new();

        for y in 0..buffer.rows() {
            let cells = buffer.row(y);
            let mut end = cells.len();

            // A row absent from a snapshot is blank for its full width, so a
            // trailing blank run costs nothing to leave out.
            while end > 0 && cells[end - 1].is_blank() {
                end -= 1;
            }

            if end == 0 {
                continue;
            }

            let spans = Self::spans(&cells[..end], &mut styles);

            if !spans.is_empty() {
                rows_data.push(RowUpdate {
                    y,
                    from_x: 0,
                    spans,
                });
            }
        }

        TerminalFrame::Snapshot {
            cols: buffer.cols(),
            rows: buffer.rows(),
            styles: styles.into_vec(),
            rows_data,
            cursor: Some(screen.cursor()),
            alt_screen: screen.alt_screen(),
            scrollback_len: screen.scrollback_len(),
        }
    }

    pub fn damage(screen: &mut Screen) -> Option<TerminalFrame> {
        let cursor_moved = screen.take_cursor_moved();
        let dirty = screen.active_mut().take_dirty();

        if dirty.is_empty() && !cursor_moved {
            return None;
        }

        let mut styles = StyleTable::new();
        let mut rows_data = Vec::new();

        for (y, from_x, to_x) in dirty {
            let cells = screen.active().row(y);
            let last = cells.len().saturating_sub(1);
            let mut from_x = usize::from(from_x).min(last);
            let to_x = usize::from(to_x).min(last);

            // Starting on the continuation column of a double-width glyph would
            // put every later cell on this row one column left of where the
            // emulator has it.
            while from_x > 0 && cells[from_x].is_continuation() {
                from_x -= 1;
            }

            if from_x > to_x {
                continue;
            }

            let spans = Self::spans(&cells[from_x..=to_x], &mut styles);

            if !spans.is_empty() {
                rows_data.push(RowUpdate {
                    y,
                    from_x: u16::try_from(from_x).unwrap_or(u16::MAX),
                    spans,
                });
            }
        }

        if rows_data.is_empty() && !cursor_moved {
            return None;
        }

        Some(TerminalFrame::Damage {
            styles: styles.into_vec(),
            rows_data,
            cursor: if cursor_moved {
                Some(screen.cursor())
            } else {
                None
            },
        })
    }

    /// Consecutive cells of one style become one span.
    ///
    /// The applier re-derives each column from the character's display width, so
    /// this has to emit exactly as many columns as the emulator holds. Two
    /// asymmetries make that more than a copy:
    ///
    /// A continuation column contributes no character, because the glyph before
    /// it already occupies two. An *orphan* continuation — one whose lead is not
    /// in this run — contributes a space instead, or every later cell on the row
    /// shifts one column left.
    ///
    /// An orphan *lead* — a width-2 cell whose continuation was overwritten —
    /// is emitted as a space rather than as its glyph, or the applier advances
    /// two columns where the emulator advanced one. A width-2 cell at the very
    /// end of a run is not an orphan: the applier fills the column after it with
    /// the same style the emulator has there.
    ///
    /// Both are defence in depth. `Buffer` maintains the invariant; this keeps a
    /// slip there costing one glyph rather than a whole row.
    pub(crate) fn spans(cells: &[Cell], styles: &mut StyleTable) -> Vec<Span> {
        let mut spans: Vec<Span> = Vec::new();
        let mut owed = 0usize;

        for cell in cells {
            let ch = if cell.is_continuation() {
                if owed > 0 {
                    owed -= 1;

                    continue;
                }

                ' '
            } else {
                if owed > 0 {
                    Self::blank_last(&mut spans);
                }

                owed = usize::from(cell.width).saturating_sub(1);

                cell.ch
            };

            let index = styles.intern(cell.style);

            match spans.last_mut() {
                Some(span) if span.style == index => span.text.push(ch),
                _ => spans.push(Span {
                    style: index,
                    text: ch.to_string(),
                }),
            }
        }

        spans
    }

    /// Rewrites the character just emitted as a space, keeping its style.
    ///
    /// A space carrying the glyph's own style is exactly what the applier writes
    /// into a continuation column, so this leaves the two in agreement.
    fn blank_last(spans: &mut [Span]) {
        if let Some(span) = spans.last_mut() {
            span.text.pop();
            span.text.push(' ');
        }
    }
}
