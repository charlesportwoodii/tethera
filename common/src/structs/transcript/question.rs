use crate::structs::ids::QuestionId;
use crate::structs::primitives::{Fingerprint, Timestamp};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct QuestionOption {
    pub label: String,
    pub description: Option<String>,
}

/// One thing being asked, inside a set.
///
/// Carries no id and no fingerprint of its own. Both belong to the set, because
/// the set is what gets answered: a person works through the questions, reviews
/// them, and one reply goes back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Ask {
    pub header: Option<String>,
    pub prompt: String,
    pub options: Vec<QuestionOption>,
    pub multi_select: bool,
    /// Whether an "Other" free-text answer is accepted.
    pub allows_free_text: bool,
}

/// The set of questions an agent is blocked on, answered as one act.
///
/// Reaches the client two ways and looks identical both times: as a
/// `Part::Question` in the transcript, and as a `Blocked` watch event. An
/// agent-initiated question is read from the transcript, where it is real
/// structure; a permission prompt is detected from what the agent has on screen.
/// Which detector found it is a server-side detail, and the client cannot tell
/// the two apart.
///
/// **A set rather than one question, because answering is atomic.** A harness
/// asks up to four at once and stays blocked until it has every answer, and its
/// picker is a single piece of screen state. Answering one at a time would move
/// that picker halfway and then ask the client to fingerprint a screen this
/// server had itself just changed — so the answers are collected, then delivered
/// together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Question {
    pub id: QuestionId,
    /// Echoed back when answering. The server refuses a stale answer rather than
    /// answering a different question blind.
    pub fingerprint: Fingerprint,
    pub asks: Vec<Ask>,
}

impl Question {
    /// The fingerprint of a set's current state.
    ///
    /// Defined here rather than server-side so both ends compute it identically
    /// from identical inputs, and so a set read out of a transcript and a set
    /// read off a screen fingerprint the same way. Not a cryptographic
    /// commitment - it only has to change when the questions do, which is what
    /// makes a stale answer detectable.
    pub fn fingerprint_of(asks: &[Ask]) -> Fingerprint {
        // FNV-1a, 64-bit. No dependency, and `common` must stay linkable by
        // consumers that pull in no hashing stack.
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;

        let mut hash = OFFSET;
        let mut eat = |bytes: &[u8]| {
            for byte in bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(PRIME);
            }
        };

        // A zero byte separates every field, so moving text across a boundary
        // changes the digest. Without it a label of "Allow" with description
        // "always" and a label of "Allowalways" would fingerprint the same.
        for ask in asks {
            eat(&[0]);
            eat(ask.header.as_deref().unwrap_or("").as_bytes());
            eat(&[0]);
            eat(ask.prompt.as_bytes());

            for option in &ask.options {
                eat(&[0]);
                eat(option.label.as_bytes());
                eat(&[0]);
                eat(option.description.as_deref().unwrap_or("").as_bytes());
            }
        }

        Fingerprint(format!("{hash:016x}"))
    }

    /// The line that stands for the whole set on a list row.
    ///
    /// The first question, because that is the one a person is asked first and
    /// the set is worked through in order.
    pub fn prompt(&self) -> &str {
        self.asks.first().map(|ask| ask.prompt.as_str()).unwrap_or("")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum Answer {
    /// The index of one option.
    Choice(u16),
    /// The indices of several, when `multi_select` is set.
    Multi(Vec<u16>),
    /// A free-text answer, when `allows_free_text` is set.
    Text(String),
}

/// What was answered, one entry per ask, in the set's own order.
///
/// `None` where the records carry no answer for that question. A historical set
/// can genuinely be part-answered — the harness writes what it has — and a hole
/// is the honest way to say so rather than an index that quietly shifts every
/// later answer onto the wrong question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct AnswerRecord {
    pub answers: Vec<Option<Answer>>,
    pub at: Timestamp,
}
