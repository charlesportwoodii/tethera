use tethera_common::structs::terminal::Pane;

/// Whether a conversation may be started again.
///
/// **A resume that lands on a session already running puts two agents on one
/// set of records.** They both append, the history interleaves, and every
/// screen that reads a transcript reads the wreckage. `LiveConversations::resume`
/// has always said so; its only guard was a pane binding, which a backend
/// routinely cannot supply — a third of the live agents on a working machine
/// announce no session at all.
///
/// So this decides from what is actually knowable, and it is deliberately a
/// question about *ruling out* rather than about proving. An unnamed agent
/// cannot be shown to be this conversation. That is exactly why a second one
/// must not be started beside it.
///
/// An associated function over plain values rather than a method that reads the
/// backend: the decision is the whole of what this type is, and it is worth
/// being testable without a machine.
pub struct ResumeGate;

impl ResumeGate {
    /// True when starting this conversation again is known to be safe.
    ///
    /// Ruled out, in order of how much each fact settles:
    ///
    /// - **A different directory settles it.** A harness indexes its sessions
    ///   per directory, so an agent running elsewhere is not this session.
    /// - **A different title settles it too.** A terminal's title and a
    ///   session's title come from the same record the harness writes, so two
    ///   that differ are two sessions. Measured rather than assumed, against
    ///   live panes.
    /// - **Anything else does not settle it**, including a pane with no title,
    ///   and the answer is no.
    ///
    /// A title is only ever read to *permit*. Nothing here maps a pane onto a
    /// conversation, because a binding invented from a directory and a name
    /// would send somebody's typed message to the wrong agent — which, unlike a
    /// refusal, cannot be taken back.
    pub fn admits(cwd: &str, title: Option<&str>, panes: &[Pane]) -> bool {
        !panes.iter().any(|pane| Self::may_be_running(cwd, title, pane))
    }

    /// Whether this pane could be the conversation, for all this machine knows.
    fn may_be_running(cwd: &str, title: Option<&str>, pane: &Pane) -> bool {
        // An identified pane is somebody else's problem: it either binds to this
        // conversation, in which case the caller answered long before here, or
        // it names a different session outright.
        if pane.agent.is_none() || pane.conversation.is_some() {
            return false;
        }

        let Some(here) = pane.cwd.as_deref() else {
            // A pane whose directory the backend did not report cannot be ruled
            // out by the one fact that would settle it.
            return true;
        };

        if !Self::same_directory(cwd, here) {
            return false;
        }

        match (title, pane.title.as_deref()) {
            (Some(wanted), Some(shown)) => wanted == shown,
            // Either side unnamed leaves the directory as the only evidence, and
            // the directory alone is a match.
            _ => true,
        }
    }

    /// Windows path comparison, because that is where this runs and a harness
    /// records the directory as it was typed.
    ///
    /// Separators and case both vary between what a person typed, what the
    /// harness wrote down and what the backend reports for the same directory.
    /// A comparison that missed on any of them would rule a pane out on a
    /// spelling difference, which is the direction that starts a second agent.
    fn same_directory(left: &str, right: &str) -> bool {
        let normalise = |path: &str| {
            path.trim_end_matches(['/', '\\'])
                .replace('\\', "/")
                .to_lowercase()
        };

        normalise(left) == normalise(right)
    }
}
