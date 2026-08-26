use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum Part {
    Text {
        text: String,
    },
    ToolUse {
        name: String,
        input: String,
        fallback_text: String,
    },
    Question {
        prompt: String,
        options: Vec<String>,
        fallback_text: String,
    },
    File {
        name: String,
        size: u64,
        fallback_text: String,
    },
    // Produced by the sender, never recovered by the receiver. postcard
    // encodes variants by index and is not self-describing, so a decoder
    // meeting an unknown index fails outright.
    Unknown {
        kind: String,
        fallback_text: String,
    },
}

impl Part {
    pub fn fallback_text(&self) -> &str {
        match self {
            Self::Text { text } => text,
            Self::ToolUse { fallback_text, .. } => fallback_text,
            Self::Question { fallback_text, .. } => fallback_text,
            Self::File { fallback_text, .. } => fallback_text,
            Self::Unknown { fallback_text, .. } => fallback_text,
        }
    }
}
