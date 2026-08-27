use super::Record;
use tethera_common::structs::conversation::AgentStats;
use tethera_common::structs::primitives::Timestamp;

/// What an agent is doing right now, from its own records.
///
/// One place, because two callers want the same answer and a stored one would go
/// stale between them — the same reason `StatusRule` is a rule rather than a
/// field.
pub struct StatsRule;

impl StatsRule {
    /// The context window of each model this workspace has verified.
    ///
    /// A table, and it is the one place a figure here is not read off a record:
    /// the records name the model and nowhere state its window. **A model that
    /// is not on this list reports no window at all**, rather than a plausible
    /// wrong one — a bar drawn against a guess would tell somebody they had room
    /// they did not have.
    ///
    /// Matched by prefix, because a model id carries a date suffix that a window
    /// does not change with.
    const WINDOWS: &'static [(&'static str, u64)] = &[
        ("claude-opus-5", 1_000_000),
        ("claude-sonnet-5", 1_000_000),
        ("claude-haiku-4-5", 200_000),
        ("claude-opus-4", 200_000),
        ("claude-sonnet-4", 200_000),
    ];

    /// The figures for the turn these records end in.
    ///
    /// `records` is the tail, oldest first. `None` when nothing in it came from
    /// the model — a conversation whose newest records are all the person's has
    /// no turn in flight to report on.
    pub fn of(records: &[Record], running: Option<String>) -> Option<AgentStats> {
        let began = Self::turn_began(records)?;

        // The newest request that carried usage. Not a sum: a harness writes one
        // record per content block and repeats the same request's usage on each,
        // so adding them up multiplies one request by however many blocks it
        // happened to produce.
        let newest = records
            .iter()
            .rev()
            .find(|record| record.is_assistant() && !record.usage().is_empty())
            .map(|record| (record.usage(), record.model()));

        let (usage, model) = newest.unwrap_or_default();

        Some(AgentStats {
            turn_started_at: began,
            tokens_in: usage.input_tokens,
            tokens_out: usage.output_tokens,
            tools: Self::tool_calls_since_the_person_spoke(records),
            context_used: usage.context_used(),
            context_window: model.as_deref().and_then(Self::window_of),
            model,
            // The records carry no price and this workspace holds no pricing
            // table. Absent is the only honest answer.
            cost_micros: None,
            activity: running,
        })
    }

    /// The window a model id names, when it is one that has been checked.
    pub fn window_of(model: &str) -> Option<u64> {
        Self::WINDOWS
            .iter()
            .find(|(named, _)| model.starts_with(named))
            .map(|(_, window)| *window)
    }

    /// When the person last spoke, which is when this turn began.
    ///
    /// A turn is everything since the last thing the person said, so the elapsed
    /// figure counts from there rather than from the newest model response —
    /// which would reset every time the agent produced another block and read as
    /// an agent that keeps starting over.
    fn turn_began(records: &[Record]) -> Option<Timestamp> {
        records
            .iter()
            .rev()
            .find(|record| !record.is_assistant())
            .and_then(|record| record.at())
            .or_else(|| records.first().and_then(|record| record.at()))
    }

    fn tool_calls_since_the_person_spoke(records: &[Record]) -> u32 {
        let from = records
            .iter()
            .rposition(|record| !record.is_assistant())
            .map(|at| at + 1)
            .unwrap_or(0);

        records[from..]
            .iter()
            .flat_map(|record| record.blocks())
            .filter(|block| matches!(block, super::ContentBlock::ToolUse { .. }))
            .count()
            .try_into()
            .unwrap_or(u32::MAX)
    }
}
