use crate::protocol::view::PaneView;
use crate::structs::ids::PaneId;
use crate::structs::terminal::Size;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Style attribute bits, packed into the `attrs` byte of a `Style`.
///
/// A bit field rather than a struct of booleans because a style table is the hot
/// path: it is interned per frame and every span indexes it.
pub mod attrs {
    pub const NONE: u8 = 0;
    pub const BOLD: u8 = 1 << 0;
    pub const DIM: u8 = 1 << 1;
    pub const ITALIC: u8 = 1 << 2;
    pub const UNDERLINE: u8 = 1 << 3;
    pub const REVERSE: u8 = 1 << 4;
    pub const STRIKE: u8 = 1 << 5;
    pub const BLINK: u8 = 1 << 6;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum Color {
    /// The renderer's own foreground or background.
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub attrs: u8,
}

/// A run of text sharing one style.
///
/// `style` indexes the style table of *the frame it arrived in*. Tables are
/// per-frame and are never carried across frames.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Span {
    pub style: u16,
    pub text: String,
}

/// A run of cells on one row, starting at `from_x`.
///
/// Replaces the cells it covers and nothing else. Columns are cell columns: a
/// double-width glyph occupies two, and the server emits no spacer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct RowUpdate {
    pub y: u16,
    pub from_x: u16,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum CursorShape {
    Block,
    Underline,
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct CursorState {
    pub x: u16,
    pub y: u16,
    pub visible: bool,
    pub shape: CursorShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum CloseReason {
    /// The process in the pane exited.
    Exited,
    /// The pane was closed.
    PaneGone,
    /// The server stopped serving this attach.
    ServerShutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct AttachSpec {
    pub pane: PaneId,
    pub view: PaneView,
    /// What the client can draw.
    ///
    /// Honoured in `Lines`, where the server lays logical lines out to this
    /// width so the client never scrolls sideways. Ignored in `Screen`, where
    /// the pane's own geometry is the only correct one and the client refits.
    ///
    /// Carried on the attach because it is the only message that knows it, and
    /// because the alternative - resizing the pane to suit the phone - reflows
    /// it on the desk as well.
    pub viewport: Size,
}

/// What the server sends on an attach stream.
///
/// Never the backend's own chrome: no tab bar, no pane borders, no status line,
/// no key-hint row. One pane's content, emulated per pane. Tabs and splits are
/// native UI in the app, driven by RPCs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum TerminalFrame {
    /// Describes the whole grid. A row absent from `rows_data` is blank for its
    /// full width; this is the only frame that clears anything implicitly.
    Snapshot {
        cols: u16,
        rows: u16,
        styles: Vec<Style>,
        rows_data: Vec<RowUpdate>,
        cursor: Option<CursorState>,
        alt_screen: bool,
        /// `None` when the pane owns the alternate screen and genuinely has no
        /// scrollback. Not zero.
        #[ts(type = "number | null")]
        scrollback_len: Option<u32>,
    },
    /// Replaces the runs it names and nothing else. Never clears implicitly: to
    /// blank a region the server sends spans of spaces.
    Damage {
        styles: Vec<Style>,
        rows_data: Vec<RowUpdate>,
        cursor: Option<CursorState>,
    },
    /// An observation, not a contract with a backend. The client refits.
    Resized {
        cols: u16,
        rows: u16,
    },
    Bell,
    Closed {
        reason: CloseReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Mods(pub u8);

impl Mods {
    pub const NONE: Mods = Mods(0);
    pub const CTRL: Mods = Mods(1 << 0);
    pub const ALT: Mods = Mods(1 << 1);
    pub const SHIFT: Mods = Mods(1 << 2);

    pub fn contains(self, other: Mods) -> bool {
        self.0 & other.0 == other.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    F(u8),
}

/// What the client sends on an attach stream.
///
/// Intent, not bytes. The server encodes, so a CTRL+C overlay bar on a phone
/// never needs to know a terminal encoding. There is deliberately no raw-bytes
/// variant: it buys nothing and opens an injection surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum TerminalInput {
    Text(String),
    Key { key: Key, mods: Mods },
}
