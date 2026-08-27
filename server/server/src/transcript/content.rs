use serde::Deserialize;

/// One block inside an agent record's `message.content`.
///
/// This is a disk format read with `serde_json` and it never crosses the wire,
/// which is why an internally tagged enum is allowed here: the postcard rule
/// that forbids `serde(tag = ...)` applies to wire types, and this is not one.
///
/// `Other` exists because Claude Code adds block types between releases. An
/// unknown block becoming nothing is right; an unknown block failing the whole
/// page is not.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        #[serde(default)]
        text: String,
    },
    Thinking {
        #[serde(default)]
        thinking: String,
    },
    ToolUse {
        #[serde(default)]
        id: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        input: serde_json::Value,
    },
    ToolResult {
        #[serde(default)]
        tool_use_id: String,
        #[serde(default)]
        content: serde_json::Value,
        #[serde(default)]
        is_error: bool,
    },
    Image {
        #[serde(default)]
        source: serde_json::Value,
    },
    #[serde(other)]
    Other,
}

impl ContentBlock {
    /// The block's own name, which is what an unmappable block carries into
    /// `Part::Unknown`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Text { .. } => "text",
            Self::Thinking { .. } => "thinking",
            Self::ToolUse { .. } => "tool_use",
            Self::ToolResult { .. } => "tool_result",
            Self::Image { .. } => "image",
            Self::Other => "other",
        }
    }

    /// The text of a tool result, whichever of the two shapes it arrived in.
    ///
    /// A result is a string most of the time and a list of text blocks the rest
    /// of the time, and nothing upstream says which to expect.
    pub fn result_text(content: &serde_json::Value) -> Option<String> {
        match content {
            serde_json::Value::String(text) => Some(text.clone()),
            serde_json::Value::Array(blocks) => {
                let joined: Vec<String> = blocks
                    .iter()
                    .filter_map(|block| {
                        block
                            .get("text")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                    })
                    .collect();

                if joined.is_empty() {
                    None
                } else {
                    Some(joined.join("\n"))
                }
            }
            _ => None,
        }
    }
}
