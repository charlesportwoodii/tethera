use serde::Deserialize;
use crate::backend::error::BackendError;
use tethera_common::protocol::error::EntityKind;

/// The body of a herdr error envelope.
///
/// herdr writes this to stdout, not stderr, and exits 1.
#[derive(Debug, Clone, Deserialize)]
pub struct Failure {
    pub code: String,
    pub message: String,
}

impl Failure {
    pub const WORKSPACE_NOT_FOUND: &'static str = "workspace_not_found";
    pub const TAB_NOT_FOUND: &'static str = "tab_not_found";
    pub const PANE_NOT_FOUND: &'static str = "pane_not_found";
    pub const AGENT_PANE_BUSY: &'static str = "agent_pane_busy";

    /// The codes that name a missing entity keep their kind, because a caller
    /// distinguishes "this pane is gone" from "the backend broke". Everything
    /// else is opaque and carries herdr's own message.
    pub fn into_backend(self) -> BackendError {
        let kind = match self.code.as_str() {
            Self::WORKSPACE_NOT_FOUND => Some(EntityKind::Workspace),
            Self::TAB_NOT_FOUND => Some(EntityKind::Tab),
            Self::PANE_NOT_FOUND => Some(EntityKind::Pane),
            _ => None,
        };

        let message = format!("herdr {}: {}", self.code, self.message);

        match kind {
            Some(kind) => BackendError::NotFound { kind },
            // Kept apart from `Backend` because a caller routes around this one.
            // The code covers both "the shell is busy" and "that is not a shell
            // I know", and a wrapped pane is always the second.
            None if self.code == Self::AGENT_PANE_BUSY => {
                BackendError::NotStartable { message }
            }
            None => BackendError::Backend { message },
        }
    }
}
