use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Pane {
    pub id: String,
    pub tab_id: String,
    pub label: String,
    pub cwd: String,
    pub focused: bool,
}

impl Pane {
    pub fn new(id: String, tab_id: String, label: String, cwd: String) -> Self {
        Self {
            id,
            tab_id,
            label,
            cwd,
            focused: false,
        }
    }
}
