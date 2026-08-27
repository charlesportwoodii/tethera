use crate::structs::ids::AssetId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How a download is getting on, while it is getting on.
///
/// Emitted from the moment a transfer is asked for rather than from the moment
/// bytes move. The gap between those two is not small: the machine hashes the
/// whole asset before it writes the head, which on a four hundred megabyte file
/// is most of a second, and it is exactly the moment a person decides the app
/// has stopped responding. A row drawn at `Opening`, with no bar, says
/// "started" without claiming to know how far.
///
/// `received` counts the whole file on disk, including whatever a previous
/// attempt left. A bar drawn from what crossed the wire this time restarts at
/// zero on every resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct DownloadProgress {
    /// This attempt, not the asset. A person may fetch the same file twice.
    pub id: String,
    pub asset: AssetId,
    pub name: String,
    #[ts(type = "number")]
    pub received: u64,
    /// The whole asset. Zero until the machine's head arrives, which is what
    /// tells a screen to draw an indeterminate row rather than a full bar.
    #[ts(type = "number")]
    pub total: u64,
    pub state: DownloadState,
    /// Where the file ended up, once it did.
    pub saved_to: Option<String>,
    pub failure: Option<String>,
}

/// What a download is doing.
///
/// `Paused` is the one that earns its place. A phone that switches apps loses
/// its connection mid-transfer, and the bytes already on disk are kept - so the
/// honest report is not a failure but an interruption that is being retried.
/// Reporting it as failed teaches a person to start over, which is the one
/// action that throws those bytes away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum DownloadState {
    /// Asked for, and the machine has not answered with a head yet.
    Opening,
    Running,
    /// Interrupted with bytes kept, and being retried.
    Paused,
    Done,
    Failed,
    Cancelled,
}
