use std::collections::VecDeque;

use tethera_common::protocol::terminal::{CursorShape, CursorState};
use unicode_width::UnicodeWidthChar;

use crate::terminal::buffer::Buffer;
use crate::terminal::cell::Cell;
use crate::terminal::pen::Pen;

/// A pane's emulated screen.
///
/// Owns both buffers, because the alternate screen is a different grid rather
/// than a mode over one grid, and scrollback belongs to the primary buffer alone.
pub struct Screen {
    primary: Buffer,
    alt: Buffer,
    alt_active: bool,
    scrollback: VecDeque<Vec<Cell>>,
    pen: Pen,
    x: u16,
    y: u16,
    saved: Option<(u16, u16, Pen)>,
    saved_alt: Option<(u16, u16, Pen)>,
    cursor_visible: bool,
    cursor_shape: CursorShape,
    cursor_moved: bool,
    scroll_top: u16,
    scroll_bottom: u16,
    autowrap: bool,
    insert_mode: bool,
    pending_wrap: bool,
    application_cursor_keys: bool,
    bracketed_paste: bool,
    last_printed: Option<char>,
    bell: bool,
    replies: Vec<u8>,
}

impl Screen {
    /// Bounded on purpose: an unbounded scrollback is a leak with a lookup
    /// method. Two thousand lines is well past what a phone will page.
    pub const SCROLLBACK_LINES: usize = 2000;

    const TAB_WIDTH: u16 = 8;

    /// A cap on any single repeat or scroll count.
    ///
    /// A hostile `CSI 65535 b` would otherwise be 65535 writes for one escape
    /// sequence. Clamping to the screen bounds costs nothing legitimate: no real
    /// program repeats past the edge of a row it can see.
    const MAX_REPEAT: u16 = 4096;

    pub fn new(cols: u16, rows: u16) -> Self {
        let primary = Buffer::new(cols, rows);
        let alt = Buffer::new(cols, rows);
        let bottom = primary.rows() - 1;

        Self {
            primary,
            alt,
            alt_active: false,
            scrollback: VecDeque::new(),
            pen: Pen::new(),
            x: 0,
            y: 0,
            saved: None,
            saved_alt: None,
            cursor_visible: true,
            cursor_shape: CursorShape::Block,
            cursor_moved: false,
            scroll_top: 0,
            scroll_bottom: bottom,
            autowrap: true,
            insert_mode: false,
            pending_wrap: false,
            application_cursor_keys: false,
            bracketed_paste: false,
            last_printed: None,
            bell: false,
            replies: Vec::new(),
        }
    }

    pub fn active(&self) -> &Buffer {
        if self.alt_active {
            &self.alt
        } else {
            &self.primary
        }
    }

    pub fn active_mut(&mut self) -> &mut Buffer {
        if self.alt_active {
            &mut self.alt
        } else {
            &mut self.primary
        }
    }

    /// The visible grid as lines, trailing blanks trimmed.
    ///
    /// What a detector reads. A wide character's continuation cell contributes
    /// nothing, because the character itself already sits in the cell before it
    /// and emitting both would double every CJK glyph on the screen.
    pub fn text(&self) -> String {
        let buffer = self.active();
        let mut lines = Vec::with_capacity(usize::from(buffer.rows()));

        for y in 0..buffer.rows() {
            let mut line = String::new();

            for cell in buffer.row(y) {
                if cell.is_continuation() {
                    continue;
                }

                line.push(cell.ch);
            }

            lines.push(line.trim_end().to_string());
        }

        lines.join("\n")
    }

    pub fn alt_screen(&self) -> bool {
        self.alt_active
    }

    pub fn application_cursor_keys(&self) -> bool {
        self.application_cursor_keys
    }

    pub fn bracketed_paste(&self) -> bool {
        self.bracketed_paste
    }

    pub fn cursor(&self) -> CursorState {
        CursorState {
            x: self.x.min(self.active().cols() - 1),
            y: self.y.min(self.active().rows() - 1),
            visible: self.cursor_visible,
            shape: self.cursor_shape,
        }
    }

    pub fn take_cursor_moved(&mut self) -> bool {
        std::mem::take(&mut self.cursor_moved)
    }

    pub fn take_bell(&mut self) -> bool {
        std::mem::take(&mut self.bell)
    }

