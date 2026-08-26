#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("TETHERA_LOG")
                .unwrap_or_else(|_| {
                    tracing_subscriber::EnvFilter::new(
                        "info,sqlx::query=warn,sea_orm_migration=warn",
                    )
                }),
        )
        .init();

    tethera_server_lib::commands::Cli::run().await;
}
