use std::process::Command;

/// A child process that opens no console window.
///
/// Every child this server spawns has to go through here on Windows. A detached
/// `tethera server start` runs with `DETACHED_PROCESS`, which means it has no
/// console of its own — and when a process with no console starts a console
/// application, Windows allocates a **new** console for it. That console is a
/// real window: it appears on the operator's desktop, takes focus, and vanishes
/// when the child exits.
///
/// The server asks its terminal backend for a snapshot on every tree read and
/// again on every heartbeat, so without this a window flashes several times a
/// minute for the life of the process.
///
/// It is not a cosmetic problem. A window that takes focus while somebody is
/// typing sends the rest of what they type somewhere they did not choose.
///
/// Applied at each spawn rather than by giving the server itself a hidden
/// console, so it holds however the server was started — from a terminal, from
/// the detached path, or from a future GUI that has no console at all.
pub struct Windowless;

impl Windowless {
    /// `CREATE_NO_WINDOW`, spelled out because the workspace depends on no
    /// Windows bindings crate. Ignored by Windows for a GUI child, which is why
    /// it is safe to apply to every spawn rather than only to the ones known to
    /// be console applications.
    #[cfg(windows)]
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Nothing is lost by refusing the console: every child spawned here has its
    /// streams redirected, so it has no use for one.
    #[cfg(windows)]
    pub fn apply(command: &mut Command) -> &mut Command {
        use std::os::windows::process::CommandExt;

        command.creation_flags(Self::CREATE_NO_WINDOW)
    }

    /// No console to allocate, so nothing to suppress. Present so the call site
    /// reads the same on both platforms rather than carrying a `cfg`.
    #[cfg(not(windows))]
    pub fn apply(command: &mut Command) -> &mut Command {
        command
    }
}
