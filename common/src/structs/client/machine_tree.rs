use crate::structs::conversation::Conversation;
use crate::structs::terminal::{Pane, Tab, TabLayout, Workspace};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The opening snapshot of a machine watch, as the webview receives it.
///
/// The same five lists `WatchOpen::Machine` carries, and a separate type rather
/// than the wire enum, because a client-local type may change shape without a
/// wire version.
///
/// Every rank arrives together: the screen draws workspaces, tabs, panes and
/// conversations before anything is tapped, so sending less would mean a request
/// per rank before the first screen appears.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct MachineTree {
    pub workspaces: Vec<Workspace>,
    pub tabs: Vec<Tab>,
    pub panes: Vec<Pane>,
    pub conversations: Vec<Conversation>,
    /// One per tab the machine could place. A tab absent from this list has no
    /// geometry the machine will vouch for, and the client draws no map for it
    /// rather than an invented one.
    pub layouts: Vec<TabLayout>,
}
