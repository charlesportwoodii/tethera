use super::Record;

/// Which records belong to the same turn.
///
/// The harness writes each content block of a model response as its own record -
/// measured: 7413 of 7415 assistant records carry exactly one block - so one
/// record per turn would render a single response as a text turn followed by a
/// separate tool turn, over and over, for the whole conversation.
pub struct TurnGrouping;

impl TurnGrouping {
    /// Whether `next` continues the turn `previous` started.
    ///
    /// Adjacency is load-bearing, not decoration. Measured: 107 cases where a
    /// `requestId` appears, is followed by other ids, and then appears again.
    /// Grouping by the id alone would merge two responses that are minutes and
    /// several tool calls apart.
    pub fn joins(previous: &Record, next: &Record) -> bool {
        if !previous.is_assistant() || !next.is_assistant() {
            return false;
        }

        match (&previous.request_id, &next.request_id) {
            // A record with no id is its own turn. Two of them in the measured
            // sample, and merging them on a shared absence would join unrelated
            // responses.
            (Some(before), Some(after)) => !before.is_empty() && before == after,
            _ => false,
        }
    }
}
