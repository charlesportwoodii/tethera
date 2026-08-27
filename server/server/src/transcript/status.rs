use tethera_common::structs::agent::AgentStatus;
use tethera_common::structs::transcript::{Part, Question, ToolStatus, Turn};

/// What an agent is doing, decided in one place.
///
/// One named predicate rather than a field, because two callers need the answer
/// and a stored one would go stale between them.
pub struct StatusRule;

impl StatusRule {
    /// `recent` is the tail of the conversation, oldest first.
    ///
    /// A conversation with no pane is `Done` whatever its records say: nothing
    /// is running, so nothing is working or waiting. That also keeps the cost
    /// honest - only a bound conversation is worth reading a tail for, and there
    /// is at most one per pane.
    pub fn decide(bound: bool, recent: &[Turn], grew_recently: bool) -> AgentStatus {
        if !bound {
            return AgentStatus::Done;
        }

        if Self::waiting_on_a_person(recent) {
            return AgentStatus::Blocked;
        }

        // A call in flight is the whole of "this agent is mid-turn". Whether it
        // is moving is what separates working from stuck, and the difference
        // matters more than either: an agent reported idle looks finished, and
        // nobody goes to look at a machine that says it is done.
        if Self::has_a_call_in_flight(recent) {
            return if grew_recently {
                AgentStatus::Working
            } else {
                AgentStatus::Stalled
            };
        }

        AgentStatus::Idle
    }

    /// The newest question, and whether it was ever answered.
    ///
    /// Newest rather than any: an earlier unanswered question that the agent
    /// moved on from is history, not a thing a person is being asked now.
    fn waiting_on_a_person(recent: &[Turn]) -> bool {
        Self::pending_question(recent).is_some()
    }

    /// The set a person is being asked now, if there is one.
    ///
    /// **Only the newest turn.** A question a person is actually waiting on is
    /// the last thing in the conversation — nothing follows it until it is
    /// answered, and a tool result landing revises that same turn rather than
    /// adding another.
    ///
    /// So anything *after* a question means the conversation moved past it, and
    /// that is the case this rule exists for. A harness offers ways out of its
    /// own picker — "chat about this" and a typed reply — and taking one leaves
    /// the tool call with no answer recorded against it, for ever. Looking
    /// further back than the newest turn reports that abandoned question as
    /// pending nine minutes later, over a card nobody can clear, on a
    /// conversation whose agent has long since replied and gone quiet.
    pub fn pending_question(recent: &[Turn]) -> Option<Question> {
        recent
            .last()?
            .parts
            .iter()
            .rev()
            .find_map(|part| match part {
                Part::Question {
                    question,
                    answered: None,
                    ..
                } => Some(question.clone()),
                _ => None,
            })
    }

    fn has_a_call_in_flight(recent: &[Turn]) -> bool {
        recent
            .last()
            .is_some_and(|turn| {
                turn.parts.iter().any(|part| {
                    matches!(part, Part::ToolUse { status: ToolStatus::Running, .. })
                })
            })
    }

    /// One line of the most recent meaningful text.
    ///
    /// The pending question's prompt when blocked, otherwise the agent's last
    /// words. Deciding what is meaningful is the same judgement as the noise
    /// filter, which is why it lives beside it rather than in the client.
    pub fn preview(recent: &[Turn]) -> Option<String> {
        let pending = recent
            .iter()
            .rev()
            .flat_map(|turn| turn.parts.iter().rev())
            .find_map(|part| match part {
                Part::Question {
                    question, answered, ..
                } if answered.is_none() => Some(question.prompt().to_string()),
                _ => None,
            });

        if pending.is_some() {
            return pending;
        }

        recent
            .iter()
            .rev()
            .flat_map(|turn| turn.parts.iter().rev())
            .find_map(|part| match part {
                Part::Text { text } => Some(Self::one_line(text)),
                _ => None,
            })
    }

    fn one_line(text: &str) -> String {
        text.lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default()
            .trim()
            .to_string()
    }
}
