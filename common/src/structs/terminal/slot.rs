use super::PaneRect;
use crate::structs::ids::PaneId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One pane's place in its tab's layout.
///
/// Deliberately not a field on `Pane`. Geometry does not arrive with the pane
/// list — a backend answers it separately and at a different cost — and it
/// changes on a different clock: splitting one pane moves every neighbour
/// without altering a single pane's identity. Keeping the two apart means a
/// client that only needs names never pays for rects, and a layout that moved
/// does not invalidate the pane list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct PaneSlot {
    pub pane: PaneId,
    pub rect: PaneRect,
}

impl PaneSlot {
    pub fn new(pane: PaneId, rect: PaneRect) -> Self {
        Self { pane, rect }
    }
}
