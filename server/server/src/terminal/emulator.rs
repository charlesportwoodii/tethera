use tethera_common::protocol::terminal::TerminalFrame;
use tethera_common::structs::terminal::Size;

use crate::terminal::frames::FrameBuilder;
use crate::terminal::screen::Screen;

/// A `vte` parser and the screen it drives.
///
/// The parser and the screen are separate fields so `advance` can borrow both:
/// `Perform` is implemented by the screen, and the parser cannot also be the
/// performer.
pub struct Emulator {
    parser: vte::Parser,
    screen: Screen,
}

impl Emulator {
    /// Half the rows dirty is where a snapshot stops being larger than the damage
    /// it would replace: a snapshot drops blank rows and trims trailing blanks,
    /// and damage may not.
    const RESNAPSHOT_NUMERATOR: usize = 2;

    pub fn new(size: Size) -> Self {
        Self {
            parser: vte::Parser::new(),
            screen: Screen::new(size.cols, size.rows),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        let Self { parser, screen } = self;
        parser.advance(screen, bytes);
    }

    pub fn resize(&mut self, size: Size) {
        self.screen.resize(size.cols, size.rows);
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn take_bell(&mut self) -> bool {
        self.screen.take_bell()
    }

    pub fn take_replies(&mut self) -> Vec<u8> {
        self.screen.take_replies()
    }

    pub fn snapshot(&mut self) -> TerminalFrame {
        // A snapshot answers every outstanding change, so the damage it subsumes
        // must not arrive again as a second frame.
        let _ = self.screen.active_mut().take_dirty();
        let _ = self.screen.take_cursor_moved();

        FrameBuilder::snapshot(&self.screen)
    }

    /// Damage, never a snapshot, whatever the resnapshot rule would prefer.
    ///
    /// A behavioural seam for the drift suite. `next_frame` resnapshots once half
    /// the rows are dirty, and a snapshot clears implicitly — so a test that
    /// applied whatever `next_frame` returned would have any damage error wiped by
    /// the next full repaint, which is the opposite of what it means to test.
    pub fn damage_only(&mut self) -> Option<TerminalFrame> {
        if self.take_bell() {
            return None;
        }

        FrameBuilder::damage(&mut self.screen)
    }

    /// The next frame this pane owes a client, or `None` when it owes nothing.
    pub fn next_frame(&mut self) -> Option<TerminalFrame> {
        if self.take_bell() {
            return Some(TerminalFrame::Bell);
        }

        let rows = usize::from(self.screen.active().rows());

        if self.screen.active().dirty_rows() * Self::RESNAPSHOT_NUMERATOR >= rows {
            return Some(self.snapshot());
        }

        FrameBuilder::damage(&mut self.screen)
    }
}
