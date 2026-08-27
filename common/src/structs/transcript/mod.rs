mod part;
mod question;

pub use part::{Part, TodoItem, TodoStatus, ToolStatus};
pub use question::{Answer, AnswerRecord, Ask, Question, QuestionOption};

use crate::structs::ids::TurnId;
use crate::structs::primitives::{Cursor, Timestamp};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum Role {
    Operator,
    Agent,
}

/// One turn: something a person said, or one response from an agent.
///
/// Only deliberate acts appear. An agent that records its own injected content
/// as user messages - skill bodies, task notifications, system reminders - has
/// that content dropped before a turn is built, because rendering machine
/// chatter attributes it to the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Turn {
    /// Resume from here. Opaque.
    pub cursor: Cursor,
    /// Stable across reads, derived from the source record, used to dedupe.
    pub id: TurnId,
    pub at: Timestamp,
    pub role: Role,
    pub parts: Vec<Part>,
}

impl Turn {
    pub fn new(cursor: Cursor, id: TurnId, at: Timestamp, role: Role, parts: Vec<Part>) -> Self {
        Self {
            cursor,
            id,
            at,
            role,
            parts,
        }
    }
}
