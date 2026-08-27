use crate::protocol::watch::WatchEvent;
use crate::structs::ids::ConversationId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One watch event, addressed.
///
/// The conversation is carried rather than encoded into the event channel's
/// name. A phone can hold a screen while a notification arrives for a different
/// conversation, and an event that did not say which one it belonged to would be
/// applied to whatever happened to be open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct ConversationEvent {
    pub conversation: ConversationId,
    pub event: WatchEvent,
}
