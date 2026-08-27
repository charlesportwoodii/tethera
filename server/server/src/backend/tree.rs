use tethera_common::structs::terminal::{Pane, Tab, TabLayout, Workspace};

/// Every rank of a backend's tree, from one read.
///
/// A struct rather than a tuple because the fourth field is where a tuple stops
/// being readable, and because the ranks are read together on purpose: a
/// backend that answered each separately would answer from a different instant,
/// and two ranks from two instants disagree about panes that moved in between.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackendTree {
    pub workspaces: Vec<Workspace>,
    pub tabs: Vec<Tab>,
    pub panes: Vec<Pane>,
    /// One per tab this backend placed. Empty for a backend with no layout
    /// engine, which is a true statement about it rather than a failed read.
    pub layouts: Vec<TabLayout>,
}
