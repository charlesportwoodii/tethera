use crate::structs::ids::AssetId;
use crate::structs::primitives::Sha256;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Opens a download stream.
///
/// After the server's `FetchHead`, the remainder of the stream is raw bytes to
/// FIN - not framed, so bulk transfer pays no per-chunk overhead and the control
/// frame cap never constrains a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct FetchSpec {
    pub asset: AssetId,
    /// Where to resume from. Zero for a fresh transfer.
    #[ts(type = "number")]
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct FetchHead {
    /// The total size of the asset, not the number of bytes about to be sent.
    /// A client reading this as "bytes remaining" draws a progress bar that
    /// finishes early on every resumed transfer.
    #[ts(type = "number")]
    pub len: u64,
    pub mime: Option<String>,
    pub sha256: Sha256,
    /// Where the server actually starts, which the client believes over the
    /// offset it asked for.
    #[ts(type = "number")]
    pub offset: u64,
}

/// Opens an upload stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct PutSpec {
    pub name: String,
    #[ts(type = "number")]
    pub len: u64,
    pub sha256: Sha256,
    /// The offset the client would like to resume from. A proposal, not a
    /// decision.
    #[ts(type = "number")]
    pub offset: u64,
}

/// The server's answer to a `PutSpec`, and what makes an upload resumable.
///
/// Only the server knows how much of a previous attempt reached disk, so the
/// client seeks to this offset rather than to the one it proposed. Without this
/// frame a resumed upload is the client guessing, which corrupts the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct PutReady {
    #[ts(type = "number")]
    pub offset: u64,
}

/// The id the upload became, which a prompt then references as an attachment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct PutResult {
    pub asset: AssetId,
}
