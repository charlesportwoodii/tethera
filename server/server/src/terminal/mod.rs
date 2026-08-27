//! A pane's bytes, emulated, as `TerminalFrame`s.
//!
//! The grid kept here is the wire format's own shape rather than a renderer's:
//! rows of styled cells, damage tracked per row, and a style table interned per
//! frame. `common/src/protocol/grid.rs` is the normative applier, and every test
//! in `tests/terminal/` proves the two agree by applying this module's output to
//! it.

mod buffer;
mod budget;
mod cell;
mod emulator;
mod event;
mod frames;
mod io;
mod keys;
mod picker;
mod prompt;
mod pen;
mod pty;
mod registry;
mod screen;
mod styles;

pub use buffer::Buffer;
pub use budget::FrameBudget;
pub use cell::Cell;
pub use emulator::Emulator;
pub use event::PaneEvent;
pub use frames::FrameBuilder;
pub use io::PaneIo;
pub use keys::KeyEncoder;
pub use pen::Pen;
pub use picker::Picker;
pub use prompt::PromptDetector;
pub use pty::{PtyBackend, PtyPane};
pub use registry::{PaneEmulator, PaneRegistry};
pub use screen::Screen;
pub use styles::StyleTable;
