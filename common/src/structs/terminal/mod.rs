mod pane;
mod size;
mod tab;

pub use pane::Pane;
pub use size::{Size, SplitDirection};
pub use tab::Tab;

use crate::structs::ids::{ConversationId, WorkspaceId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub cwd: Option<String>,
    /// So a collapsed row can say "2 tabs" without fetching them.
    pub tab_count: u16,
    /// The agent this workspace exists for, when it has one.
    pub conversation: Option<ConversationId>,
}

impl Workspace {
    pub fn new(id: WorkspaceId, name: String) -> Self {
        Self {
            id,
            name,
            cwd: None,
            tab_count: 0,
            conversation: None,
        }
    }
}
