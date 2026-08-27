use super::{AttachmentOrigin, MessageBody};
use serde::Deserialize;

/// Context the harness injected into a turn.
///
/// Almost all of it is machine chatter - skill bodies, tool listings, hook
/// output, the token reminder - and the type is dropped wholesale for that
/// reason. One shape is not: a message the person typed while the agent was
/// still working is queued and recorded here rather than as a `user` record.
/// Measured across 224 files, 393 of those carry `origin.kind: "human"`, and
/// 392 of them appear nowhere else in the file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Attachment {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub prompt: MessageBody,
    #[serde(default)]
    pub origin: Option<AttachmentOrigin>,
}

impl Attachment {
    /// The only attachment shape that is a person speaking.
    pub const QUEUED: &'static str = "queued_command";

    /// Whether this is a message the person typed mid-turn.
    ///
    /// An absent origin is not a claim of humanity: older records carry none,
    /// and treating them as the person would put a peer agent's message under
    /// the operator's name.
    pub fn is_queued_by_a_person(&self) -> bool {
        self.kind == Self::QUEUED
            && self
                .origin
                .as_ref()
                .is_some_and(AttachmentOrigin::is_person)
    }

    pub fn text(&self) -> Option<String> {
        self.prompt.joined()
    }
}
