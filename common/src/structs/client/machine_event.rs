use crate::protocol::watch::WatchEvent;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One machine watch event, addressed to the machine it came from.
///
/// A client-local type, not a wire type. It is JSON over the Tauri boundary and
/// never postcard, which is why it may carry a string server id that the wire
/// has no field for: on the wire the connection is the address, and here one
/// webview holds several.
///
/// Addressed for the same reason `ConversationEvent` is. A phone can hold one
/// machine's screen while another machine's tree changes, and an event that did
/// not say which machine it belonged to would be applied to whichever happened
/// to be open — which looks exactly like that machine changing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct MachineEvent {
    pub server: String,
    pub event: WatchEvent,
}
