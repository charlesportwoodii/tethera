use super::Snapshot;
use serde::Deserialize;

/// The `session_snapshot` result.
///
/// herdr's `result` is a tagged union and every variant puts its payload under
/// its own key beside `type`. `session_snapshot` nests the whole session under
/// `snapshot`, so decoding `Snapshot` straight out of `result` fails on the
/// first required field — which is exactly what it did.
#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotBody {
    pub snapshot: Snapshot,
}
