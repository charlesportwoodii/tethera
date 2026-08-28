use crate::terminal::{PendingQuestion, PromptDetector};
use crate::transcript::{SessionCatalog, StatusRule, TranscriptReader};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tethera_common::protocol::error::WireError;
use tethera_common::protocol::watch::WatchEvent;
use tethera_common::structs::agent::Agent;
use tethera_common::structs::conversation::AgentStats;
use tethera_common::structs::ids::ConversationId;
use tethera_common::structs::transcript::{Part, Question, ToolStatus, Turn};

use crate::protocol::live::LiveTerminals;

/// Whether a conversation is waiting on a person, from both places it can be.
///
/// **One watcher, because there is one answer.** An agent-initiated question is
/// real structure in the records; a permission prompt is drawn on screen and
/// never written down. Emitting from each source separately would send two
/// `Blocked` events for the one question a harness draws both ways, under two
/// different ids, and one `Unblocked` to clear them.
///
/// The records are preferred where they have it. A screen carries the prompt and
/// the rows and nothing else, while the records carry the headers, the
/// descriptions, whether several answers are allowed and whether free text is —
/// all of which a person is shown and none of which can be read off a picker.
pub struct BlockWatch {
    terminals: Arc<LiveTerminals>,
    catalog: Arc<SessionCatalog>,
    readers: Arc<Mutex<HashMap<ConversationId, TranscriptReader>>>,
    agent: Agent,
}

impl BlockWatch {
    /// How often a watched conversation is asked whether it is waiting.
    ///
    /// Slower than the transcript's own tail poll, because the screen half costs
    /// a subprocess per look while the records half is a read of a file already
    /// indexed. A prompt noticed two seconds late is a prompt noticed; the same
    /// call four times a second, per watched conversation, is a machine busy
    /// asking itself questions.
    pub const POLL: Duration = Duration::from_secs(2);

    /// How much of the tail decides whether a question is pending.
    const TAIL: u16 = 16;

    /// How much of a tool call's subject a row can carry.
    const ACTIVITY_CHARS: usize = 60;

    pub fn new(
        terminals: Arc<LiveTerminals>,
        catalog: Arc<SessionCatalog>,
        readers: Arc<Mutex<HashMap<ConversationId, TranscriptReader>>>,
        agent: Agent,
    ) -> Self {
        Self {
            terminals,
            catalog,
            readers,
            agent,
        }
    }

    /// What this conversation is waiting on, or nothing.
    ///
    /// The same answer `answer` checks a client's fingerprint against, which is
    /// what makes the two agree: a set detected one way and answered against the
    /// other would refuse every answer as stale.
    /// **`Ok(None)` is "nothing is pending". `Err` is "this machine could not
    /// tell", and the two must never be confused.**
    ///
    /// Both sources can fail for reasons that have nothing to do with the
    /// question: reading the screen is a subprocess behind an admission gate,
    /// and a gate that is busy answers `Busy`. Folding that into "no question"
    /// dismisses a prompt that is still on the screen, and then refuses the
    /// answer a person sends against it as being for a question nobody asked.
    pub async fn pending(&self, id: &ConversationId) -> Result<Option<PendingQuestion>, WireError> {
        let recorded = self.recorded(id).await;

        if let Ok(Some(found)) = recorded {
            return Ok(Some(found));
        }

        Self::settle(recorded, self.on_screen(id).await)
    }

    /// What the two sources say together, once the records have not answered
    /// outright.
    ///
    /// Its own function because this distinction is the whole of what this type
    /// decides, and it is worth being testable with no machine behind it. The
    /// screen is preferred only where the records had nothing, since a
    /// permission prompt is never written down; and an absence is only reported
    /// as an absence when both sources were actually consulted.
    pub fn settle(
        recorded: Result<Option<PendingQuestion>, WireError>,
        on_screen: Result<Option<PendingQuestion>, WireError>,
    ) -> Result<Option<PendingQuestion>, WireError> {
        match (recorded, on_screen) {
            // A question was found. That the other source failed no longer
            // matters, because there is nothing left to be uncertain about.
            (_, Ok(Some(found))) => Ok(Some(found)),
            // Both consulted. Only here is nothing a fact.
            (Ok(found), Ok(None)) => Ok(found),
            // One of them could not be read, and nothing was found. Reporting
            // that as no question dismisses a prompt that may well still be on
            // the screen, and refuses the answer sent against it.
            (Err(error), _) | (_, Err(error)) => Err(error),
        }
    }

