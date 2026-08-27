use super::ContentBlock;
use serde::Deserialize;

/// A message's content, which is a bare string for a typed prompt and a list of
/// blocks for everything else.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum MessageBody {
    Blocks(Vec<ContentBlock>),
    Text(String),
    /// Neither shape. Absent rather than an error: a record whose content this
    /// build cannot read still has a timestamp and a role worth keeping.
    Unreadable(serde_json::Value),
}

impl Default for MessageBody {
    fn default() -> Self {
        Self::Blocks(Vec::new())
    }
}

impl MessageBody {
    pub fn blocks(&self) -> &[ContentBlock] {
        match self {
            Self::Blocks(blocks) => blocks,
            _ => &[],
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The body as one string, whichever shape carried it.
    ///
    /// A queued prompt is a bare string 385 times in 393 and a list of blocks
    /// the other 8, and both are the person typing.
    pub fn joined(&self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text.clone()),
            Self::Blocks(blocks) => {
                let text: Vec<String> = blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect();

                if text.is_empty() {
                    None
                } else {
                    Some(text.join("\n"))
                }
            }
            Self::Unreadable(_) => None,
        }
    }
}
