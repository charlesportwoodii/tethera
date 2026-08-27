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

        match kind {
            Some(kind) => BackendError::NotFound { kind },
            None => BackendError::Backend {
                message: format!("herdr {}: {}", self.code, self.message),
            },
        }
    }
}
