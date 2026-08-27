use crate::protocol::WireVersion;
use crate::structs::ids::AssetId;
use crate::structs::transcript::question::{AnswerRecord, Question};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum ToolStatus {
    Running,
    Ok,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct TodoItem {
    pub text: String,
    pub status: TodoStatus,
}

/// One normalised piece of a turn.
///
/// The set is closed. Every variant except `Text` carries `fallback_text` - the
/// source rows verbatim - which is what a server emits inside `Unknown` when the
/// negotiated version cannot carry the real variant.
///
/// Only the agent's own records are represented. There is deliberately no tier
/// that infers structure from a screen: a conversation built on a guess at
/// structure is worse than the terminal it was guessed from, so a pane with no
/// readable transcript has no conversation surface at all and the client offers
/// its terminal instead.
///
/// Never add a serde tag attribute here. An internally-tagged enum needs
/// `deserialize_any`, which postcard does not implement: it compiles and then
/// fails at runtime the first time a transcript crosses the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum Part {
    Text {
        text: String,
    },
    ToolUse {
        name: String,
        input: String,
        result: Option<String>,
        status: ToolStatus,
        fallback_text: String,
    },
    Diff {
        path: String,
        unified: String,
        #[ts(type = "number | null")]
        added: Option<u32>,
        #[ts(type = "number | null")]
        removed: Option<u32>,
        fallback_text: String,
    },
    Todo {
        items: Vec<TodoItem>,
        fallback_text: String,
    },
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        fallback_text: String,
    },
    Status {
        label: String,
        detail: Option<String>,
        fallback_text: String,
    },
    File {
        asset: AssetId,
        name: String,
        mime: Option<String>,
        #[ts(type = "number | null")]
        size: Option<u64>,
        fallback_text: String,
    },
    // Produced by the sender, never recovered by the receiver. postcard encodes
    // variants by index and is not self-describing, so a decoder meeting an
    // unknown index fails outright.
    Unknown {
        kind: String,
        fallback_text: String,
    },
    Question {
        question: Question,
        answered: Option<AnswerRecord>,
        fallback_text: String,
    },
}

impl Part {
    /// The variant's own name, which is what a downgrade preserves.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Text { .. } => "text",
            Self::ToolUse { .. } => "tool_use",
            Self::Diff { .. } => "diff",
            Self::Todo { .. } => "todo",
            Self::Table { .. } => "table",
            Self::Status { .. } => "status",
            Self::File { .. } => "file",
            Self::Unknown { .. } => "unknown",
            Self::Question { .. } => "question",
        }
    }

    /// The wire version this variant was introduced at.
    ///
    /// A table rather than ad-hoc code at each emit site, so adding a variant
    /// means adding a row and nothing else can forget to handle it.
    pub fn since(&self) -> WireVersion {
        match self {
            Self::Text { .. }
            | Self::ToolUse { .. }
            | Self::Diff { .. }
            | Self::Todo { .. }
            | Self::Table { .. }
            | Self::Status { .. }
            | Self::File { .. }
            | Self::Unknown { .. }
            | Self::Question { .. } => WireVersion(1),
        }
    }

    pub fn fallback_text(&self) -> &str {
        match self {
            Self::Text { text } => text,
            Self::ToolUse { fallback_text, .. }
            | Self::Diff { fallback_text, .. }
            | Self::Todo { fallback_text, .. }
            | Self::Table { fallback_text, .. }
            | Self::Status { fallback_text, .. }
            | Self::File { fallback_text, .. }
            | Self::Unknown { fallback_text, .. }
            | Self::Question { fallback_text, .. } => fallback_text,
        }
    }

    /// This part as the negotiated version can carry it.
    ///
    /// The server calls this on the way out. The client never calls it: a client
    /// that tried to downgrade would be guessing at what its peer meant.
    pub fn for_version(&self, version: WireVersion) -> Part {
        if version >= self.since() || matches!(self, Self::Unknown { .. }) {
            return self.clone();
        }

        Part::Unknown {
            kind: self.kind().to_string(),
            fallback_text: self.fallback_text().to_owned(),
        }
    }
}
