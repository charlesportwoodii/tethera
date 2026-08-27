use iroh::endpoint::Connection;
use tauri::{AppHandle, Emitter};
use tethera_client_core::watch::Watch;
use tethera_common::protocol::watch::{WatchOpen, WatchSpec};
use tethera_common::structs::client::{MachineEvent, MachineTree};
use tokio::sync::{oneshot, Mutex};

/// The live tree subscription this app holds.
///
/// One at a time rather than a map keyed by machine: the phone shows one
/// machine's tree at a time, and a second subscription on the same machine would
/// emit every event twice — which a screen that deduped by id would hide rather
/// than fix.
pub struct MachineWatch {
    live: Mutex<Option<oneshot::Sender<()>>>,
}

impl MachineWatch {
    /// The Tauri event every machine watch emits on.
    pub const CHANNEL: &'static str = "machine";

    /// Announces a tree subscription that stopped without being asked to.
    ///
    /// A separate channel rather than a `WatchEvent` variant, because
    /// `WatchEvent` is a wire type encoded with postcard, and this fact never
    /// crosses the wire. It is one process telling its own webview that the
    /// stream it was reading has gone.
    ///
    /// It exists because a silent end is indistinguishable from a machine where
    /// nothing is happening — which is exactly the failure this whole watch was
    /// added to end.
    pub const ENDED: &'static str = "machine_watch_ended";

    pub fn new() -> Self {
        Self {
            live: Mutex::new(None),
        }
    }

    /// Subscribes, and answers with the tree the machine opened with.
    ///
    /// The connection is shared with every other request to this machine and is
    /// owned by `AppState`, so this holds a clone to keep it alive and never
    /// closes it. Closing here would take every other screen's requests down
    /// with the watch.
    pub async fn start(
        &self,
        app: AppHandle,
        connection: Connection,
        server: String,
    ) -> Result<MachineTree, String> {
        self.stop().await;

        let (opened, watch) = Watch::open(&connection, WatchSpec::Machine)
            .await
            .map_err(|error| error.to_string())?;

        let WatchOpen::Machine {
            workspaces,
            tabs,
            panes,
            conversations,
            layouts,
        } = opened
        else {
            return Err("the machine answered a tree watch with a conversation snapshot".into());
        };

        let (cancel, cancelled) = oneshot::channel();

        tokio::spawn(Self::pump(app, connection, watch, server, cancelled));

        *self.live.lock().await = Some(cancel);

        Ok(MachineTree {
            workspaces,
            tabs,
            panes,
            conversations,
            layouts,
        })
    }

    /// Reads events until the machine finishes the stream or the screen closes.
    ///
    /// Cancellation is a signal the loop selects on rather than an abort. An
    /// aborted task never reaches the close below, so the machine would see
    /// every closed screen as a peer that vanished and could not tell that from
    /// a phone that lost its route.
    async fn pump(
        app: AppHandle,
        connection: Connection,
        mut watch: Watch,
        server: String,
        mut cancelled: oneshot::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                // Either a deliberate stop or the sender dropped with the
                // registry. Both mean nobody is listening.
                _ = &mut cancelled => break,

                received = watch.next() => match received {
                    Ok(Some(event)) => {
                        let notice = MachineEvent {
                            server: server.clone(),
                            event,
                        };

                        // A failed emit means the webview is gone, which is the
                        // end of anything this task could usefully do.
                        if app.emit(Self::CHANNEL, notice).is_err() {
                            break;
                        }
                    }
                    // The machine finished the stream. Not a failure: it is
                    // shutting down, or it has nothing further to say.
                    Ok(None) => break,
                    Err(error) => {
                        log::warn!("machine watch ended: {error}");

                        // Said out loud, so the screen can open another one. A
                        // QUIC path closed for being idle is the ordinary fate
                        // of a phone left alone for a minute, and it must not
                        // leave the tab strip frozen for the life of the screen.
                        app.emit(Self::ENDED, server.clone()).ok();

                        break;
                    }
                },
            }
        }

        watch.close().await;
        drop(connection);
    }

    /// Ends the subscription if one is running. Silent when none is.
    pub async fn stop(&self) {
        if let Some(cancel) = self.live.lock().await.take() {
            cancel.send(()).ok();
        }
    }
}
