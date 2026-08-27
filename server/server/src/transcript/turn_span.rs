use super::RecordSpan;

/// The records of one turn.
///
/// A turn is a group, not a record: the harness writes each content block of a
/// model response separately. The index is granular in turns for a reason the
/// wire depends on - `limit` counts turns, `has_earlier` answers about turns,
/// and a page boundary that landed inside a group would split one response into
/// two turns with different ids that no dedupe could rejoin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnSpan {
    /// The first record's offset, which is the turn's cursor.
    pub offset: u64,
    pub records: Vec<RecordSpan>,
}

impl TurnSpan {
    pub fn new(first: RecordSpan) -> Self {
        Self {
            offset: first.offset,
            records: vec![first],
        }
    }
}
