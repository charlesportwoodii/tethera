use super::{
    AssetIndex, AssetNaming, ContentBlock, MarkdownTables, Record, Segment, SentFiles,
    SlashCommand, ToolOutcome,
};
use std::sync::Arc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tethera_common::structs::agent::{Agent, CommandTags};
use tethera_common::structs::ids::{QuestionId, TurnId};
use tethera_common::structs::primitives::{Cursor, Timestamp};
use tethera_common::structs::transcript::{
    Answer, AnswerRecord, Ask, Part, Question, QuestionOption, Role, TodoItem, TodoStatus,
    ToolStatus,
    Turn,
};
use tethera_common::traits::AgentTrait;

/// One agent's records, as turns.
///
/// Every difference between two agents is read off the agent's tables, so this
/// file holds no agent's name.
pub struct TurnMapper {
    agent: Agent,
    /// Where the files this mapper names actually live.
    ///
    /// Written to, never read. The id it puts on a `Part::File` is a one-way
    /// hash, so unless the way back is recorded as the record is read, nothing
    /// can ever open the card that id produces — and the path exists nowhere
    /// else. Registering is not a filesystem read and changes no output, which
    /// is what keeps this a pure function of its records.
    assets: Option<Arc<AssetIndex>>,
    /// This machine's own upload directory.
    ///
    /// The anchor that tells a line this machine wrote from a line a person
    /// typed. Absent means no prompt is searched at all, which is what a mapper
    /// with no machine behind it should do.
    uploads: Option<PathBuf>,
}

impl TurnMapper {
    pub fn new(agent: Agent) -> Self {
        Self {
            agent,
            assets: None,
            uploads: None,
        }
    }

    /// The same mapper, recording where each file it names can be found.
    pub fn indexing(agent: Agent, assets: Arc<AssetIndex>) -> Self {
        Self {
            agent,
            assets: Some(assets),
            uploads: None,
        }
    }

    /// The same mapper, able to recognise the files this machine stored.
    ///
    /// Without the directory a person's own attachments stay raw paths inside
    /// their message: there is no way to tell a line this machine wrote from
    /// one they typed, and guessing would eat their words.
    pub fn from_store(mut self, uploads: PathBuf) -> Self {
        self.uploads = Some(uploads);

        self
    }

    pub fn agent(&self) -> Agent {
        self.agent
    }

