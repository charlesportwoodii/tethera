use std::path::Path;

use tethera_server_lib::config::HerdrConfig;

const SAMPLE: &str = r#"# Herdr configuration
[terminal]
# Executable used for new interactive panes.
# Empty means $SHELL, then /bin/sh.
# default_shell = ""

# Startup mode for new interactive pane shells.
shell_mode = "auto"
"#;

fn shim() -> &'static Path {
    Path::new("C:\\Program Files\\tethera\\tethera-shim.exe")
}

// The operator's comments are the documentation for their own config. A rewrite
// that dropped them would be a silent act of vandalism on a file tethera does
// not own.
#[test]
fn hooking_preserves_every_comment_and_other_setting() {
    let hooked = HerdrConfig::hook(SAMPLE, shim());

    assert!(hooked.contains("# Herdr configuration"));
    assert!(hooked.contains("# Empty means $SHELL, then /bin/sh."));
    assert!(hooked.contains(r#"shell_mode = "auto""#));
    assert!(hooked.contains("tethera-shim.exe"));
}

// Idempotent, because an operator will run this more than once and a config that
// grew a line each time would be unreadable within a week.
#[test]
fn hooking_twice_is_the_same_as_hooking_once() {
    let once = HerdrConfig::hook(SAMPLE, shim());

    assert_eq!(HerdrConfig::hook(&once, shim()), once);
}

// Reverting must leave the setting unset rather than empty, so herdr falls back
// to its own default. An operator who tried tethera and stopped must not be left
// with every new pane launching a binary they deleted.
#[test]
fn unhooking_returns_the_setting_to_unset() {
    let hooked = HerdrConfig::hook(SAMPLE, shim());

    assert_eq!(
        HerdrConfig::hooked(&hooked).as_deref(),
        Some("C:\\Program Files\\tethera\\tethera-shim.exe")
    );
    assert_eq!(HerdrConfig::hooked(&HerdrConfig::unhook(&hooked)), None);
}

// A config with no `[terminal]` table at all still has to be hookable: that is
// what a default install looks like before anybody edits it.
#[test]
fn a_config_without_a_terminal_table_gains_one() {
    let hooked = HerdrConfig::hook("# nothing here yet\n", shim());

    assert_eq!(
        HerdrConfig::hooked(&hooked).as_deref(),
        Some("C:\\Program Files\\tethera\\tethera-shim.exe")
    );
    assert!(hooked.contains("# nothing here yet"));
}

// A config tethera cannot parse is one it must not rewrite. Returning the
// original unchanged is what makes the caller's backup-and-verify step able to
// notice nothing happened.
#[test]
fn an_unparseable_config_is_returned_untouched() {
    let broken = "[terminal\nthis is not toml";

    assert_eq!(HerdrConfig::hook(broken, shim()), broken);
    assert_eq!(HerdrConfig::unhook(broken), broken);
}

// A `default_shell` inside a build directory breaks every pane on the machine
// the next time somebody runs `cargo clean`, and nothing in the failure points
// at the cause.
#[test]
fn a_shim_inside_a_build_directory_is_refused() {
    let target = Path::new("C:\\src\\tethera\\target\\debug\\tethera-shim.exe");

    assert!(HerdrConfig::installable(target).is_err());
    assert!(HerdrConfig::installable(Path::new(
        "/home/me/src/tethera/target/release/tethera-shim"
    ))
    .is_err());
    assert!(HerdrConfig::installable(shim()).is_ok());
}

// A herdr plugin writes a region it owns and later rewrites it, closing it with
// a comment. Measured on a real config, carrying an `ez-corp.space-usage`
// block.
const FENCED: &str = r#"onboarding = false

[ui]
agent_panel_sort = "priority"

# --- added by ez-corp.space-usage (removed by `status-disable`) ---
[ui.sidebar.spaces]
rows = [
  ["state_icon", "workspace"],
]
# --- end ez-corp.space-usage ---
"#;

// `[terminal]` must not land inside a region another tool owns.
//
// A created table is appended, and appending puts it below the document's
// trailing comment — which is the plugin's fence terminator. Inside the fence,
// `status-disable` deletes it along with the plugin's own settings, and every
// new pane on the machine silently returns to a shell nobody chose.
#[test]
fn hooking_puts_the_new_table_below_a_plugins_closing_comment() {
    let hooked = HerdrConfig::hook(FENCED, shim());

    let fence = hooked
        .find("# --- end ez-corp.space-usage ---")
        .expect("the plugin's fence survives");
    let table = hooked.find("[terminal]").expect("the table was created");

    assert!(
        fence < table,
        "`[terminal]` was written inside the plugin's region:\n{hooked}"
    );
}

// Unhooking has to be the inverse of hooking, to the byte.
//
// The header is what breaks it: `default_shell` goes, `[terminal]` stays, and
// the next hook finds a table already present and writes into wherever the
// leftover sits — never running the placement above. An emptied header also
// carries the decor above it, so removing it must hand that comment back rather
// than delete it with the table.
#[test]
fn hooking_then_unhooking_leaves_the_document_exactly_as_it_was() {
    for original in [FENCED, "# nothing here yet\n"] {
        let restored = HerdrConfig::unhook(&HerdrConfig::hook(original, shim()));

        assert_eq!(restored, original, "the round trip changed the document");
    }
}

// An operator's own `[terminal]` table is left where they put it, and survives
// the round trip with everything else under it.
#[test]
fn unhooking_keeps_a_terminal_table_that_holds_anything_else() {
    let restored = HerdrConfig::unhook(&HerdrConfig::hook(SAMPLE, shim()));

    assert_eq!(restored, SAMPLE);
    assert!(restored.contains(r#"shell_mode = "auto""#));
}

// A path that is not the shim's name must be refused before it reaches the
// config.
//
// The binary answers to two names and the name is the whole of the dispatch:
// called anything else it parses its arguments as the CLI, and a bare
// invocation with no subcommand is a usage error exiting 2. Installed as
// `default_shell` that is not a degraded pane, it is no pane — measured, every
// new tab, split and agent start on the machine failed at once, and nothing in
// the failure named the config.
//
// The hook accepted `tethera.exe` until this existed.
#[test]
fn a_path_that_is_not_the_shims_name_is_refused() {
    let cli = Path::new(r"C:\Users\me\AppData\Local\tethera\tethera.exe");

    let error = HerdrConfig::installable(cli).expect_err("the CLI is not a shell");

    assert!(
        error.contains("tethera-shim"),
        "the refusal must name what to install instead: {error}"
    );

    assert!(HerdrConfig::installable(Path::new("/usr/local/bin/tethera")).is_err());
    assert!(HerdrConfig::installable(Path::new("/usr/local/bin/tethera-shim")).is_ok());
}