    /// What the agent is doing right now, in figures.
    ///
    /// The tool call in flight comes from the turns and the numbers come from the
    /// records behind them, so both are taken in one read of the same tail.
    pub async fn stats(&self, id: &ConversationId) -> Option<AgentStats> {
        let session = id.as_str().strip_prefix(ConversationId::PREFIX)?;
        let path = self.catalog.locate(session)?;

        let readers = self.readers.clone();
        let agent = self.agent;
        let id = id.clone();

        tokio::task::spawn_blocking(move || {
            let mut open = readers.lock().expect("lock");
            let reader = open
                .entry(id)
                .or_insert_with(|| TranscriptReader::open(path, agent));

            let running = reader
                .page(None, Self::TAIL)
                .ok()
                .and_then(|page| Self::in_flight(&page.items));

            reader.stats(running).ok().flatten()
        })
        .await
        .ok()
        .flatten()
    }

    /// The tool call that has not returned yet, as a person would read it.
    ///
    /// The one line that makes a working row look like something happening. A
    /// call that has returned is history and belongs in the transcript, not here.
    fn in_flight(recent: &[Turn]) -> Option<String> {
        recent
            .iter()
            .rev()
            .flat_map(|turn| turn.parts.iter().rev())
            .find_map(|part| match part {
                Part::ToolUse {
                    name,
                    input,
                    status: ToolStatus::Running,
                    ..
                } => Some(Self::describe(name, input)),
                _ => None,
            })
    }

    /// "Read src/lib/deeplink.ts" — the tool and what it is working on.
    ///
    /// The subject is whatever single-line string the call's own arguments lead
    /// with, because every tool names its subject differently and none of them
    /// names it the same way twice. A call whose arguments are all structure gets
    /// its name alone, which is still better than a spinner.
    fn describe(name: &str, input: &str) -> String {
        let subject = serde_json::from_str::<serde_json::Value>(input)
            .ok()
            .and_then(|value| {
                value.as_object().and_then(|fields| {
                    fields
                        .values()
                        .find_map(serde_json::Value::as_str)
                        .map(str::to_string)
                })
            })
            .map(|text| Self::one_line(&text))
            .filter(|text| !text.is_empty());

        match subject {
            Some(subject) => format!("{name} {subject}"),
            None => name.to_string(),
        }
    }

    /// The first line, shortened. A row is one line and a tool's argument can be
    /// a whole file.
    fn one_line(text: &str) -> String {
        let first = text.lines().next().unwrap_or_default().trim();

        if first.chars().count() <= Self::ACTIVITY_CHARS {
            return first.to_string();
        }

        let cut: String = first.chars().take(Self::ACTIVITY_CHARS).collect();

        format!("{cut}…")
    }

    /// The pending set as the records have it.
    async fn recorded(&self, id: &ConversationId) -> Result<Option<PendingQuestion>, WireError> {
        let Some(session) = id.as_str().strip_prefix(ConversationId::PREFIX) else {
            return Ok(None);
        };

        // No records file is not a failure: a harness that writes none, or a
        // conversation that has not written one yet, genuinely has no recorded
        // question and the screen is the only place left to look.
        let Some(path) = self.catalog.locate(session) else {
            return Ok(None);
        };

        let readers = self.readers.clone();
        let agent = self.agent;
        let id = id.clone();

        let tail = tokio::task::spawn_blocking(move || {
            let mut open = readers.lock().expect("lock");
            let reader = open
                .entry(id)
                .or_insert_with(|| TranscriptReader::open(path, agent));

            reader.page(None, Self::TAIL)
        })
        .await
        .map_err(|error| WireError::Backend {
            message: format!("reading the records did not finish: {error}"),
        })?
        ?;

        // The records cannot know what is ticked on a screen they never saw, and
        // `None` says so rather than reporting every row clear.
        Ok(StatusRule::pending_question(&tail.items)
            .map(|question| PendingQuestion::new(question, None)))
    }

