use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The head of a file, enough to show it without pulling the whole thing.
///
/// A phone must never fetch a five gigabyte dump to find out it is a log. The
/// fetch stream is dropped once this much has arrived, which is why `truncated`
/// exists: a long file read short is otherwise indistinguishable from a short
/// file, and a reader who cannot tell will believe they have seen all of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct AssetPreview {
    pub mime: Option<String>,
    /// The whole asset's size, not the number of bytes carried here.
    #[ts(type = "number | null")]
    pub len: Option<u64>,
    /// Decoded as UTF-8 at this boundary rather than in a component. A
    /// component holding a decoder owns an encoding bug.
    pub text: Option<String>,
    /// A `data:` URL, for the one kind that cannot be shown as text.
    ///
    /// An image has to arrive whole to decode, so this is absent for anything
    /// large enough that a phone should not be asked to hold it.
    pub image_data_url: Option<String>,
    pub truncated: bool,
}
