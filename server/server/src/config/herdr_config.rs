use std::path::{Path, PathBuf};

/// herdr's own `config.toml`, read and written in place.
///
/// The only file tethera edits that it does not own. A shim reaches a pane only
/// by being that pane's shell, and herdr decides that from `[terminal]
/// default_shell` — which has no `config set` on the CLI, so this is a
/// read-modify-write on somebody else's file.
///
/// Comment-preserving for that reason. The operator's comments are the
/// documentation for their own configuration, and a rewrite that dropped them
/// would be vandalism dressed as a feature.
pub struct HerdrConfig;

impl HerdrConfig {
    /// Where the backup goes before anything is written.
    pub const BACKUP_SUFFIX: &'static str = ".tethera-backup";

    /// herdr's config, on this platform.
    ///
    /// `None` when the directory a config would live in cannot be located at
    /// all, which is not the same as the file being absent — the caller reports
    /// those differently.
    pub fn path() -> Option<PathBuf> {
        let base = if cfg!(windows) {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        };

        Some(base?.join("herdr").join("config.toml"))
    }

    /// The shell herdr currently starts new panes with, if it names one.
    pub fn hooked(document: &str) -> Option<String> {
        let parsed = document.parse::<toml_edit::DocumentMut>().ok()?;

        parsed
            .get("terminal")?
            .get("default_shell")?
            .as_str()
            .filter(|shell| !shell.is_empty())
            .map(str::to_owned)
    }

    /// The document with `default_shell` set to the shim.
    ///
    /// Idempotent: hooking a document that is already hooked to the same path
    /// returns it unchanged. This runs on request rather than on every start,
    /// but a config that grew a line each time would be unreadable within a week
    /// and the operator would be right to distrust it.
    ///
    /// The original is returned unchanged when it cannot be parsed. A config
    /// tethera cannot read is one it must not rewrite.
    pub fn hook(document: &str, shim: &Path) -> String {
        let Ok(mut parsed) = document.parse::<toml_edit::DocumentMut>() else {
            return document.to_string();
        };

        // A table toml_edit creates is appended, which puts it below whatever
        // trailing comment the document ends on — and a trailing comment is how
        // a herdr plugin closes a region it owns and rewrites. Measured on a
        // real config: `[terminal]` landed inside
        // `# --- added by ez-corp.space-usage ---` ...
        // `# --- end ez-corp.space-usage ---`, where the plugin's own
        // `status-disable` would delete it and return every pane on the machine
        // to a shell nobody chose, with nothing pointing at the cause.
        //
        // So the tail moves above the new table instead. Only when the table is
        // being created: an existing `[terminal]` is left exactly where the
        // operator put it.
        let tail = match parsed.get("terminal") {
            Some(_) => None,
            None => parsed.trailing().as_str().map(str::to_owned),
        };

        let table = parsed
            .entry("terminal")
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));

        let Some(table) = table.as_table_mut() else {
            return document.to_string();
        };

        table["default_shell"] = toml_edit::value(shim.to_string_lossy().as_ref());

        if let Some(tail) = &tail {
            table.decor_mut().set_prefix(tail.clone());
        }

        if tail.is_some() {
            parsed.set_trailing("");
        }

        parsed.to_string()
    }

    /// The document with `default_shell` removed entirely.
    ///
    /// Removed rather than set empty, so herdr falls back to its own default
    /// exactly as it would on a config that never named one. An operator who
    /// tried tethera and stopped must not be left with every new pane launching
    /// a binary they deleted.
    ///
    /// A `[terminal]` header left with nothing under it goes too. Leaving it is
    /// how `hook` and `unhook` stop being each other's inverse: the header
    /// survives, so the next `hook` finds the table already there, writes into
    /// wherever it sits, and never runs the placement above.
    pub fn unhook(document: &str) -> String {
        let Ok(mut parsed) = document.parse::<toml_edit::DocumentMut>() else {
            return document.to_string();
        };

        let mut emptied = None;

        if let Some(table) = parsed.get_mut("terminal").and_then(|item| item.as_table_mut()) {
            table.remove("default_shell");

            if table.is_empty() {
                // The header carries the decor above it, which is the operator's
                // own comment or a plugin's fence terminator. Removing the table
                // would take that with it, so it returns to the document tail it
                // came from.
                emptied = Some(
                    table
                        .decor()
                        .prefix()
                        .and_then(|prefix| prefix.as_str())
                        .unwrap_or_default()
                        .to_string(),
                );
            }
        }

        if let Some(prefix) = emptied {
            parsed.remove("terminal");

            let tail = format!("{prefix}{}", parsed.trailing().as_str().unwrap_or_default());

            parsed.set_trailing(tail);
        }

        parsed.to_string()
    }

    /// Whether this path is safe to name as every pane's shell.
    ///
    /// A `default_shell` inside a build directory breaks every pane on the
    /// machine the next time somebody runs `cargo clean` — including panes
    /// opened for work that has nothing to do with tethera, and with no message
    /// pointing at the cause.
    pub fn installable(shim: &Path) -> Result<(), String> {
        let text = shim.to_string_lossy().replace('\\', "/");

        for build in ["/target/debug/", "/target/release/"] {
            if text.contains(build) {
                return Err(format!(
                    "{} is inside a build directory. herdr would launch it for every new pane, \
                     and the next `cargo clean` would leave this machine with no working \
                     terminal. install the binary somewhere stable first",
                    shim.display()
                ));
            }
        }

        // The same binary answers to two names and the name is the whole of the
        // dispatch: under any other one it parses its arguments as this CLI, and
        // a bare invocation with no subcommand is a usage error exiting 2.
        //
        // As `default_shell` that is not a degraded pane, it is no pane. Every
        // new tab, split and agent start on the machine failed at once, and
        // nothing in the failure named the config that caused it.
        let stem = shim
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();

        if stem != crate::terminal::Shim::ARGV0 {
            return Err(format!(
                "{} would not run as a shell. herdr launches `default_shell` with no arguments, \
                 and this binary only becomes the shim when it is called `{}` — under any other \
                 name it reads a subcommand and exits. copy or link it to `{}{}` and name that",
                shim.display(),
                crate::terminal::Shim::ARGV0,
                crate::terminal::Shim::ARGV0,
                std::env::consts::EXE_SUFFIX,
            ));
        }

        Ok(())
    }
}
