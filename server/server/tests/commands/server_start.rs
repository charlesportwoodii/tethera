use clap::Parser;
use tethera_server_lib::commands::server::start::Config as StartConfig;
use tethera_server_lib::commands::server::ServerSubCommand;
use tethera_server_lib::commands::{Cli, SubCommand};
use tethera_server_lib::config::{ApplicationConfig, TerminalKind};

fn parse_detached_child(config: &ApplicationConfig) -> Cli {
    let mut argv = vec![std::ffi::OsString::from("tethera")];
    argv.extend(StartConfig::detached_arguments(config));

    Cli::try_parse_from(argv).expect("the detached child cannot parse its own argv")
}

fn start_command(cli: &Cli) -> &StartConfig {
    match &cli.cmd {
        SubCommand::Server(server) => match &server.cmd {
            ServerSubCommand::Start(start) => start,
            other => panic!("the detached child runs {other:?} instead of server start"),
        },
        other => panic!("the detached child runs {other:?} instead of server start"),
    }
}

#[test]
fn the_detached_child_reparses_every_global_the_parent_resolved() {
    let mut config = ApplicationConfig::with_data_dir(std::path::PathBuf::from("/tmp/tethera-test"));
    config.bind_port = 23890;
    config.terminal_backend = TerminalKind::Pty;
    config.label = Some("workshop".to_string());
    config.relay_url = Some("https://relay.example".to_string());

    let child = parse_detached_child(&config);

    assert_eq!(child.bind_port, config.bind_port);
    assert_eq!(child.terminal_backend, config.terminal_backend);
    assert_eq!(child.data_dir, Some(config.data_dir.clone()));
    assert_eq!(child.label, config.label);
    assert_eq!(child.relay_url, config.relay_url);
}

#[test]
fn the_detached_child_is_not_told_to_detach_again() {
    let config = ApplicationConfig::with_data_dir(std::path::PathBuf::from("/tmp/tethera-test"));

    let child = parse_detached_child(&config);

    assert!(!start_command(&child).detach);
}

#[test]
fn the_relay_token_never_reaches_the_detached_child_argv() {
    let mut config = ApplicationConfig::with_data_dir(std::path::PathBuf::from("/tmp/tethera-test"));
    config.relay_token = Some("shared-secret".to_string());

    let argv = StartConfig::detached_arguments(&config);

    assert!(!argv.iter().any(|argument| argument == "shared-secret"));
}
