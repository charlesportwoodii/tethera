use crate::structs::ids::AssetId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A file that has reached the machine but not yet the agent.
///
/// The distinction is the whole point. An upload stages bytes and answers an id;
/// the id becomes an attachment only when a prompt is sent carrying it. So a
/// chip on a composer holding one of these is armed, not delivered, and a file
/// picked and never sent is an orphan the machine clears up rather than
/// something a person has to cancel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Attached {
    pub asset: AssetId,
    pub name: String,
    #[ts(type = "number")]
    pub size: u64,
}
