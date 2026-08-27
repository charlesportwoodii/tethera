use std::ffi::OsStr;
use std::path::Path;

/// Whether a command this machine would run is actually on it.
///
/// The agent catalog is a compile-time list of the harnesses this build knows
/// how to launch, which is not the same question as which ones a person has
/// installed. Answering the first where the second was asked puts a row in front
/// of somebody that fails the moment they tap it.
pub struct Installed;

impl Installed {
    /// The suffixes a bare name may carry on Windows.
    ///
    /// `PATHEXT` is what the shell itself consults, so a name that resolves only
    /// as `claude.cmd` — which is how every npm-installed tool arrives — is
    /// invisible to a check that looks for the bare name alone.
    const DEFAULT_PATHEXT: &'static str = ".COM;.EXE;.BAT;.CMD";

    /// Whether `name` resolves to something runnable on this machine.
    ///
    /// A path with a separator in it is taken as a path and nothing else, the
    /// same way a shell would: `PATH` is consulted only for a bare name.
    pub fn has(name: &str) -> bool {
        match std::env::var_os("PATH") {
            Some(path) => Self::has_in(name, &path),
            None => Self::has_in(name, OsStr::new("")),
        }
    }

    /// The same question against a search path the caller names.
    ///
    /// A behavioural seam rather than a builder, and the only safe way to test
    /// this: `PATH` is process-wide, so a test that set it would change what
    /// every other test in the binary resolves — including the one that spawns
    /// the real terminal backend.
    pub fn has_in(name: &str, search_path: &OsStr) -> bool {
        if name.is_empty() {
            return false;
        }

        if name.contains('/') || name.contains('\\') {
            return Self::runnable(Path::new(name));
        }

        std::env::split_paths(search_path).any(|directory| Self::found_in(&directory, name))
    }

    fn found_in(directory: &Path, name: &str) -> bool {
        if directory.as_os_str().is_empty() {
            return false;
        }

        Self::extensions()
            .iter()
            .any(|extension| Self::runnable(&directory.join(format!("{name}{extension}"))))
    }

    /// The suffixes to try, most specific last, always including none at all.
    fn extensions() -> Vec<String> {
        if !cfg!(windows) {
            return vec![String::new()];
        }

        let declared = std::env::var("PATHEXT")
            .unwrap_or_else(|_| Self::DEFAULT_PATHEXT.to_string());

        let mut found = vec![String::new()];
        found.extend(
            declared
                .split(';')
                .map(str::trim)
                .filter(|extension| !extension.is_empty())
                .map(str::to_owned),
        );

        found
    }

    fn runnable(path: &Path) -> bool {
        path.is_file()
    }
}
