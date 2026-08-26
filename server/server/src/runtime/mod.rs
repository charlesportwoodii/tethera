use crate::config::ApplicationConfig;
use crate::services::TransportService;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tethera_transport::error::TransportError;

pub struct ServerRuntime {
    config: Arc<ApplicationConfig>,
    shutdown_flag: Arc<AtomicBool>,
}

impl ServerRuntime {
    pub fn new(config: Arc<ApplicationConfig>) -> Self {
        Self {
            config,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let transport = TransportService::bind(&self.config).await?;

        tracing::info!(endpoint_id = %transport.id(), "endpoint bound");

        loop {
            if self.shutdown_flag.load(Ordering::SeqCst) {
                break;
            }

            tokio::select! {
                accepted = transport.accept_bi() => {
                    match accepted {
                        Ok(_streams) => tracing::info!("accepted a control stream"),
                        // A closed endpoint never reopens. Logging and looping
                        // on it spins the core until the process is killed.
                        Err(TransportError::EndpointClosed) => {
                            tracing::info!("endpoint closed; stopping the accept loop");
                            break;
                        }
                        Err(error) => tracing::warn!(%error, "accept failed"),
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("shutdown requested");
                    self.request_shutdown();
                }
            }
        }

        Ok(())
    }

    pub fn request_shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
    }
}
