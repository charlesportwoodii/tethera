use std::sync::Arc;

use crate::terminal::relay::ShimRelay;

/// Accepts shims on the local channel, for the life of the process.
///
/// Lives beside the registry a shim adopts into rather than in a CLI command,
/// because a pane announces itself whenever it opens — including one split by
/// hand at the desk — so there is nothing to discover and nothing to poll.
pub struct ShimListener;

impl ShimListener {
    /// Serves the channel on its own task.
    ///
    /// A refused shim is logged and dropped. It is never fatal: the pane on the
    /// other end is a working terminal whether or not this side understood its
    /// greeting, and taking the accept loop down would cost every *other* pane
    /// on the machine.
    pub fn spawn(relay: Arc<ShimRelay>, address: String) {
        tokio::spawn(async move {
            if let Err(error) = Self::accept(relay, &address).await {
                tracing::warn!(%error, address, "the shim channel stopped accepting");
            }
        });
    }

    #[cfg(windows)]
    async fn accept(relay: Arc<ShimRelay>, address: &str) -> anyhow::Result<()> {
        use tokio::net::windows::named_pipe::ServerOptions;

        // The next instance exists before the current one is handed off, which is
        // how a named pipe server stays reachable. Creating it afterwards leaves a
        // window where a dialling client gets `ERROR_PIPE_BUSY` — and a shim opens
        // two channels back to back, so it loses that race routinely rather than
        // rarely.
        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(address)?;

        loop {
            server.connect().await?;

            let connected =
                std::mem::replace(&mut server, ServerOptions::new().create(address)?);
            let relay = Arc::clone(&relay);

            tokio::spawn(async move {
                if let Err(error) = Arc::clone(&relay).serve(connected).await {
                    tracing::warn!(%error, "a shim was refused");
                }
            });
        }
    }

    #[cfg(unix)]
    async fn accept(relay: Arc<ShimRelay>, address: &str) -> anyhow::Result<()> {
        use tokio::net::UnixListener;

        // A socket file outlives the process that made it, so a restart binds a
        // path that already exists and fails. Removed rather than reused: the old
        // file names a socket nothing is listening on.
        let _ = std::fs::remove_file(address);

        let listener = UnixListener::bind(address)?;

        loop {
            let (stream, _) = listener.accept().await?;
            let relay = Arc::clone(&relay);

            tokio::spawn(async move {
                if let Err(error) = Arc::clone(&relay).serve(stream).await {
                    tracing::warn!(%error, "a shim was refused");
                }
            });
        }
    }
}
