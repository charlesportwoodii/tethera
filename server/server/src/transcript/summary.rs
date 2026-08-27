use std::path::PathBuf;
use tethera_common::structs::primitives::Timestamp;

/// What one session says about itself without reading all of it.
///
/// Assembled from a bounded read of the file's head and tail. One session on the
/// development machine is 57.5 MB, and a home screen that read every session
/// whole would cost more than the screen is worth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub session_id: String,
    pub path: PathBuf,
    pub cwd: Option<String>,
    pub started_at: Option<Timestamp>,
    pub last_active: Option<Timestamp>,
    pub title: Option<String>,
    /// One line of the most recent meaningful text.
    pub preview: Option<String>,
}
