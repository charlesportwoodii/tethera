use std::path::{Path, PathBuf};

/// Where an agent keeps its own record of a session.
///
/// No `TS` derive: this is server-side behaviour. The client learns the only
/// part of it that concerns it through `AgentProfile::provides_transcript`.
///
/// `Absent` is a real answer, not a failure. A pane whose agent has no readable
/// records has no conversation surface at all, and the client offers its
/// terminal instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptSource {
    /// One file, one JSON record per line, appended to while the session runs.
    JsonLines { path: PathBuf },
    Absent,
}

impl TranscriptSource {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::JsonLines { path } => Some(path),
            Self::Absent => None,
        }
    }

    pub fn is_readable(&self) -> bool {
        matches!(self, Self::JsonLines { .. })
    }
}
