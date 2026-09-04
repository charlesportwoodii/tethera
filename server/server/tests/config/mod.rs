mod herdr_config;

use tethera_server_lib::config::ApplicationConfig;

#[test]
fn every_stored_path_sits_under_the_configured_data_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = ApplicationConfig::with_data_dir(dir.path().to_path_buf());

    for path in [
        config.identity_path(),
        config.database_path(),
        config.pid_path(),
    ] {
        assert!(
            path.starts_with(dir.path()),
            "{path:?} escaped the data dir"
        );
    }
}

#[test]
fn the_database_url_points_at_the_database_path_and_creates_on_open() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = ApplicationConfig::with_data_dir(dir.path().to_path_buf());

    let url = config.database_url();

    assert!(url.starts_with("sqlite://"), "unexpected scheme in {url}");
    assert!(url.ends_with("?mode=rwc"), "missing rwc mode in {url}");
}

#[test]
fn the_default_data_dir_is_absolute_so_the_cwd_never_changes_where_state_lives() {
    assert!(ApplicationConfig::default().data_dir.is_absolute());
}

// The shim runs as every pane's shell once herdr's default_shell names it,
// including when the data directory cannot be created. Reaching a shell matters
// more than reaching a data directory, so the shim is dispatched before the
// checks every other subcommand genuinely needs.
#[test]
fn the_shim_is_recognised_before_the_data_directory_is_touched() {
    use clap::Parser;
    use tethera_server_lib::commands::Cli;

    let shim = Cli::try_parse_from(["tethera", "shim", "--shell", "pwsh.exe"]).expect("parse");

    assert!(shim.is_shim());

    let other = Cli::try_parse_from(["tethera", "server", "start"]).expect("parse");

    assert!(!other.is_shim());
}
