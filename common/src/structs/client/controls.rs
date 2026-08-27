use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a machine will let a conversation screen do.
///
/// Read from the capability set recorded at the last handshake rather than
/// discovered by trying. A control drawn and then refused on press teaches
/// somebody that the app is unreliable; a control that is absent, with a line
/// saying why, teaches them what this machine is.
///
/// Booleans rather than the raw set, because the screen asks the same seven
/// questions every time and a set would put the capability names in the
/// TypeScript, where a rename would go unnoticed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct ConversationControls {
    /// The machine's own ceiling on turns per page.
    ///
    /// Read from the machine rather than chosen here. The machine bounds a page
    /// by bytes as well as by count, so asking for its ceiling returns however
    /// many fit and reads no more than it sends. A number picked by the client
    /// is either smaller than it needs to be or larger than a frame can carry.
    pub transcript_page: u16,
    pub send: bool,
    pub answer: bool,
    pub interrupt: bool,
    pub resume: bool,
    pub stop: bool,
    pub read_files: bool,
    pub attach_files: bool,
}