    /// Bytes the pane is owed in answer to a query it sent.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies)
    }

    pub fn scrollback_len(&self) -> Option<u32> {
        // Not zero. A pane owning the alternate screen genuinely has no
        // scrollback, and zero reads as "there is history and it is empty".
        if self.alt_active {
            return None;
        }

        Some(u32::try_from(self.scrollback.len()).unwrap_or(u32::MAX))
    }

    /// Lines older than `before_line`, oldest first, so a page applies as rows
    /// top to bottom. Index 0 is the oldest line still held.
    pub fn scrollback_page(
        &self,
        before_line: Option<u32>,
        limit: u16,
    ) -> (Vec<Vec<Cell>>, Option<u32>, bool) {
        let end = before_line
            .map(|line| usize::try_from(line).unwrap_or(usize::MAX).min(self.scrollback.len()))
            .unwrap_or(self.scrollback.len());
        let take = usize::from(limit).min(end);
        let start = end - take;
        let lines: Vec<Vec<Cell>> = self
            .scrollback
            .iter()
            .skip(start)
            .take(take)
            .cloned()
            .collect();
        let next = if start == 0 {
            None
        } else {
            Some(u32::try_from(start).unwrap_or(u32::MAX))
        };

        (lines, next, start > 0)
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.primary.resize(cols, rows);
        self.alt.resize(cols, rows);

        // A shrink can cut between a double-width glyph's two halves, and the
        // applier drops a glyph that has fewer columns left than it needs - along
        // with the rest of that row's spans.
        self.primary.repair_all_wide();
        self.alt.repair_all_wide();

        // The region is reset rather than clamped, which is what xterm does and
        // the only safe answer: a `scroll_bottom` left past the new last row
        // makes `line_feed`'s equality test unsatisfiable, and the cursor then
        // sticks on the bottom row forever, overwriting one line and never
        // scrolling. That is a pane that looks alive and shows nothing.
        let bottom = self.active().rows() - 1;
        self.scroll_top = 0;
        self.scroll_bottom = bottom;

        // Clamped for the same reason: `Buffer::set` silently drops a write out
        // of bounds, so an unclamped cursor is a pane that goes blank with no
        // error anywhere.
        self.x = self.x.min(self.active().cols() - 1);
        self.y = self.y.min(bottom);
        self.pending_wrap = false;

        // A saved cursor from the old geometry may name a cell that no longer
        // exists, and restoring it would move the cursor somewhere the program
        // never put it.
        self.saved = None;
        self.saved_alt = None;
        self.cursor_moved = true;
    }

    fn push_scrollback(&mut self, mut line: Vec<Cell>) {
        while line.last().map(Cell::is_blank).unwrap_or(false) {
            line.pop();
        }

        if self.scrollback.len() == Self::SCROLLBACK_LINES {
            self.scrollback.pop_front();
        }

        self.scrollback.push_back(line);
    }

    fn carriage_return(&mut self) {
        self.x = 0;
        self.pending_wrap = false;
        self.cursor_moved = true;
    }

    fn line_feed(&mut self) {
        self.pending_wrap = false;
        self.cursor_moved = true;

        if self.y == self.scroll_bottom {
            let top = self.scroll_top;
            let bottom = self.scroll_bottom;
            let full = top == 0 && bottom == self.active().rows() - 1;
            let evicted = self.active_mut().scroll_up(top, bottom, 1);

            // Only the primary buffer has history, and only a scroll of the whole
            // screen pushes a line out of it. A region scroll inside a
            // full-screen application is a repaint, not history.
            if !self.alt_active && full {
                for line in evicted {
                    self.push_scrollback(line);
                }
            }

            return;
        }

        if self.y + 1 < self.active().rows() {
            self.y += 1;
        }
    }

    fn reverse_index(&mut self) {
        self.pending_wrap = false;
        self.cursor_moved = true;

        if self.y == self.scroll_top {
            let (top, bottom) = (self.scroll_top, self.scroll_bottom);
            self.active_mut().scroll_down(top, bottom, 1);

            return;
        }

        self.y = self.y.saturating_sub(1);
    }

    fn tab(&mut self) {
        let cols = self.active().cols();
        let next = (self.x / Self::TAB_WIDTH)
            .saturating_add(1)
            .saturating_mul(Self::TAB_WIDTH);
        self.x = next.min(cols - 1);
        self.pending_wrap = false;
        self.cursor_moved = true;
    }

    fn move_to(&mut self, x: u16, y: u16) {
        self.x = x.min(self.active().cols() - 1);
        self.y = y.min(self.active().rows() - 1);
        self.pending_wrap = false;
        self.cursor_moved = true;
    }

    /// Draws one character at the cursor.
    ///
    /// Named apart from `Perform::put`, which takes a `u8` and is a DCS payload
    /// byte. An inherent method wins name resolution over a trait one, so calling
    /// both `put` would work today and would silently break the moment somebody
    /// implements DCS handling.
    fn print_char(&mut self, ch: char) {
        // A zero-width character has no cell of its own and cannot be folded into
        // the previous one: `GridCell.ch` is a single `char`, so a base plus a
        // combining mark is not expressible on this wire.
        let width = match UnicodeWidthChar::width(ch) {
            Some(0) | None => return,
            Some(measured) => u16::try_from(measured).unwrap_or(1),
        };

        let cols = self.active().cols();

        if width > cols {
            return;
        }

        if self.pending_wrap && self.autowrap {
            self.carriage_return();
            self.line_feed();
        }

        if self.x.saturating_add(width) > cols {
            if self.autowrap {
                self.carriage_return();
                self.line_feed();
            } else {
                // Stuck at the right margin, overwriting. The cursor does not
                // advance, which is what makes a non-wrapping row end in the last
                // glyph written rather than in the first.
                self.x = cols - width;
            }
        }

        let style = self.pen.style();
        let (x, y) = (self.x, self.y);
        let cell = Cell::new(ch, style, u8::try_from(width).unwrap_or(1));

        // `IRM`. Shifting first is what makes this an insert rather than an
        // overwrite; without it a program that set the mode silently corrupts the
        // line it is editing instead of failing visibly.
        if self.insert_mode {
            self.active_mut().insert_cells(x, y, width);
        }

        self.active_mut().clear_wide_overlap(x, y, width);
        self.active_mut().set(x, y, cell);

        for continuation in 1..width {
            self.active_mut()
                .set(x + continuation, y, Cell::continuation(style));
        }

        self.last_printed = Some(ch);
        self.cursor_moved = true;

        if self.autowrap && self.x.saturating_add(width) >= cols {
            // Held at the last column until the next character arrives. Moving
            // off the row now would make every wrapped line lose its first
            // character.
            self.x = cols - 1;
            self.pending_wrap = true;
        } else {
            self.x = (self.x + width).min(cols - 1);
        }
    }

    fn erase_in_line(&mut self, mode: u16) {
        let last = self.active().cols() - 1;
        let (x, y) = (self.x, self.y);

        match mode {
            1 => self.active_mut().fill(y, 0, x, Cell::blank()),
            2 => self.active_mut().fill(y, 0, last, Cell::blank()),
            _ => self.active_mut().fill(y, x, last, Cell::blank()),
        }
    }

    fn erase_in_display(&mut self, mode: u16) {
        let last_col = self.active().cols() - 1;
        let rows = self.active().rows();
        let (x, y) = (self.x, self.y);

        match mode {
            1 => {
                for row in 0..y {
                    self.active_mut().clear_row(row);
                }

                self.active_mut().fill(y, 0, x, Cell::blank());
            }
            2 => {
                for row in 0..rows {
                    self.active_mut().clear_row(row);
                }
            }
            3 => {
                for row in 0..rows {
                    self.active_mut().clear_row(row);
                }

                self.scrollback.clear();
            }
            _ => {
                self.active_mut().fill(y, x, last_col, Cell::blank());

                for row in y + 1..rows {
                    self.active_mut().clear_row(row);
                }
            }
        }
    }

    fn switch_alt(&mut self, on: bool, save_and_clear: bool) {
        if self.alt_active == on {
            return;
        }

        if on {
            if save_and_clear {
                self.saved_alt = Some((self.x, self.y, self.pen));
            }

            self.alt_active = true;

            // Only `1049` clears. `47` and `1047` switch to whatever the
            // alternate buffer already held, which is the historical difference
            // and the reason all three are handled rather than aliased: a program
            // that switches away with `47` and back expects its content to
            // survive.
            //
            // Cleared rather than reallocated, so a buffer that was resized while
            // hidden cannot come back with the old geometry.
            if save_and_clear {
                for row in 0..self.alt.rows() {
                    self.alt.clear_row(row);
                }

                self.x = 0;
                self.y = 0;
                self.pen.reset();
            }
        } else {
            self.alt_active = false;

            if save_and_clear {
                if let Some((x, y, pen)) = self.saved_alt.take() {
                    self.x = x;
                    self.y = y;
                    self.pen = pen;
                }
            }
        }

        let bottom = self.active().rows() - 1;
        self.scroll_top = 0;
        self.scroll_bottom = bottom;
        self.x = self.x.min(self.active().cols() - 1);
        self.y = self.y.min(bottom);
        self.pending_wrap = false;

        // Every cell of the newly visible grid changed.
        self.active_mut().mark_all_dirty();
        self.cursor_moved = true;
    }

    fn set_mode(&mut self, private: bool, code: u16, on: bool) {
        if !private {
            match code {
                // `IRM`. The only ANSI mode this emulator implements; the rest
                // are listed as gaps rather than silently ignored.
                4 => self.insert_mode = on,
                _ => {}
            }

            return;
        }

        match code {
            1 => self.application_cursor_keys = on,
            7 => {
                self.autowrap = on;
                self.pending_wrap = false;
            }
            25 => {
                self.cursor_visible = on;
                self.cursor_moved = true;
            }
            47 | 1047 => self.switch_alt(on, false),
            1049 => self.switch_alt(on, true),
            2004 => self.bracketed_paste = on,
            // Mouse reporting, focus reporting and win32-input-mode are accepted
            // and ignored: this protocol carries no pointer or focus event, and an
            // unknown private mode must be ignored rather than mishandled.
            _ => {}
        }
    }

    fn reset(&mut self) {
        let (cols, rows) = (self.primary.cols(), self.primary.rows());

        // Scrollback survives, which is a departure from what RIS conventionally
        // does. History a person can still scroll back to is not screen state, and
        // an agent that resets its terminal mid-session should not take the
        // transcript of what it just did with it.
        let scrollback = std::mem::take(&mut self.scrollback);

        // Replies and the bell survive too, because they are owed to the program
        // rather than being state of the screen. A reset arriving in the same
        // chunk as a device query would otherwise swallow the answer - and an
        // unanswered query is a pane that never starts on Windows.
        let replies = std::mem::take(&mut self.replies);
        let bell = self.bell;

        *self = Self::new(cols, rows);
        self.scrollback = scrollback;
        self.replies = replies;
        self.bell = bell;
        self.active_mut().mark_all_dirty();
    }

    fn param(params: &vte::Params, index: usize) -> u16 {
        params
            .iter()
            .nth(index)
            .and_then(|values| values.first().copied())
            .unwrap_or(0)
    }

    /// A count parameter, where zero means one and anything huge is clamped.
    fn count(params: &vte::Params, index: usize) -> u16 {
        Self::param(params, index).max(1).min(Self::MAX_REPEAT)
    }
}

