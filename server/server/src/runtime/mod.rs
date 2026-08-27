use crate::config::ApplicationConfig;
use crate::machine::MachineAddress;
use crate::protocol::{Dispatcher, LivePorts};
use crate::services::TransportService;
use crate::storage::Storage;
use iroh::endpoint::Connection;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tethera_common::protocol::close::CloseCode;
use tethera_transport::error::TransportError;
use tokio::sync::Semaphore;

pub struct ServerRuntime {
    config: Arc<ApplicationConfig>,
    shutdown_flag: Arc<AtomicBool>,
}

impl ServerRuntime {
    /// How often the running server rewrites where it can be reached, and asks
    /// the terminal backend whether it is still there.
    ///
    /// `tethera pair` has no channel to this process, so a record rewritten on a
    /// timer is what lets it tell a running server from a dead one. Three of
    /// these is the staleness bound.
    pub const ADDRESS_HEARTBEAT: Duration = Duration::from_secs(30);

    /// How often a watched tree is re-read, and so how quickly a conversation's
    /// status changes on a phone.
    ///
    /// Short because it is what a person reads as the screen being alive, and
    /// affordable because it runs only while a machine watch is open — a herdr
    /// snapshot measured at 15 ms, against a client-side poll that costs a full
    /// listing per client per tick.
    pub const TREE_HEARTBEAT: Duration = Duration::from_secs(5);

    pub fn new(config: Arc<ApplicationConfig>) -> Self {
        Self {
            config,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let transport = Arc::new(TransportService::bind(&self.config).await?);

        tracing::info!(endpoint_id = %transport.id(), "endpoint bound");

        let db = Arc::new(Storage::connect(&self.config).await?);
        let ports =
            LivePorts::new_shared(self.config.clone(), db, transport.id().to_string()).await;

        tracing::info!(
            capabilities = ?ports.capabilities(),
            "serving the protocol"
        );

        self.serve(transport, ports).await
    }

    /// The accept loop, over a transport and ports somebody else built.
    ///
    /// Separate from `start` so a test can drive the real loop over a loopback
    /// endpoint. Both arguments are things `start` builds itself, so nothing
    /// internal is exposed by taking them.
    ///
    /// Ctrl-C is the only thing that ends this. The shutdown flag is read at the
    /// top of each iteration and the loop then blocks in `select!`, so setting it
    /// from elsewhere does not wake it - and nothing outside can reach it
    /// anyway, since `start` owns the only `ServerRuntime`. A real stop channel
    /// is a `CancellationToken` third arm, which is cancel-safe and so would not
    /// reintroduce the handshake cancellation the two-arm shape exists to avoid.
    pub async fn serve(
        &self,
        transport: Arc<TransportService>,
        ports: Arc<LivePorts>,
    ) -> anyhow::Result<()> {
        let dispatcher = Arc::new(Dispatcher::new(ports.clone()));
        let connections = Arc::new(Semaphore::new(self.config.max_connections));
        let heartbeat = self.spawn_heartbeat(transport.clone(), ports);

        loop {
            if self.shutdown_flag.load(Ordering::SeqCst) {
                break;
            }

            // Only two arms, and both are safe to cancel. `accept` completes a
            // QUIC handshake, which is not: a third arm winning this race would
            // drop a connection mid-handshake every time it fired, and the peer
            // would see a failed dial with nothing logged here. That is why the
            // heartbeat is a task of its own rather than a tick in this select.
            tokio::select! {
                accepted = transport.accept() => {
                    match accepted {
                        Ok(connection) => Self::spawn_connection(
                            &dispatcher,
                            &connections,
                            connection,
                            self.config.max_connections,
                        ),
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

        heartbeat.abort();

        // A dead server that left its addresses behind would put them in the
        // next pairing QR, and the phone that scanned it would dial nothing.
        MachineAddress::clear(&self.config);

        Ok(())
    }

    pub fn request_shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
    }

    // Spawned, never awaited. Serving inline parks the accept loop behind one
    // peer, and a phone on a flaky path that connects and opens nothing would
    // then lock every other device out of the machine.
    fn spawn_connection(
        dispatcher: &Arc<Dispatcher<LivePorts>>,
        connections: &Arc<Semaphore>,
        connection: Connection,
        limit: usize,
    ) {
        let Ok(permit) = connections.clone().try_acquire_owned() else {
            tracing::warn!(
                remote = %connection.remote_id(),
                limit,
                "refusing a connection: this machine is already serving its limit"
            );
            // A transport-level close rather than a `Refuse` frame: there is no
            // handshake to answer on, and `RefuseReason` has no variant for
            // load. QUIC carries both the code and the reason to the peer, so
            // this is legible as "busy, try again" rather than as a lost
            // connection - which sends a person somewhere different.
            let refusal = CloseCode::AtCapacity;
            connection.close(refusal.as_u32().into(), refusal.reason());

            return;
        };

        let dispatcher = dispatcher.clone();

        tokio::spawn(async move {
            if let Err(error) = dispatcher.serve_connection(connection).await {
                tracing::warn!(%error, "connection ended in error");
            }

            drop(permit);
        });
    }

    fn spawn_heartbeat(
        &self,
        transport: Arc<TransportService>,
        ports: Arc<LivePorts>,
    ) -> tokio::task::JoinHandle<()> {
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut addresses = tokio::time::interval(Self::ADDRESS_HEARTBEAT);
            let mut tree = tokio::time::interval(Self::TREE_HEARTBEAT);

            loop {
                tokio::select! {
                    _ = addresses.tick() => {
                        Self::publish_address(&config, &transport);
                        ports.reprobe_terminals().await;
                    }
                    // Its own interval, because the two answer different
                    // questions. Where this machine can be reached changes when
                    // a network does; what an agent is doing changes while
                    // somebody watches it, and at thirty seconds a status mark
                    // is stale for most of its life.
                    //
                    // Affordable only because it does nothing with nobody
                    // watching. See `LivePorts::refresh_watched_tree`.
                    _ = tree.tick() => {
                        let _ = ports.refresh_watched_tree().await;
                    }
                }
            }
        })
    }

    fn publish_address(config: &ApplicationConfig, transport: &TransportService) {
        let record =
            MachineAddress::from_endpoint_addr(&transport.addr(), chrono::Utc::now().timestamp());

        if let Err(error) = record.publish(config) {
            tracing::warn!(%error, "could not record where this machine is reachable");
        }
    }
}
