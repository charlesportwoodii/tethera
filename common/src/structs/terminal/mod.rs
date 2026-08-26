mod pane;
mod tab;

pub use pane::Pane;
pub use tab::Tab;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Workspace {
    pub id: String,
    pub name: String,
}

impl Workspace {
    pub fn new(id: String, name: String) -> Self {
        Self { id, name }
    }
}