impl vte::Perform for Screen {
    fn print(&mut self, c: char) {
        self.print_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x07 => self.bell = true,
            0x08 => {
                self.x = self.x.saturating_sub(1);
                self.pending_wrap = false;
                self.cursor_moved = true;
            }
            0x09 => self.tab(),
            0x0a | 0x0b | 0x0c => self.line_feed(),
            0x0d => self.carriage_return(),
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        ignore: bool,
        action: char,
    ) {
        if ignore {
            return;
        }

        let private = intermediates.first() == Some(&b'?');
        let space = intermediates.first() == Some(&b' ');
        let greater = intermediates.first() == Some(&b'>');
        let first = Self::param(params, 0);
        let count = Self::count(params, 0);

        match action {
            'A' => {
                let y = self.y.saturating_sub(count);
                self.move_to(self.x, y);
            }
            'B' | 'e' => {
                let y = self.y.saturating_add(count);
                self.move_to(self.x, y);
            }
            'C' | 'a' => {
                let x = self.x.saturating_add(count);
                self.move_to(x, self.y);
            }
            'D' => {
                let x = self.x.saturating_sub(count);
                self.move_to(x, self.y);
            }
            'E' => {
                let y = self.y.saturating_add(count);
                self.move_to(0, y);
            }
            'F' => {
                let y = self.y.saturating_sub(count);
                self.move_to(0, y);
            }
            'G' | '`' => self.move_to(count - 1, self.y),
            'd' => self.move_to(self.x, count - 1),
            'H' | 'f' => {
                let row = Self::count(params, 0) - 1;
                let column = Self::count(params, 1) - 1;
                self.move_to(column, row);
            }
            'J' => self.erase_in_display(first),
            'K' => self.erase_in_line(first),
            // Both are no-ops with the cursor outside the scroll region, which is
            // what xterm does. Scrolling from the cursor to the region's bottom
            // regardless would move a band larger than the region a program asked
            // to confine itself to.
            'L' => {
                let (y, top, bottom) = (self.y, self.scroll_top, self.scroll_bottom);

                if y >= top && y <= bottom {
                    self.active_mut().scroll_down(y, bottom, count);
                }
            }
            'M' => {
                let (y, top, bottom) = (self.y, self.scroll_top, self.scroll_bottom);

                if y >= top && y <= bottom {
                    self.active_mut().scroll_up(y, bottom, count);
                }
            }
            '@' => {
                let (x, y) = (self.x, self.y);
                self.active_mut().insert_cells(x, y, count);
            }
            'P' => {
                let (x, y) = (self.x, self.y);
                self.active_mut().delete_cells(x, y, count);
            }
            'X' => {
                let (x, y) = (self.x, self.y);
                let to = x.saturating_add(count - 1);
                self.active_mut().fill(y, x, to, Cell::blank());
            }
            'S' => {
                let (top, bottom) = (self.scroll_top, self.scroll_bottom);
                let evicted = self.active_mut().scroll_up(top, bottom, count);
                let full = top == 0 && bottom == self.active().rows() - 1;

                if !self.alt_active && full {
                    for line in evicted {
                        self.push_scrollback(line);
                    }
                }
            }
            'T' => {
                let (top, bottom) = (self.scroll_top, self.scroll_bottom);
                self.active_mut().scroll_down(top, bottom, count);
            }
            'b' => {
                if let Some(ch) = self.last_printed {
                    for _ in 0..count {
                        self.print_char(ch);
                    }
                }
            }
            // Only the plain form. `CSI ?1000m` is a private-parameter SGR, which
            // xterm ignores rather than reading as a colour.
            'm' if intermediates.is_empty() => self.pen.apply_sgr(params),
            'h' => self.set_mode(private, first, true),
            'l' => self.set_mode(private, first, false),
            'r' => {
                let bottom_edge = self.active().rows() - 1;
                let top = (Self::count(params, 0) - 1).min(bottom_edge);
                let bottom = match Self::param(params, 1) {
                    0 => bottom_edge,
                    value => value.min(bottom_edge + 1) - 1,
                };

                // A region the program asked for that cannot exist resets to the
                // whole screen rather than being ignored. Leaving a
                // `scroll_bottom` past the last row makes `line_feed`'s equality
                // test unsatisfiable, and the cursor then sticks on one row
                // forever: four bytes from the program would wedge the pane.
                if top < bottom {
                    self.scroll_top = top;
                    self.scroll_bottom = bottom;
                } else {
                    self.scroll_top = 0;
                    self.scroll_bottom = bottom_edge;
                }

                // DECSTBM homes the cursor inside the new region.
                let home = self.scroll_top;
                self.move_to(0, home);
            }
            's' => self.saved = Some((self.x, self.y, self.pen)),
            'u' => {
                if let Some((x, y, pen)) = self.saved {
                    self.pen = pen;
                    self.move_to(x, y);
                }
            }
            'q' if space => {
                self.cursor_shape = match first {
                    3 | 4 => CursorShape::Underline,
                    5 | 6 => CursorShape::Bar,
                    _ => CursorShape::Block,
                };
                self.cursor_moved = true;
            }
            // A program that asks and is never answered hangs. ConPTY asks for
            // the cursor position before it will run anything at all, so these
            // replies are what makes a pane start on Windows.
            'c' if greater => self.replies.extend_from_slice(b"\x1b[>0;10;1c"),
            'c' => self.replies.extend_from_slice(b"\x1b[?62;22c"),
            'n' => match first {
                5 => self.replies.extend_from_slice(b"\x1b[0n"),
                6 => {
                    let cursor = self.cursor();
                    let reply = format!("\x1b[{};{}R", cursor.y + 1, cursor.x + 1);
                    self.replies.extend_from_slice(reply.as_bytes());
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        if ignore || !intermediates.is_empty() {
            // Charset designation and the other intermediate forms are accepted
            // and ignored: this emulator draws what it is sent rather than
            // translating a G0 set.
            return;
        }

        match byte {
            b'7' => self.saved = Some((self.x, self.y, self.pen)),
            b'8' => {
                if let Some((x, y, pen)) = self.saved {
                    self.pen = pen;
                    self.move_to(x, y);
                }
            }
            b'D' => self.line_feed(),
            b'E' => {
                self.carriage_return();
                self.line_feed();
            }
            b'M' => self.reverse_index(),
            b'c' => self.reset(),
            _ => {}
        }
    }

    // Parsed and dropped, every one of them. A window title has no
    // `TerminalFrame` variant, a hyperlink has nowhere to live - `Style` is a
    // foreground, a background and an attribute byte - and a clipboard write
    // from a pane is not something this protocol carries. Consuming them here is
    // what stops the payload being drawn as text.
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}
}
