use std::ffi::OsString;
use tethera_server_lib::machine::Installed;

/// A directory holding files that stand in for installed commands.
///
/// Handed to `has_in` as the whole search path rather than exported into the
/// process. `PATH` is process-wide, and a test that set it would change what
/// every other test in this binary resolves — the one that spawns the real
/// terminal backend included.
struct Somewhere {
    dir: tempfile::TempDir,
}

impl Somewhere {
    fn with(names: &[&str]) -> Self {
        let dir = tempfile::tempdir().expect("a directory");

        for name in names {
            std::fs::write(dir.path().join(name), b"").expect("a command");
        }

        Self { dir }
    }

    fn search_path(&self) -> OsString {
        self.dir.path().as_os_str().to_owned()
    }
}

#[test]
fn a_command_in_a_search_path_directory_is_installed() {
    let somewhere = Somewhere::with(&["claude"]);

    assert!(Installed::has_in("claude", &somewhere.search_path()));
    assert!(!Installed::has_in("codex", &somewhere.search_path()));
}

// Every npm-installed tool on Windows arrives as a `.cmd` shim and nothing else,
// so a check for the bare name alone reports the machine's own agent as absent
// and the catalog goes empty on a machine that has one.
#[cfg(windows)]
#[test]
fn a_command_that_exists_only_under_a_pathext_suffix_is_installed() {
    let somewhere = Somewhere::with(&["claude.cmd"]);

    assert!(Installed::has_in("claude", &somewhere.search_path()));
}

// A name is asked of the search path; a path is asked of itself. Resolving a
// path through the search path would report a command as installed because
// something of that name sat in a directory the person never named.
#[test]
fn a_path_is_taken_as_a_path_and_never_looked_for_on_the_search_path() {
    let somewhere = Somewhere::with(&["claude"]);
    let named = somewhere
        .dir
        .path()
        .join("claude")
        .to_string_lossy()
        .replace('\\', "/");

    assert!(Installed::has_in(&named, &somewhere.search_path()));
    assert!(!Installed::has_in("./claude", &somewhere.search_path()));
}

#[test]
fn nothing_is_installed_under_an_empty_name() {
    assert!(!Installed::has_in("", &Somewhere::with(&[]).search_path()));
}

#[test]
fn nothing_is_installed_when_the_search_path_is_empty() {
    assert!(!Installed::has_in("claude", &OsString::new()));
}

// A directory is not a command. Without the file check, any directory whose name
// matched would report the harness as installed and every start would fail.
#[test]
fn a_directory_of_the_right_name_is_not_a_command() {
    let somewhere = Somewhere::with(&[]);
    std::fs::create_dir(somewhere.dir.path().join("claude")).expect("a directory");

    assert!(!Installed::has_in("claude", &somewhere.search_path()));
}
