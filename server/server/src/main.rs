#[tokio::main]
async fn main() {
    // Installed against the default data directory rather than the one a
    // `--data-dir` flag may name, because the flag is parsed inside `Cli::run`
    // and a failure before that point would otherwise have nowhere to go.
    let config = tethera_server_lib::ApplicationConfig::default();

    if let Err(error) = tethera_server_lib::logging::Logging::install(&config) {
        eprintln!("could not install logging: {error}");
    }

    tethera_server_lib::commands::Cli::run().await;
}
