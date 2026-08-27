use crate::structs::ids::{AssetId, ConversationId, TabId};
use crate::structs::primitives::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which files to list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum AssetScope {
    Conversation(ConversationId),
    Tab(TabId),
}

/// One file the server's scan has seen.
///
/// The id is opaque and server-issued, which is what makes every id the client
/// holds resolvable. A client that could mint one would produce references the
/// scan has never seen, and those cannot be fetched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct AssetCard {
    pub asset: AssetId,
    pub name: String,
    pub mime: Option<String>,
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    pub modified: Option<Timestamp>,
}
