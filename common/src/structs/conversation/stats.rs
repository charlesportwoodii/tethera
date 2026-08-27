use crate::structs::primitives::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What an agent is doing right now, in figures.
///
/// The point of this is that a working agent should look like work happening.
/// A spinner says only that something has not finished; these say what is going
/// on, and the context figure says whether the session is about to hit a wall.
///
/// Every field here is read off the agent's own records. **Nothing is
/// estimated** — a figure a person acts on has to be right, and a number that is
/// quietly wrong is worse than one that is absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct AgentStats {
    /// When this turn began.
    ///
    /// The start, not an elapsed count. A client ticks its own clock from here,
    /// so the machine sends figures when the figures change rather than once a
    /// second to move a number the client could have moved itself.
    pub turn_started_at: Timestamp,
    #[ts(type = "number")]
    pub tokens_in: u64,
    #[ts(type = "number")]
    pub tokens_out: u64,
    /// Tool calls made since the person last spoke.
    pub tools: u32,
    /// Tokens of context the newest request carried — everything the model was
    /// sent, cached or not.
    #[ts(type = "number")]
    pub context_used: u64,
    /// What the model's context holds, when this machine knows the model.
    ///
    /// `None` rather than a guess. The records name the model and nowhere states
    /// its window, so this is a table of the ones that have been verified — and
    /// a model nobody has checked reports no window rather than a plausible
    /// wrong one. A client draws the figure without a bar.
    #[ts(type = "number | null")]
    pub context_window: Option<u64>,
    /// Display only. The client never branches on it.
    pub model: Option<String>,
    /// Millionths of a dollar. Always absent today.
    ///
    /// The records carry no price and this workspace holds no pricing table, so
    /// there is no honest way to compute it. Present as a field so one can
    /// arrive later with no wire change; a client omits the figure entirely
    /// rather than drawing a zero.
    ///
    /// An integer count of a small unit rather than a fraction of a dollar. A
    /// turn can cost less than a cent, and money that accumulates through a
    /// binary float accumulates its rounding too.
    #[ts(type = "number | null")]
    pub cost_micros: Option<u64>,
    /// The tool call in flight — "Read src/lib/deeplink.ts".
    ///
    /// The one line that makes a row read as something happening rather than
    /// something stuck. Not a transcript entry, because the call has not
    /// returned.
    pub activity: Option<String>,
}
