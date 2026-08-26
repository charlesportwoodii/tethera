use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Tab {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
}

impl Tab {
    pub fn new(id: String, workspace_id: String, title: String) -> Self {
        Self {
            id,
            workspace_id,
            title,
        }
    }
}