    /// How this harness records a command, if anybody has measured it.
    fn commands(&self) -> Option<&'static CommandTags> {
        self.agent.command_tags()
    }

    /// Whether this record can contribute anything to a turn.
    ///
    /// The index calls this so that what it counts and what a page returns are
    /// the same set. An index that counted a record the mapper then dropped
    /// would hand back short pages and a `has_earlier` about entries rather
    /// than about turns.
    pub fn yields_turn(&self, record: &Record) -> bool {
        if !record.is_turn_candidate(self.commands()) {
            return false;
        }

        // A subagent's conversation is its own, and threading it into the
        // person's would interleave two dialogues that never addressed each
        // other.
        if record.is_sidechain {
            return false;
        }

        if record.is_assistant() {
            return record.blocks().iter().any(|block| match block {
                ContentBlock::Text { text } => !text.trim().is_empty(),
                ContentBlock::ToolUse { .. } | ContentBlock::Image { .. } => true,
                _ => false,
            });
        }

        self.person_spoke(record)
    }

    /// Whether a `user` record is the person and not the harness wearing their
    /// role.
    fn person_spoke(&self, record: &Record) -> bool {
        // A command a person ran is something they did, and it carries its own
        // body rather than a message. It has to clear this gate before any of
        // the tests below reach for a `message` it does not have.
        if let Some(tags) = self.commands().filter(|_| record.is_local_command(self.commands())) {
            return record
                .content
                .as_deref()
                .and_then(|text| SlashCommand::spoken(tags, text))
                .is_some();
        }

        if record.carries_tool_results() || record.is_meta {
            return false;
        }

        // Typed while the agent was working. It reaches the file as an
        // attachment rather than a user record, and it is still the person.
        if let Some(queued) = record.queued_text() {
            return !self.agent.noise_filter().is_noise(&queued);
        }

        // The seam where the harness replaced its own context. Not the person,
        // but not nothing either: `turn` renders it as a marker, so it has to
        // survive this gate to reach that.
        if record.is_compact_summary {
            return true;
        }

        if record.interrupted() {
            return true;
        }

        if record.prompt_source.as_deref() == Some(Self::INJECTED_SOURCE) {
            return false;
        }

        let filter = self.agent.noise_filter();

        if let Some(text) = record.plain_text() {
            return !filter.is_noise(text);
        }

        record.blocks().iter().any(|block| match block {
            ContentBlock::Text { text } => !filter.is_noise(text),
            ContentBlock::Image { .. } => true,
            _ => false,
        })
    }

    /// `promptSource` when the harness spoke rather than the person.
    const INJECTED_SOURCE: &'static str = "system";

    /// One turn from one group of records.
    ///
    /// A group is one model response: the harness writes each content block as
    /// its own record, and they are rejoined here so a response with a sentence
    /// and a tool call is one turn with two parts rather than two turns.
    pub fn turn(
        &self,
        group: &[Record],
        cursor: Cursor,
        outcomes: &HashMap<String, ToolOutcome>,
    ) -> Option<Turn> {
        let first = group.first()?;
        let role = first.role(self.commands())?;
        let id = TurnId(first.uuid.clone()?);
        let at = first.at().unwrap_or(Timestamp(0));

        // The seam a person scrolling back would otherwise cross with no
        // indication: the agent's context was replaced here. Rendered under the
        // agent's role because the harness wrote it, not the person, and its
        // body is dropped - it is a summary of what was discarded, not
        // something anybody said.
        if first.is_compact_summary {
            return Some(Turn::new(
                cursor,
                id,
                at,
                Role::Agent,
                vec![Self::status_part(Self::COMPACTED)],
            ));
        }

        let mut parts = Vec::new();

        for record in group {
            match role {
                Role::Agent => self.agent_parts(record, outcomes, &mut parts),
                Role::Operator => self.operator_parts(record, &mut parts),
            }
        }

        if parts.is_empty() {
            return None;
        }

        Some(Turn::new(cursor, id, at, role, parts))
    }

    fn agent_parts(
        &self,
        record: &Record,
        outcomes: &HashMap<String, ToolOutcome>,
        parts: &mut Vec<Part>,
    ) {
        for block in record.blocks() {
            match block {
                ContentBlock::Text { text } if !text.trim().is_empty() => {
                    Self::prose_parts(text, parts)
                }
                ContentBlock::ToolUse { id, name, input } => {
                    parts.extend(self.tool_parts(id, name, input, outcomes.get(id)))
                }
                ContentBlock::Image { source } => parts.push(Self::image_part(source)),
                // Reasoning has no variant in the closed part set, and inventing
                // one is a wire change rather than a mapping.
                _ => {}
            }
        }
    }

    /// Agent prose, with any table in it sent as a table.
    ///
    /// `Part::Table` and the client rendering for it both already existed and
    /// nothing produced one, so a table travelled inside a text part and drew as
    /// a paragraph of pipes with its newlines collapsed.
    fn prose_parts(text: &str, parts: &mut Vec<Part>) {
        for segment in MarkdownTables::split(text) {
            match segment {
                Segment::Prose(prose) if prose.trim().is_empty() => {}
                Segment::Prose(prose) => parts.push(Part::Text { text: prose }),
                Segment::Table {
                    columns,
                    rows,
                    source,
                } => parts.push(Part::Table {
                    columns,
                    rows,
                    // The markdown it came from. A peer that does not know this
                    // variant is sent the table as the text it always was,
                    // rather than nothing.
                    fallback_text: source,
                }),
            }
        }
    }

    /// A slash command as the person's own line, and what it printed.
    ///
    /// The output is a fold rather than more prose: `/context` prints a page,
    /// and a transcript that inlined every one of those would bury the
    /// conversation it belongs to.
    fn command_parts(tags: &CommandTags, text: &str, parts: &mut Vec<Part>) {
        if let Some(spoken) = SlashCommand::spoken(tags, text) {
            parts.push(Part::Text {
                text: spoken.clone(),
            });

            if let Some(printed) = SlashCommand::output(tags, text) {
                parts.push(Self::command_output(&spoken, printed));
            }

            return;
        }

        if let Some(printed) = SlashCommand::output(tags, text) {
            parts.push(Self::command_output("command output", printed));
        }
    }

    fn command_output(name: &str, printed: String) -> Part {
        Part::ToolUse {
            name: name.to_string(),
            input: String::new(),
            fallback_text: printed.clone(),
            result: Some(printed),
            status: ToolStatus::Ok,
        }
    }

    fn operator_parts(&self, record: &Record, parts: &mut Vec<Part>) {
        if let Some(tags) = self.commands().filter(|_| record.is_local_command(self.commands())) {
            if let Some(spoken) = record
                .content
                .as_deref()
                .and_then(|text| SlashCommand::spoken(tags, text))
            {
                parts.push(Part::Text { text: spoken });
            }

            return;
        }

        // The person stopped the agent. Dropping it leaves a response that ends
        // mid-sentence for no visible reason, so it is rendered rather than
        // filtered - and it does not return early, because a record can carry
        // the marker and the person's next words together.
        if record.interrupted() {
            parts.push(Self::status_part(Self::INTERRUPTED));
        }

        let filter = self.agent.noise_filter();

        if let Some(queued) = record.queued_text() {
            if !filter.is_noise(&queued) {
                self.spoken_parts(&queued, parts);
            }

            return;
        }

        if let Some(text) = record.plain_text() {
            // Before the filter, because the filter's job is to drop what the
            // harness wrote under this role and a slash command is the one
            // shape there that a person actually did.
            if let Some(tags) = self
                .commands()
                .filter(|tags| SlashCommand::is_command(tags, text))
            {
                Self::command_parts(tags, text, parts);

                return;
            }

            if !filter.is_noise(text) {
                self.spoken_parts(text, parts);
            }

            return;
        }

        for block in record.blocks() {
            match block {
                ContentBlock::Text { text } if !filter.is_noise(text) => {
                    parts.push(Part::Text { text: text.clone() })
                }
                ContentBlock::Image { source } => parts.push(Self::image_part(source)),
                _ => {}
            }
        }
    }

    /// What a person said, and the files they said it with.
    ///
    /// A file they sent reached the agent as a line of text inside their own
    /// prompt, so without this their half of the history is a raw absolute path
    /// where the agent's half is a card. They can see everything shared *with*
    /// them and nothing they shared themselves.
    ///
    /// Words first, then the cards. The order is what a client draws: the
    /// message in the bubble, the files under it. A turn that was only files
    /// yields only cards rather than an empty bubble above them.
    fn spoken_parts(&self, text: &str, parts: &mut Vec<Part>) {
        let Some(uploads) = &self.uploads else {
            parts.push(Part::Text { text: text.to_string() });

            return;
        };

        let (spoken, sent) = SentFiles::split(text, uploads);

        if !spoken.is_empty() {
            parts.push(Part::Text { text: spoken });
        }

        for path in sent {
            // The size is not in the record and is not read from disk: this
            // mapper makes no filesystem calls, and `FetchHead` carries the true
            // length at the one moment it matters, which is opening the card.
            parts.push(self.file_part(
                &path.to_string_lossy(),
                AssetNaming::mime_for(&path),
                None,
            ));
        }
    }

    const INTERRUPTED: &'static str = "Interrupted";
    const COMPACTED: &'static str = "Context compacted";

    fn status_part(label: &str) -> Part {
        Part::Status {
            label: label.to_string(),
            detail: None,
            fallback_text: label.to_string(),
        }
    }

    /// An image the person pasted.
    ///
    /// `Unknown` rather than a variant, and the base64 body is deliberately not
    /// carried: the part set has no image, and a megabyte of data URI on a
    /// control frame would be refused by the frame cap anyway.
    fn image_part(source: &serde_json::Value) -> Part {
        let media = source
            .get("media_type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("image");

        Part::Unknown {
            kind: "image".to_string(),
            fallback_text: format!("[{media}]"),
        }
    }

    fn tool_parts(
        &self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
        outcome: Option<&ToolOutcome>,
    ) -> Vec<Part> {
        // Each specialised mapping may decline - an empty patch, a call whose
        // result has not arrived with the fields it needs - and a declined
        // mapping falls back to the tool row rather than dropping the call.
        if self.agent.question_tools().contains(&name) {
            let parts = Self::question_parts(id, input, outcome);

            if !parts.is_empty() {
                return parts;
            }
        }

        // Every file this call names becomes fetchable, whatever the call was.
        // A card is still only made for a deliberate hand-over — an agent edits
        // constantly, and a card per edit buries the conversation in offers
        // nobody asked for — but "not worth a card" is a different judgement
        // from "not worth being able to open", and a person looking at a
        // conversation may well want the file it was about.
        self.note_files(input, outcome);

        if self.agent.file_push_tools().contains(&name) {
            let parts = self.file_parts(input, outcome);

            if !parts.is_empty() {
                return parts;
            }
        }

        if self.agent.diff_tools().contains(&name) {
            let parts = Self::diff_parts(input, outcome);

            if !parts.is_empty() {
                return parts;
            }
        }

        if self.agent.todo_tools().contains(&name) {
            if let Some(part) = Self::todo_part(input) {
                return vec![part];
            }
        }

        vec![Self::tool_use_part(name, input, outcome)]
    }

    fn tool_use_part(
        name: &str,
        input: &serde_json::Value,
        outcome: Option<&ToolOutcome>,
    ) -> Part {
        let encoded = serde_json::to_string(input).unwrap_or_default();
        let result = outcome.and_then(|outcome| outcome.text.clone());

        // Running is a real answer, not a placeholder. The last call in a live
        // transcript genuinely has no result yet.
        let status = match outcome {
            None => ToolStatus::Running,
            Some(outcome) if outcome.is_error => ToolStatus::Failed,
            Some(_) => ToolStatus::Ok,
        };

        let mut fallback = format!("{name}\n{encoded}");

        if let Some(text) = &result {
            fallback.push('\n');
            fallback.push_str(text);
        }

        Part::ToolUse {
            name: name.to_string(),
            input: encoded,
            result,
            status,
            fallback_text: fallback,
        }
    }

    fn question_parts(
        id: &str,
        input: &serde_json::Value,
        outcome: Option<&ToolOutcome>,
    ) -> Vec<Part> {
        let asked = match input.get("questions").and_then(serde_json::Value::as_array) {
            Some(asked) => asked,
            None => return Vec::new(),
        };

        let recorded = outcome.and_then(|outcome| outcome.field("answers"));

        // One call, one part. The harness asks up to four questions at once and
        // stays blocked until it has every answer, so the set is the unit a
        // person is put in front of and the unit that gets answered.
        let asks: Vec<Ask> = asked
            .iter()
            .filter_map(|raw| {
                let prompt = raw.get("question").and_then(serde_json::Value::as_str)?;

                Some(Ask {
                    header: raw
                        .get("header")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    prompt: prompt.to_string(),
                    options: Self::options_of(raw),
                    multi_select: raw
                        .get("multiSelect")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                    // The harness always offers a free-text "Other", and nothing
                    // in the call says so, so it is stated here rather than read.
                    allows_free_text: true,
                })
            })
            .collect();

        if asks.is_empty() {
            return Vec::new();
        }

        // Aligned to the asks by position, because that is the order a person
        // answers them in and the order the answers go back. The harness keys
        // its record by the question's own text, so a question it never
        // answered leaves a hole rather than shifting every later answer onto
        // the wrong question.
        let answers: Vec<Option<Answer>> = asks
            .iter()
            .map(|ask| {
                recorded
                    .and_then(|recorded| recorded.get(&ask.prompt))
                    .and_then(|chosen| Self::answer_of(chosen, &ask.options))
            })
            .collect();

        let question = Question {
            id: QuestionId::mint(id),
            fingerprint: Question::fingerprint_of(&asks),
            asks,
        };

        let answered = answers.iter().any(Option::is_some).then(|| AnswerRecord {
            answers,
            at: outcome.and_then(|outcome| outcome.at).unwrap_or(Timestamp(0)),
        });

        let fallback_text = Self::question_fallback(&question);

        vec![Part::Question {
            question,
            answered,
            fallback_text,
        }]
    }

    fn options_of(raw: &serde_json::Value) -> Vec<QuestionOption> {
        raw.get("options")
            .and_then(serde_json::Value::as_array)
            .map(|options| {
                options
                    .iter()
                    .filter_map(|option| {
                        Some(QuestionOption {
                            label: option
                                .get("label")
                                .and_then(serde_json::Value::as_str)?
                                .to_string(),
                            description: option
                                .get("description")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_string),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The recorded answer as an index into the options, or as free text.
    ///
    /// The harness records what was chosen by its label, so a label that is no
    /// longer among the options is an "Other" answer rather than a lost one.
    fn answer_of(chosen: &serde_json::Value, options: &[QuestionOption]) -> Option<Answer> {
        let index_of = |label: &str| {
            options
                .iter()
                .position(|option| option.label == label)
                .map(|position| position as u16)
        };

        match chosen {
            serde_json::Value::String(label) => Some(match index_of(label) {
                Some(index) => Answer::Choice(index),
                None => Answer::Text(label.clone()),
            }),
            serde_json::Value::Array(labels) => {
                let text: Vec<&str> = labels
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect();

                if text.is_empty() {
                    return None;
                }

                let indices: Vec<u16> = text.iter().filter_map(|label| index_of(label)).collect();

                Some(if indices.len() == text.len() {
                    Answer::Multi(indices)
                } else {
                    Answer::Text(text.join(", "))
                })
            }
            _ => None,
        }
    }

    fn question_fallback(question: &Question) -> String {
        let mut text = String::new();

        for ask in &question.asks {
            if !text.is_empty() {
                text.push_str("\n\n");
            }

            text.push_str(&ask.prompt);

            for option in &ask.options {
                text.push_str("\n- ");
                text.push_str(&option.label);

                if let Some(description) = &option.description {
                    text.push_str(" - ");
                    text.push_str(description);
                }
            }
        }

        text
    }

    /// Field names a tool call uses for the file it is working on.
    ///
    /// Measured across the harness's own tools rather than guessed: every one of
    /// them names its subject differently, and none of them names it twice.
    const PATH_FIELDS: &'static [&'static str] =
        &["file_path", "path", "notebook_path", "filePath"];

    /// Records where every file a call names can be found, without making a card
    /// for it.
    ///
    /// The index is what turns an id back into a path, and there is no cost to
    /// knowing about a file nobody asks for. What this does **not** do is put it
    /// in the transcript: a card is a deliberate hand-over, and widening that
    /// would bury the conversation.
    fn note_files(&self, input: &serde_json::Value, outcome: Option<&ToolOutcome>) {
        let Some(index) = &self.assets else {
            return;
        };

        let named = Self::PATH_FIELDS
            .iter()
            .filter_map(|field| input.get(field))
            .chain(
                outcome
                    .and_then(|outcome| outcome.field("filePath"))
                    .into_iter(),
            )
            .filter_map(serde_json::Value::as_str);

        for path in named {
            let canonical = AssetNaming::canonical_of(Path::new(path));

            index.register(AssetNaming::id_for(&canonical), &canonical);
        }
    }

    fn file_parts(&self, input: &serde_json::Value, outcome: Option<&ToolOutcome>) -> Vec<Part> {
        // The result carries the size and the media type the call does not, so
        // it is preferred; a call still in flight falls back to its own paths.
        if let Some(attachments) = outcome
            .and_then(|outcome| outcome.field("attachments"))
            .and_then(serde_json::Value::as_array)
        {
            let parts: Vec<Part> = attachments
                .iter()
                .filter_map(|attachment| {
                    let path = attachment.get("path").and_then(serde_json::Value::as_str)?;

                    Some(self.file_part(
                        path,
                        attachment
                            .get("media_type")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string),
                        attachment.get("size").and_then(serde_json::Value::as_u64),
                    ))
                })
                .collect();

            if !parts.is_empty() {
                return parts;
            }
        }

        input
            .get("files")
            .and_then(serde_json::Value::as_array)
            .map(|files| {
                files
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(|path| self.file_part(path, None, None))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn file_part(&self, path: &str, mime: Option<String>, size: Option<u64>) -> Part {
        let as_path = Path::new(path);

        // A file this machine stored sits behind a digest prefix so two people
        // sending `screenshot.png` do not overwrite each other. The prefix is
        // storage rather than identity, and a card showing it names the file
        // something nobody chose — so it comes off, but only for files that are
        // actually in the store, or a hyphenated name of somebody else's would
        // lose its first word.
        let stored_here = self
            .uploads
            .as_deref()
            .is_some_and(|uploads| SentFiles::is_stored_in(as_path, uploads));

        let name = if stored_here {
            SentFiles::readable_name(as_path)
        } else {
            as_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string())
        };

        let fallback = match size {
            Some(size) => format!("{name} ({size} bytes)"),
            None => name.clone(),
        };

        // Canonicalised once, and both the id and the way back are taken from
        // that one spelling. Deriving them from different spellings is how a
        // card ends up pointing at nothing.
        let canonical = AssetNaming::canonical_of(as_path);
        let asset = AssetNaming::id_for(&canonical);

        if let Some(index) = &self.assets {
            index.register(asset.clone(), &canonical);
        }

        Part::File {
            asset,
            name,
            mime,
            size,
            fallback_text: fallback,
        }
    }

    fn diff_parts(input: &serde_json::Value, outcome: Option<&ToolOutcome>) -> Vec<Part> {
        let hunks = match outcome
            .and_then(|outcome| outcome.field("structuredPatch"))
            .and_then(serde_json::Value::as_array)
        {
            Some(hunks) if !hunks.is_empty() => hunks,
            // A file created rather than changed has no hunks. There is nothing
            // to draw as a diff, so the call renders as the call it was.
            _ => return Vec::new(),
        };

        let path = outcome
            .and_then(|outcome| outcome.field("filePath"))
            .and_then(serde_json::Value::as_str)
            .or_else(|| input.get("file_path").and_then(serde_json::Value::as_str))
            .unwrap_or_default()
            .to_string();

        let mut unified = format!("--- {path}\n+++ {path}\n");
        let mut added = 0u32;
        let mut removed = 0u32;

        for hunk in hunks {
            let old_start = hunk.get("oldStart").and_then(serde_json::Value::as_u64).unwrap_or(0);
            let old_lines = hunk.get("oldLines").and_then(serde_json::Value::as_u64).unwrap_or(0);
            let new_start = hunk.get("newStart").and_then(serde_json::Value::as_u64).unwrap_or(0);
            let new_lines = hunk.get("newLines").and_then(serde_json::Value::as_u64).unwrap_or(0);

            unified.push_str(&format!(
                "@@ -{old_start},{old_lines} +{new_start},{new_lines} @@\n"
            ));

            for line in hunk
                .get("lines")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
            {
                match line.as_bytes().first() {
                    Some(b'+') => added += 1,
                    Some(b'-') => removed += 1,
                    _ => {}
                }

                unified.push_str(line);
                unified.push('\n');
            }
        }

        vec![Part::Diff {
            path,
            unified: unified.clone(),
            added: Some(added),
            removed: Some(removed),
            fallback_text: unified,
        }]
    }

    fn todo_part(input: &serde_json::Value) -> Option<Part> {
        let listed = input.get("todos").and_then(serde_json::Value::as_array)?;

        let items: Vec<TodoItem> = listed
            .iter()
            .filter_map(|todo| {
                Some(TodoItem {
                    text: todo
                        .get("content")
                        .and_then(serde_json::Value::as_str)?
                        .to_string(),
                    status: match todo.get("status").and_then(serde_json::Value::as_str) {
                        Some("in_progress") => TodoStatus::InProgress,
                        Some("completed") | Some("done") => TodoStatus::Done,
                        _ => TodoStatus::Pending,
                    },
                })
            })
            .collect();

        if items.is_empty() {
            return None;
        }

        let fallback = items
            .iter()
            .map(|item| {
                let mark = match item.status {
                    TodoStatus::Pending => " ",
                    TodoStatus::InProgress => "~",
                    TodoStatus::Done => "x",
                };

                format!("[{mark}] {}", item.text)
            })
            .collect::<Vec<String>>()
            .join("\n");

        Some(Part::Todo {
            items,
            fallback_text: fallback,
        })
    }
}