    /// The pending prompt as the agent has it on screen.
    ///
    /// Only reached when the records have nothing, which is the common case: a
    /// permission prompt is the question a person is asked most often and the
    /// one the records never carry.
    async fn on_screen(&self, id: &ConversationId) -> Result<Option<PendingQuestion>, WireError> {
        // Nothing is running, so nothing is drawing a prompt. An absence this
        // machine is sure of.
        let Some(pane) = self
            .terminals
            .bindings()
            .into_iter()
            .find(|(_, conversation)| conversation == id)
            .map(|(pane, _)| pane)
        else {
            return Ok(None);
        };

        // Not `ok()`. A screen this machine could not read is the case this
        // whole signature exists for: the subprocess sits behind an admission
        // gate shared with every other terminal call, and losing that race is
        // ordinary rather than exceptional.
        // A harness nobody has measured has no chrome to read, so nothing is
        // reported as pending rather than a guess being published to a phone.
        let Some(detector) = PromptDetector::for_agent(self.agent) else {
            return Ok(None);
        };

        let screen = self.terminals.screen(&pane).await?;

        Ok(detector.detect(&screen))
    }

    /// Publishes the transitions of one conversation until nobody is listening.
    ///
    /// Transitions, not states: a client is told when a question appears and
    /// when it clears, and a repeat of either would draw the same prompt twice or
    /// dismiss one that is still up.
    pub fn publish(
        self,
        id: ConversationId,
        events: tokio::sync::broadcast::Sender<WatchEvent>,
    ) {
        tokio::spawn(async move {
            let mut asked: Option<Question> = None;
            let mut reported: Option<AgentStats> = None;

            loop {
                tokio::time::sleep(Self::POLL).await;

                if events.receiver_count() == 0 {
                    tracing::debug!(
                        conversation = id.as_str(),
                        "nobody is watching this conversation; stopping its block watch"
                    );

                    return;
                }

                // Sent when they change, never on a clock. `turn_started_at` is
                // a start rather than an elapsed count, so a client ticks its own
                // second hand and nothing has to be published to move a number
                // the client could move itself.
                let figures = self.stats(&id).await;

                if figures.is_some() && figures != reported {
                    if let Some(figures) = figures.clone() {
                        let _ = events.send(WatchEvent::Stats(figures));
                    }
                }

                reported = figures;

                let found = match self.pending(&id).await {
                    Ok(found) => found,
                    // **Publishing nothing is the only safe answer here.** The
                    // arm below would read this as the question having cleared
                    // and send `Unblocked`, which takes the prompt off somebody's
                    // screen while they are part-way through answering it — and
                    // the answer they then send has nothing left to attach to.
                    Err(error) => {
                        tracing::debug!(
                            ?error,
                            conversation = id.as_str(),
                            "could not tell whether a question is pending; leaving it as it was"
                        );

                        continue;
                    }
                };

                // What a watcher publishes is the question itself. The tick
                // state beside it is for driving the picker, and never leaves
                // this machine.
                let found = found.map(|pending| pending.question);

                match (&asked, &found) {
                    // Cleared, by any route — including somebody answering at the
                    // machine, which is the case a phone can never observe for
                    // itself and would otherwise show as blocked forever.
                    (Some(was), None) => {
                        tracing::info!(
                            conversation = id.as_str(),
                            question = was.id.as_str(),
                            "a question cleared"
                        );

                        let _ = events.send(WatchEvent::Unblocked {
                            question: was.id.clone(),
                        });
                    }
                    // A different question replaced the last one without a gap
                    // between them. Both events are owed: one prompt went and
                    // another arrived.
                    (Some(was), Some(now)) if was.id != now.id => {
                        tracing::info!(
                            conversation = id.as_str(),
                            went = was.id.as_str(),
                            came = now.id.as_str(),
                            fingerprint = now.fingerprint.as_str(),
                            "one question replaced another"
                        );

                        let _ = events.send(WatchEvent::Unblocked {
                            question: was.id.clone(),
                        });
                        let _ = events.send(WatchEvent::Blocked {
                            question: now.clone(),
                        });
                    }
                    (None, Some(now)) => {
                        // The event a client turns into an answering control, so
                        // "was one ever sent" is the first question asked when
                        // somebody cannot answer from their phone. It had no
                        // answer until this line existed.
                        tracing::info!(
                            conversation = id.as_str(),
                            question = now.id.as_str(),
                            fingerprint = now.fingerprint.as_str(),
                            asks = now.asks.len(),
                            listeners = events.receiver_count(),
                            "a question is waiting on somebody"
                        );

                        let _ = events.send(WatchEvent::Blocked {
                            question: now.clone(),
                        });
                    }
                    _ => {}
                }

                asked = found;
            }
        });
    }
}
