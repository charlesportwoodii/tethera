use crate::structs::ids::{ConversationId, TabId, WorkspaceId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One tab.
///
/// `conversation` and `foreground_command` are denormalisations of facts that
/// live on `Pane`. Without them the client issues a `ListPanes` per tab to draw
/// a tab row, which is a request storm on a phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Tab {
    pub id: TabId,
    pub workspace_id: WorkspaceId,
    /// The backend's own ordinal. A person calls this tab `2:build`, so an index
    /// assigned by list position would renumber when a tab closes.
    pub index: u16,
    pub title: String,
    pub conversation: Option<ConversationId>,
    /// Of the tab's primary pane. A split tab still draws one row.
    pub foreground_command: Option<String>,
}

impl Tab {
    pub fn new(id: TabId, workspace_id: WorkspaceId, index: u16, title: String) -> Self {
        Self {
            id,
            workspace_id,
            index,
            title,
            conversation: None,
            foreground_command: None,
        }
    }
}
