use crate::terminal::cell::Cell;

/// One screen's cells, and the columns that changed since the last drain.
///
/// Damage is the inclusive minimum and maximum column per row. A per-row flag
/// alone would send a whole row for a one-cell change; per-cell tracking costs
/// more to keep than it saves on the wire. Min and max is the useful middle, and
/// a terminal's writes are overwhelmingly contiguous.
#[derive(Debug, Clone)]
pub struct Buffer {
    cols: u16,
    rows: u16,
    cells: Vec<Cell>,
    dirty: Vec<Option<(u16, u16)>>,
}

impl Buffer {
    pub fn new(cols: u16, rows: u16) -> Self {
        // A zero-column or zero-row grid has no cell to address, and every
        // method below would then have to guard against an empty row. Clamping
        // once here is cheaper than clamping everywhere.
        let cols = cols.max(1);
        let rows = rows.max(1);

        Self {
            cols,
            rows,
            cells: vec![Cell::blank(); usize::from(cols) * usize::from(rows)],
            dirty: vec![None; usize::from(rows)],
        }
    }

    pub fn cols(&self) -> u16 {
        self.cols
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }

    fn index(&self, x: u16, y: u16) -> usize {
        usize::from(y) * usize::from(self.cols) + usize::from(x)
    }

    pub fn row(&self, y: u16) -> &[Cell] {
        let y = y.min(self.rows - 1);
        let start = self.index(0, y);

        &self.cells[start..start + usize::from(self.cols)]
    }

    pub fn cell(&self, x: u16, y: u16) -> Cell {
        if x >= self.cols || y >= self.rows {
            return Cell::blank();
        }

        self.cells[self.index(x, y)]
    }

    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if x >= self.cols || y >= self.rows {
            return;
        }

