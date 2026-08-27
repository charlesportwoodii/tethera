use tethera_common::protocol::terminal::{attrs, Color, Style};

use crate::terminal::cell::Cell;

/// The SGR state a printed character is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pen {
    style: Style,
}

impl Pen {
    pub fn new() -> Self {
        Self { style: Cell::PLAIN }
    }

    pub fn style(&self) -> Style {
        self.style
    }

    pub fn reset(&mut self) {
        self.style = Cell::PLAIN;
    }

    /// `CSI m`.
    ///
    /// An empty parameter list is `SGR 0`, which is not a nicety: ConPTY opens a
    /// pane with a bare `ESC[m` and treating it as a no-op would leave the pen
    /// wherever it was.
    pub fn apply_sgr(&mut self, params: &vte::Params) {
        if params.is_empty() {
            self.reset();

            return;
        }

        let flat: Vec<Vec<u16>> = params.iter().map(<[u16]>::to_vec).collect();
        let mut index = 0;

        while index < flat.len() {
            let item = &flat[index];
            let code = item.first().copied().unwrap_or(0);

            let consumed = match code {
                38 | 48 => {
                    // A subparameter form carries its arguments with it, so
                    // `38:2:r:g:b` must not also eat the parameters after it the
                    // way `38;2;r;g;b` does.
                    let (color, consumed) = if item.len() > 1 {
                        (Self::extended(&item[1..]), 1)
                    } else {
                        let rest: Vec<u16> = flat[index + 1..]
                            .iter()
                            .filter_map(|param| param.first().copied())
                            .collect();
                        let (color, used) = Self::sequential(&rest);

                        (color, 1 + used)
                    };

                    if let Some(color) = color {
                        if code == 38 {
                            self.style.fg = color;
                        } else {
                            self.style.bg = color;
                        }
                    }

                    consumed
                }
                _ => {
                    self.apply_one(code);

                    1
                }
            };

            index += consumed.max(1);
        }
    }

    fn apply_one(&mut self, code: u16) {
        match code {
            0 => self.reset(),
            1 => self.style.attrs |= attrs::BOLD,
            2 => self.style.attrs |= attrs::DIM,
            3 => self.style.attrs |= attrs::ITALIC,
            4 => self.style.attrs |= attrs::UNDERLINE,
            5 | 6 => self.style.attrs |= attrs::BLINK,
            7 => self.style.attrs |= attrs::REVERSE,
            9 => self.style.attrs |= attrs::STRIKE,
            21 | 22 => self.style.attrs &= !(attrs::BOLD | attrs::DIM),
            23 => self.style.attrs &= !attrs::ITALIC,
            24 => self.style.attrs &= !attrs::UNDERLINE,
            25 => self.style.attrs &= !attrs::BLINK,
            27 => self.style.attrs &= !attrs::REVERSE,
            29 => self.style.attrs &= !attrs::STRIKE,
            30..=37 => self.style.fg = Color::Indexed((code - 30) as u8),
            39 => self.style.fg = Color::Default,
            40..=47 => self.style.bg = Color::Indexed((code - 40) as u8),
            49 => self.style.bg = Color::Default,
            90..=97 => self.style.fg = Color::Indexed((code - 90 + 8) as u8),
            100..=107 => self.style.bg = Color::Indexed((code - 100 + 8) as u8),
            _ => {}
        }
    }

    /// The `38:5:n` and `38:2:r:g:b` subparameter forms.
    fn extended(args: &[u16]) -> Option<Color> {
        match args.first().copied() {
            Some(5) => args.get(1).map(|index| Color::Indexed(*index as u8)),
            Some(2) => match (args.get(1), args.get(2), args.get(3)) {
                (Some(r), Some(g), Some(b)) => Some(Color::Rgb(*r as u8, *g as u8, *b as u8)),
                _ => None,
            },
            _ => None,
        }
    }

    /// The `38;5;n` and `38;2;r;g;b` forms, which spend the parameters that
    /// follow them. Returns how many were spent so the caller can skip them.
    fn sequential(rest: &[u16]) -> (Option<Color>, usize) {
        match rest.first().copied() {
            Some(5) => (
                rest.get(1).map(|index| Color::Indexed(*index as u8)),
                2.min(rest.len()),
            ),
            Some(2) => {
                let color = match (rest.get(1), rest.get(2), rest.get(3)) {
                    (Some(r), Some(g), Some(b)) => {
                        Some(Color::Rgb(*r as u8, *g as u8, *b as u8))
                    }
                    _ => None,
                };

                (color, 4.min(rest.len()))
            }
            _ => (None, rest.len().min(1)),
        }
    }
}

impl Default for Pen {
    fn default() -> Self {
        Self::new()
    }
}
