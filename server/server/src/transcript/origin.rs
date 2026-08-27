use serde::Deserialize;

/// Who put a queued message in the queue.
///
/// The person is not the only thing that enqueues one: an auto-continuation and
/// a message from a peer agent arrive the same way and are not the person
/// speaking.
#[derive(Debug, Clone, Deserialize)]
pub struct AttachmentOrigin {
    #[serde(default)]
    pub kind: Option<String>,
}

impl AttachmentOrigin {
    pub const HUMAN: &'static str = "human";

    pub fn is_person(&self) -> bool {
        self.kind.as_deref() == Some(Self::HUMAN)
    }
}
