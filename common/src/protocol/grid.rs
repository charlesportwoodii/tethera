use crate::protocol::terminal::{attrs, Color, CursorState, Span, Style, TerminalFrame};
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridCell {
    pub ch: char,
    pub style: Style,
}

impl Default for GridCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            style: Style {
                fg: Color::Default,
                bg: Color::Default,
                attrs: attrs::NONE,
            },
        }
    }
}

/// A cell grid, and the one definition of how a `TerminalFrame` applies to it.
///
/// This lives in `common` rather than in the client because the apply rules are
/// normative and easy to get subtly wrong: a client that inferred "damage clears
/// to end of line" would erase content the server still believes is on screen,
/// and the result looks plausible rather than broken. The server's tests and the
/// client's renderer share this.
#[derive(Debug, Clone, Default)]
pub struct TerminalGrid {
    cols: u16,
    rows: u16,
    cells: Vec<GridCell>,
    cursor: Option<CursorState>,
}

impl TerminalGrid {
    pub fn cols(&self) -> u16 {
        self.cols
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }

    pub fn cursor(&self) -> Option<CursorState> {
        self.cursor
    }

    pub fn cell(&self, x: u16, y: u16) -> Option<&GridCell> {
        if x >= self.cols || y >= self.rows {
            return None;
        }

        self.cells
            .get(usize::from(y) * usize::from(self.cols) + usize::from(x))
    }

    /// The text of one row, for tests and for accessibility.
    pub fn line(&self, y: u16) -> String {
        (0..self.cols)
            .map(|x| self.cell(x, y).map(|cell| cell.ch).unwrap_or(' '))
            .collect()
    }

    pub fn apply(&mut self, frame: &TerminalFrame) {
        match frame {
            TerminalFrame::Snapshot {
                cols,
                rows,
                styles,
                rows_data,
                cursor,
                ..
            } => {
                // A snapshot describes the whole grid, so start from blank.
                self.resize(*cols, *rows);
                self.cells
                    .iter_mut()
                    .for_each(|cell| *cell = GridCell::default());

                for row in rows_data {
                    self.write_row(row.y, row.from_x, &row.spans, styles);
                }

                self.cursor = *cursor;
            }
            TerminalFrame::Damage {
                styles,
                rows_data,
                cursor,
            } => {
                // Never clears implicitly. Only the named runs change.
                for row in rows_data {
                    self.write_row(row.y, row.from_x, &row.spans, styles);
                }

                if cursor.is_some() {
                    self.cursor = *cursor;
                }
            }
            TerminalFrame::Resized { cols, rows } => self.resize(*cols, *rows),
            TerminalFrame::Bell | TerminalFrame::Closed { .. } => {}
        }
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        let mut next = vec![GridCell::default(); usize::from(cols) * usize::from(rows)];

        // Keep whatever still fits. A resize is an observation of something the
        // server already did, so discarding the overlap would flash the screen
        // for no reason.
        for y in 0..rows.min(self.rows) {
            for x in 0..cols.min(self.cols) {
                if let Some(cell) = self.cell(x, y) {
                    next[usize::from(y) * usize::from(cols) + usize::from(x)] = *cell;
                }
            }
        }

        self.cols = cols;
        self.rows = rows;
        self.cells = next;
    }

    fn write_row(&mut self, y: u16, from_x: u16, spans: &[Span], styles: &[Style]) {
        // A row past the bottom edge is ignored and a run past the right edge is
        // clipped. The bytes come from a peer, so a malformed frame must not be
        // able to crash a renderer.
        if y >= self.rows {
            return;
        }

        let mut x = from_x;

        for span in spans {
            // A frame that indexes past its own style table is malformed.
            // Drawing it plainly beats dropping the row.
            let style = styles
                .get(usize::from(span.style))
                .copied()
                .unwrap_or(GridCell::default().style);

            for ch in span.text.chars() {
                // Cell columns, not characters. A double-width glyph occupies
                // two and the sender emits no spacer, so the applier steps over
                // the second column itself; advancing one per character would
                // put every later cell on this row one column early, and the
                // result reads as corruption rather than as a bug here.
                let width = u16::try_from(UnicodeWidthChar::width(ch).unwrap_or(0).max(1))
                    .unwrap_or(1);

                // A glyph with fewer columns left than it needs is dropped
                // rather than half-drawn: half of a wide glyph renders as a
                // different character, not as a clipped one.
                if x.saturating_add(width) > self.cols {
                    return;
                }

                let index = usize::from(y) * usize::from(self.cols) + usize::from(x);
                self.cells[index] = GridCell { ch, style };

                for continuation in 1..width {
                    let column = usize::from(y) * usize::from(self.cols)
                        + usize::from(x + continuation);
                    self.cells[column] = GridCell { ch: ' ', style };
                }

                x = x.saturating_add(width);
            }
        }
    }
}
