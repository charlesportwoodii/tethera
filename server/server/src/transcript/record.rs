use super::{Attachment, ContentBlock, Message};
use serde::Deserialize;
use tethera_common::structs::agent::CommandTags;
use tethera_common::structs::primitives::Timestamp;
use tethera_common::structs::transcript::Role;

/// One line of an agent's own JSONL record of a session.
///
/// Only the fields a turn is built from are named. Everything else on the line -
/// token accounting, git branch, request ids, the several bookkeeping record
/// types - is left in the JSON and never read, so a new field upstream is not a
/// parse failure here.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Record {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(rename = "sessionId", default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub message: Option<Message>,
    #[serde(rename = "isMeta", default)]
    pub is_meta: bool,
    #[serde(rename = "isSidechain", default)]
    pub is_sidechain: bool,
    /// Groups the records of one model response. The harness writes each content
    /// block as its own record, so this is what rejoins them.
    #[serde(rename = "requestId", default)]
    pub request_id: Option<String>,
    /// The harness's own summary of a context it discarded.
    #[serde(rename = "isCompactSummary", default)]
    pub is_compact_summary: bool,
    /// `typed` and `queued` are a person. `system` is not. Absent on nothing in
    /// the measured sample, but absent is possible, which is why the noise
    /// filter runs as well.
    #[serde(rename = "promptSource", default)]
    pub prompt_source: Option<String>,
    /// Present when the person stopped the agent.
    #[serde(rename = "interruptedMessageId", default)]
    pub interrupted_message_id: Option<String>,
    /// The shape varies per tool, so only the mapper reads into it.
    #[serde(rename = "toolUseResult", default)]
    pub tool_use_result: Option<serde_json::Value>,
    /// A system record's body, which sits at the top level rather than under
    /// `message`. A slash command's own line arrives this way and nowhere else.
    #[serde(default)]
    pub content: Option<String>,
    #[serde(rename = "aiTitle", default)]
    pub ai_title: Option<String>,
    /// The name a person gave the session with the harness's own rename
    /// command, on a record of its own kind.
    ///
    /// A separate field rather than a second spelling of the same one, because
    /// the two coexist in one file and mean different things: the harness titles
    /// a session from its first turn and never revises it, so an `aiTitle`
    /// written before a rename would otherwise keep winning on recency.
    #[serde(rename = "customTitle", default)]
    pub custom_title: Option<String>,
    #[serde(default)]
    pub subtype: Option<String>,
    /// Injected context. Dropped wholesale but for one shape - see
    /// `Attachment::is_queued_by_a_person`.
    #[serde(default)]
    pub attachment: Option<Attachment>,
}

impl Record {
    /// What the model was sent and produced for this record.
    ///
    /// Empty rather than absent when the record carried none, so a caller can
    /// tell "no usage here" from "a request that used nothing" without an
    /// `Option` at every call site.
    pub fn usage(&self) -> super::Usage {
        self.message
            .as_ref()
            .and_then(|message| message.usage)
            .unwrap_or_default()
    }

    /// Which model produced this record.
    pub fn model(&self) -> Option<String> {
        self.message
            .as_ref()
            .and_then(|message| message.model.clone())
    }

    pub const ASSISTANT: &'static str = "assistant";
    pub const USER: &'static str = "user";
    pub const ATTACHMENT: &'static str = "attachment";

    pub fn is_assistant(&self) -> bool {
        self.kind == Self::ASSISTANT
    }

    pub fn is_user(&self) -> bool {
        self.kind == Self::USER
    }

    /// Whether this record could become a turn at all.
    ///
    /// `attachment`, `system` and the dozen bookkeeping types are excluded here
    /// rather than deeper in, because they are the bulk of the file - measured
    /// at roughly half of every line - and inspecting their text would be work
    /// spent on content nobody typed.
    pub fn is_turn_candidate(&self, tags: Option<&CommandTags>) -> bool {
        self.is_assistant()
            || self.is_user()
            || self.is_queued_prompt()
            || self.is_local_command(tags)
    }

