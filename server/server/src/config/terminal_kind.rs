/// Which terminal backend a machine drives.
///
/// The two are exclusive today, and that is a temporary shape rather than the
/// architecture. `Herdr` has the real tree — workspaces, tabs, splits — and no
/// byte stream, so nothing in it can be attached to. `Pty` can be attached to
/// and has a flat synthetic tree and no `split`. Neither is a superset of the
/// other, so an operator picking one gives something up either way.
///
/// The hybrid is what this should become: herdr owning the tree while tethera's
/// own ptys own the panes it opens. `PaneRegistry` already holds only panes a
/// backend adopted, and the scrollback path already forks on whether it holds
/// one, so nothing here has to be undone to get there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum TerminalKind {
    /// herdr, over its socket API. The default, so an existing machine is
    /// unchanged.
    #[default]
    Herdr,
    /// Ptys this process owns. The only one that can attach.
    Pty,
}

