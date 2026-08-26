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
