use serde::Deserialize;

/// One tab, as herdr reports it.
///
/// `number` is herdr's own ordinal and is what a person means by `2:build`. It
/// is not the position of this record in the list, and the two disagree as soon
/// as a tab closes.
#[derive(Debug, Clone, Deserialize)]
pub struct TabInfo {
    pub tab_id: String,
    pub workspace_id: String,
    pub number: u64,
    pub label: String,
    pub focused: bool,
    pub pane_count: u64,
    #[serde(default)]
    pub agent_status: Option<String>,
}
