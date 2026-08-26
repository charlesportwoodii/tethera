mod part;

pub use part::Part;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct TranscriptEntry {
    pub id: String,
    pub parts: Vec<Part>,
}

impl TranscriptEntry {
    pub fn new(id: String, parts: Vec<Part>) -> Self {
        Self { id, parts }
    }

    pub fn unparsed(kind: &str, raw: &str) -> Vec<Self> {
        if raw.is_empty() {
            return Vec::new();
        }

        vec![Self::new(
            String::new(),
            vec![Part::Unknown {
                kind: kind.to_string(),
                fallback_text: raw.to_string(),
            }],
        )]
    }
}