    /// The record the harness writes when a person runs a slash command.
    ///
    /// Its body sits at the top level rather than under `message`, which is why
    /// it reached none of the paths that read a message: the command a person
    /// ran was not in the transcript at all, and what showed instead was its
    /// output with no sign of what had produced it.
    ///
    /// **Which kind of record that is belongs to the harness**, so it arrives as
    /// a table rather than being spelled here. An agent nobody has measured has
    /// no table, and none of its records are read as commands.
    pub fn is_local_command(&self, tags: Option<&CommandTags>) -> bool {
        tags.is_some_and(|tags| {
            self.kind == tags.record_kind && self.subtype.as_deref() == Some(tags.record_subtype)
        })
    }

    /// A message the person typed while the agent was still working.
    ///
    /// It arrives as an `attachment` rather than a `user` record, and measured
    /// across 224 files it appears nowhere else - so dropping the attachment
    /// type wholesale loses the person's words, which is the failure the noise
    /// filter exists to prevent, inverted.
    pub fn is_queued_prompt(&self) -> bool {
        self.kind == Self::ATTACHMENT
            && self
                .attachment
                .as_ref()
                .is_some_and(Attachment::is_queued_by_a_person)
    }

    /// The prompt a queued attachment carries.
    pub fn queued_text(&self) -> Option<String> {
        self.attachment
            .as_ref()
            .filter(|attachment| attachment.is_queued_by_a_person())
            .and_then(Attachment::text)
    }

    /// The sentence the harness writes when it carries no field for an
    /// interrupt.
    pub const INTERRUPT_TEXT: &'static str = "[Request interrupted by user]";

    /// Whether the person stopped the agent.
    ///
    /// Two detections because the harness records it two ways: 179 records
    /// carry the field naming the message they stopped, and 46 carry nothing but
    /// the sentence. Neither alone finds them all.
    pub fn interrupted(&self) -> bool {
        if self.interrupted_message_id.is_some() {
            return true;
        }

        self.text_of_record()
            .is_some_and(|text| text.trim() == Self::INTERRUPT_TEXT)
    }

    /// The record's text, whichever of the two message shapes carried it.
    pub fn text_of_record(&self) -> Option<&str> {
        if let Some(text) = self.plain_text() {
            return Some(text);
        }

        self.blocks().iter().find_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
    }

    pub fn role(&self, tags: Option<&CommandTags>) -> Option<Role> {
        if self.is_assistant() {
            Some(Role::Agent)
        } else if self.is_user() || self.is_queued_prompt() || self.is_local_command(tags) {
            Some(Role::Operator)
        } else {
            None
        }
    }

    /// The record's own timestamp as epoch milliseconds.
    pub fn at(&self) -> Option<Timestamp> {
        let raw = self.timestamp.as_deref()?;

        chrono::DateTime::parse_from_rfc3339(raw)
            .ok()
            .map(|moment| Timestamp(moment.timestamp_millis()))
    }

    pub fn blocks(&self) -> &[ContentBlock] {
        self.message
            .as_ref()
            .map(|message| message.content.blocks())
            .unwrap_or(&[])
    }

    pub fn plain_text(&self) -> Option<&str> {
        self.message
            .as_ref()
            .and_then(|message| message.content.text())
    }

    /// Whether this record carries a tool's answer rather than a person's.
    pub fn carries_tool_results(&self) -> bool {
        self.tool_use_result.is_some()
            || self
                .blocks()
                .iter()
                .any(|block| matches!(block, ContentBlock::ToolResult { .. }))
    }

    /// The tool call ids this record answers.
    pub fn answered_tool_ids(&self) -> Vec<String> {
        self.blocks()
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.clone()),
                _ => None,
            })
            .collect()
    }
}