        let index = self.index(x, y);
        self.cells[index] = cell;
        self.touch(y, x, x);
    }

    /// Blanks any double-width glyph that `x..x + width` would only half cover.
    ///
    /// Writing over either half of a wide glyph has to erase the whole glyph. A
    /// leftover first half would make this row disagree with the applier, which
    /// advances by the glyph's own width and would then place every later cell on
    /// the row one column further along than the emulator has it.
    pub fn clear_wide_overlap(&mut self, x: u16, y: u16, width: u16) {
        if x >= self.cols || y >= self.rows {
            return;
        }

        if x > 0 && self.cell(x, y).is_continuation() {
            let index = self.index(x - 1, y);
            self.cells[index] = Cell::blank();
            self.touch(y, x - 1, x - 1);
        }

        for offset in 0..width {
            let column = match x.checked_add(offset) {
                Some(column) if column < self.cols => column,
                _ => break,
            };
            let covered = self.cell(column, y);

            for tail in 1..u16::from(covered.width) {
                let partner = match column.checked_add(tail) {
                    Some(partner) if partner < self.cols => partner,
                    _ => break,
                };

                if partner < x || partner >= x.saturating_add(width) {
                    let index = self.index(partner, y);
                    self.cells[index] = Cell::blank();
                    self.touch(y, partner, partner);
                }
            }
        }
    }

    /// Inclusive on both ends.
    pub fn fill(&mut self, y: u16, from_x: u16, to_x: u16, cell: Cell) {
        if y >= self.rows || from_x >= self.cols || from_x > to_x {
            return;
        }

        let to_x = to_x.min(self.cols - 1);

        for x in from_x..=to_x {
            let index = self.index(x, y);
            self.cells[index] = cell;
        }

        self.touch(y, from_x, to_x);

        // Every erase reaches here, and an erase whose run starts or ends inside a
        // double-width pair blanks one half and leaves the other. A lone half is
        // what makes this row disagree with the applier about every later column.
        self.repair_wide(y);
    }

    pub fn clear_row(&mut self, y: u16) {
        self.fill(y, 0, self.cols - 1, Cell::blank());
    }

    fn touch(&mut self, y: u16, from_x: u16, to_x: u16) {
        if y >= self.rows {
            return;
        }

        let from_x = from_x.min(self.cols - 1);
        let to_x = to_x.min(self.cols - 1);
        let slot = &mut self.dirty[usize::from(y)];

        *slot = Some(match *slot {
            None => (from_x, to_x),
            Some((low, high)) => (low.min(from_x), high.max(to_x)),
        });
    }

    pub fn mark_all_dirty(&mut self) {
        let last = self.cols - 1;

        for y in 0..self.rows {
            self.touch(y, 0, last);
        }
    }

    pub fn dirty_rows(&self) -> usize {
        self.dirty.iter().filter(|span| span.is_some()).count()
    }

    /// `(y, from_x, to_x)` for every dirty row, top row first, clearing the
    /// tracking as it goes.
    pub fn take_dirty(&mut self) -> Vec<(u16, u16, u16)> {
        let mut taken = Vec::new();

        for y in 0..self.rows {
            if let Some((from_x, to_x)) = self.dirty[usize::from(y)].take() {
                taken.push((y, from_x, to_x));
            }
        }

        taken
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let cols = cols.max(1);
        let rows = rows.max(1);

        if cols == self.cols && rows == self.rows {
            return;
        }

        let mut next = vec![Cell::blank(); usize::from(cols) * usize::from(rows)];

        for y in 0..rows.min(self.rows) {
            for x in 0..cols.min(self.cols) {
                next[usize::from(y) * usize::from(cols) + usize::from(x)] = self.cell(x, y);
            }
        }

        self.cols = cols;
        self.rows = rows;
        self.cells = next;
        self.dirty = vec![None; usize::from(rows)];
        self.mark_all_dirty();
    }

    /// Moves `top..=bottom` up by `count`, returning the rows that left the top
    /// so a caller can push them to scrollback.
    pub fn scroll_up(&mut self, top: u16, bottom: u16, count: u16) -> Vec<Vec<Cell>> {
        if top > bottom || bottom >= self.rows || count == 0 {
            return Vec::new();
        }

        let span = bottom - top + 1;
        let count = count.min(span);
        let mut evicted = Vec::with_capacity(usize::from(count));

        for offset in 0..count {
            evicted.push(self.row(top + offset).to_vec());
        }

        // `count == span` means every row in the region left, so there is nothing
        // to move up and the fill below blanks the whole region. Subtracting
        // first would underflow.
        if count < span {
            for y in top..=bottom - count {
                let source = self.index(0, y + count);
                let target = self.index(0, y);

                for x in 0..usize::from(self.cols) {
                    self.cells[target + x] = self.cells[source + x];
                }
            }
        }

        for y in (bottom + 1 - count)..=bottom {
            let start = self.index(0, y);

            for x in 0..usize::from(self.cols) {
                self.cells[start + x] = Cell::blank();
            }
        }

        // Every row in the region changed, so the whole region is dirty.
        for y in top..=bottom {
            self.touch(y, 0, self.cols - 1);
        }

        evicted
    }

    pub fn scroll_down(&mut self, top: u16, bottom: u16, count: u16) {
        if top > bottom || bottom >= self.rows || count == 0 {
            return;
        }

        let span = bottom - top + 1;
        let count = count.min(span);

        if count < span {
            for y in (top + count..=bottom).rev() {
                let source = self.index(0, y - count);
                let target = self.index(0, y);

                for x in 0..usize::from(self.cols) {
                    self.cells[target + x] = self.cells[source + x];
                }
            }
        }

        for y in top..top + count {
            let start = self.index(0, y);

            for x in 0..usize::from(self.cols) {
                self.cells[start + x] = Cell::blank();
            }
        }

        for y in top..=bottom {
            self.touch(y, 0, self.cols - 1);
        }
    }

    /// Blanks any double-width glyph a shift left only half of.
    ///
    /// `insert_cells` and `delete_cells` move cells one at a time, so a pair can
    /// be split at either edge of the shift. A lone half is what makes the
    /// emitter and the applier disagree about every later column on the row.
    /// Repairs every row, for an operation that could have split a pair anywhere.
    pub fn repair_all_wide(&mut self) {
        for y in 0..self.rows {
            self.repair_wide(y);
        }
    }

    fn repair_wide(&mut self, y: u16) {
        for x in 0..self.cols {
            let cell = self.cell(x, y);

            if cell.is_continuation() {
                let lead_is_wide = x > 0 && self.cell(x - 1, y).width > 1;

                if !lead_is_wide {
                    let index = self.index(x, y);
                    self.cells[index] = Cell::blank();
                    self.touch(y, x, x);
                }

                continue;
            }

            for tail in 1..u16::from(cell.width) {
                match x.checked_add(tail) {
                    Some(partner) if partner < self.cols => {
                        if !self.cell(partner, y).is_continuation() {
                            let index = self.index(x, y);
                            self.cells[index] = Cell::blank();
                            self.touch(y, x, x);
                        }
                    }
                    // A wide glyph whose second column fell off the right edge.
                    _ => {
                        let index = self.index(x, y);
                        self.cells[index] = Cell::blank();
                        self.touch(y, x, x);
                    }
                }
            }
        }
    }

    pub fn insert_cells(&mut self, x: u16, y: u16, count: u16) {
        if y >= self.rows || x >= self.cols || count == 0 {
            return;
        }

        let count = count.min(self.cols - x);

        for target in (x + count..self.cols).rev() {
            let cell = self.cell(target - count, y);
            let index = self.index(target, y);
            self.cells[index] = cell;
        }

        for target in x..x + count {
            let index = self.index(target, y);
            self.cells[index] = Cell::blank();
        }

        self.touch(y, x, self.cols - 1);
        self.repair_wide(y);
    }

    pub fn delete_cells(&mut self, x: u16, y: u16, count: u16) {
        if y >= self.rows || x >= self.cols || count == 0 {
            return;
        }

        let count = count.min(self.cols - x);

        for target in x..self.cols - count {
            let cell = self.cell(target + count, y);
            let index = self.index(target, y);
            self.cells[index] = cell;
        }

        for target in self.cols - count..self.cols {
            let index = self.index(target, y);
            self.cells[index] = Cell::blank();
        }

        self.touch(y, x, self.cols - 1);
        self.repair_wide(y);
    }
}
