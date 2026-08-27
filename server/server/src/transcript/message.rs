use super::{MessageBody, Usage};
use serde::Deserialize;

/// The model-facing message an agent record wraps.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Message {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: MessageBody,
    /// What the model was sent and produced. Present on an assistant record and
    /// on nothing else.
    #[serde(default)]
    pub usage: Option<Usage>,
    /// Display only. The wire carries it so a person can see which model is
    /// working; nothing branches on it.
    #[serde(default)]
    pub model: Option<String>,
}
