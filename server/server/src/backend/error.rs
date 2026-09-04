use tethera_common::protocol::error::{EntityKind, WireError};

/// How a terminal backend fails, classified.
///
/// `TerminalBackendTrait` returns `anyhow::Result`, which carries no structure,
/// and a port above it has to answer `NotFound` for a pane that is gone rather
/// than `Backend` for everything. This travels inside the `anyhow::Error` and
/// is recovered by downcast at the port, so the trait keeps its shape and the
/// classification is not lost.
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("no such {kind:?} on this machine")]
    NotFound { kind: EntityKind },
    /// The link to the backend is serialised and something is holding it.
    #[error("the terminal backend is busy")]
    Busy,
    #[error("{message}")]
    Backend { message: String },
    /// The multiplexer will not start an agent in this pane.
    ///
    /// Its own variant because the cause is ambiguous and the two readings need
    /// different handling. herdr reports `agent_pane_busy` both for a pane whose
    /// shell is running something and for a pane whose shell it does not
    /// recognise — which includes every pane the shim wraps. One is a refusal to
    /// respect, the other is a refusal to route around, and only the caller with
    /// the pane in front of it can tell them apart.
    #[error("{message}")]
    NotStartable { message: String },
}

impl BackendError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Backend {
            message: message.into(),
        }
    }

    pub fn into_wire(self) -> WireError {
        match self {
            Self::NotFound { kind } => WireError::NotFound { kind },
            Self::Busy => WireError::Busy,
            Self::Backend { message } => WireError::Backend { message },
            Self::NotStartable { message } => WireError::Backend { message },
        }
    }

    /// What a port reports for an error that arrived without a classification.
    ///
    /// An unclassified failure is a real failure, so it becomes `Backend` with
    /// its own message rather than being flattened into a not-found that would
    /// make a broken backend look like an empty one.
    pub fn classify(error: anyhow::Error) -> WireError {
        match error.downcast::<Self>() {
            Ok(backend) => backend.into_wire(),
            Err(other) => WireError::Backend {
                message: other.to_string(),
            },
        }
    }
}
