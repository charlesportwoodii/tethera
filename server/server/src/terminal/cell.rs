use tethera_common::protocol::terminal::{attrs, Color, Style};

/// One grid cell.
///
/// `width` is the glyph's column count, so a span builder can emit the glyph once
/// and step over the columns it covers. A continuation cell is `width == 0` and
/// contributes no character to a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub style: Style,
    pub width: u8,
}

impl Cell {
    pub const PLAIN: Style = Style {
        fg: Color::Default,
        bg: Color::Default,
        attrs: attrs::NONE,
    };

    pub fn blank() -> Self {
        Self {
            ch: ' ',
            style: Self::PLAIN,
            width: 1,
        }
    }

    pub fn new(ch: char, style: Style, width: u8) -> Self {
        Self { ch, style, width }
    }

    /// The second column of a double-width glyph.
    pub fn continuation(style: Style) -> Self {
        Self {
            ch: ' ',
            style,
            width: 0,
        }
    }

    pub fn is_continuation(&self) -> bool {
        self.width == 0
    }

    /// A default-styled space, which a snapshot may drop from the end of a row.
    ///
    /// The style has to match too: a space carrying a background colour is
    /// visible, so trimming it would erase a highlight the emulator drew.
    pub fn is_blank(&self) -> bool {
        self.ch == ' ' && self.style == Self::PLAIN
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self::blank()
    }
}
