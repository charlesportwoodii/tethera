#[tokio::main]
async fn main() {
    // Installed against the default data directory rather than the one a
    // `--data-dir` flag may name, because the flag is parsed inside `Cli::run`
    // and a failure before that point would otherwise have nowhere to go.
    let config = tethera_server_lib::ApplicationConfig::default();

    if let Err(error) = tethera_server_lib::logging::Logging::install(&config) {
        eprintln!("could not install logging: {error}");
    }

    // Before any argument parsing. Under the shim's name this process is a
    // pane's shell, and a shell's arguments are not this CLI's - clap would exit
    // on them, which as a `default_shell` means a pane that never starts.
    if tethera_server_lib::commands::Cli::invoked_as_shim() {
        tethera_server_lib::commands::Cli::run_as_shim().await;

        return;
    }

    tethera_server_lib::commands::Cli::run().await;
}
