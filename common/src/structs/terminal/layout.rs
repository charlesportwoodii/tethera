use super::PaneSlot;
use crate::structs::ids::{PaneId, TabId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How one tab's panes are arranged, as the backend reports it.
///
/// The whole tab or nothing. A layout carrying some of its panes is not a
/// partial picture, it is a wrong one: a client normalises rects against the
/// area they cover, so a missing pane silently stretches its neighbours over
/// the gap and draws a map that looks correct. A backend that cannot place
/// every pane reports no layout at all.
///
/// There is no `area` field. The area is the union of the slots, which the
/// panes tile exactly, so sending it separately would be a second copy of the
/// same fact and a chance for the two to disagree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct TabLayout {
    pub tab: TabId,
    pub slots: Vec<PaneSlot>,
    /// The pane filling the tab on its own, when one is.
    ///
    /// A zoomed tab still reports every slot at its unzoomed rect, because
    /// that is the layout that comes back when the zoom is released. Naming
    /// the pane rather than carrying a flag is what lets a client draw the map
    /// and say which rectangle is currently covering the rest.
    pub zoomed: Option<PaneId>,
}

impl TabLayout {
    pub fn new(tab: TabId, slots: Vec<PaneSlot>) -> Self {
        Self {
            tab,
            slots,
            zoomed: None,
        }
    }
}
