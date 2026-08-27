use tethera_common::protocol::terminal::CloseReason;
use tethera_common::structs::terminal::Size;

/// What a pane hands the emulator.
///
/// A resize is an event rather than a call because the backend observes it: a
/// split re-lays-out its neighbour, so a pane's geometry changes without anyone
/// asking. The protocol has no resize in either direction and the client only
/// ever refits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneEvent {
    Output(Vec<u8>),
    Resized(Size),
    Closed(CloseReason),
}
