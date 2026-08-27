use super::{ContentBlock, Record};
use tethera_common::structs::primitives::Timestamp;

/// What one tool call answered.
///
/// Assembled from two places in the same record: the `tool_result` block, which
/// carries the text and the error flag, and `toolUseResult`, which carries the
/// structure - a patch, an answer, a list of attachments - that the block does
/// not.
#[derive(Debug, Clone, Default)]
pub struct ToolOutcome {
    pub text: Option<String>,
    pub is_error: bool,
    pub detail: Option<serde_json::Value>,
    /// When the answer arrived, which is what an `AnswerRecord` needs.
    pub at: Option<Timestamp>,
}

impl ToolOutcome {
    /// Every outcome a record carries, paired with the call it answers.
    pub fn from_record(record: &Record) -> Vec<(String, Self)> {
        let at = record.at();
        let detail = record.tool_use_result.clone();

        record
            .blocks()
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => Some((
                    tool_use_id.clone(),
                    Self {
                        text: ContentBlock::result_text(content),
                        is_error: *is_error,
                        detail: detail.clone(),
                        at,
                    },
                )),
                _ => None,
            })
            .collect()
    }

    pub fn field(&self, name: &str) -> Option<&serde_json::Value> {
        self.detail.as_ref().and_then(|detail| detail.get(name))
    }
}
