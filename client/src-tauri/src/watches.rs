use std::collections::HashMap;

use iroh::endpoint::Connection;
use tauri::{AppHandle, Emitter};
use tethera_client_core::watch::Watch;
use tethera_common::protocol::watch::{WatchOpen, WatchSpec};
use tethera_common::structs::client::ConversationEvent;
use tethera_common::structs::conversation::Conversation;
use tethera_common::structs::ids::ConversationId;
use tethera_common::structs::primitives::Cursor;
use tokio::sync::{oneshot, Mutex};

/// Every live conversation subscription this app holds.
///
/// One per conversation, keyed by its id. Opening a second watch on the same
/// conversation replaces the first: two subscriptions would emit every turn
/// twice, and a screen that dedupes by turn id would hide the fault rather than
/// fix it.
pub struct ConversationWatches {
    live: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

impl ConversationWatches {
    /// The Tauri event every watch emits on.
    pub const CHANNEL: &'static str = "conversation";

    /// Announces a tail that stopped for a reason nobody asked for.
    ///
    /// A separate channel rather than a new `WatchEvent` variant, because
    /// `WatchEvent` is a wire type: it is encoded with postcard, whose variants
    /// are positional, and this fact never crosses the wire. It is one process
    /// telling its own webview that the stream it was reading has gone.
    ///
    /// It exists because a silent end is indistinguishable from a quiet
    /// conversation. The screen kept saying it was following, the last question
    /// it had been told about stayed on screen, and answering it did nothing at
    /// all - the live question the answer needed had stopped arriving minutes
    /// earlier.
    pub const ENDED: &'static str = "conversation_tail_ended";

    pub fn new() -> Self {
        Self {
            live: Mutex::new(HashMap::new()),
        }
    }

    /// Subscribes, and answers with the snapshot the machine opened with.
    ///
    /// The connection is shared with every other request to this machine and is
    /// owned by `AppState`, so this holds a clone to keep it alive and never
    /// closes it. Closing here would take every other screen's requests down
    /// with the watch.
    pub async fn start(
        &self,
        app: AppHandle,
        connection: Connection,
        id: ConversationId,
        after: Option<Cursor>,
    ) -> Result<(Conversation, Cursor), String> {
        self.stop(&id).await;

        let spec = WatchSpec::Conversation {
            id: id.clone(),
            after,
        };

        let (opened, watch) = Watch::open(&connection, spec)
            .await
            .map_err(|error| error.to_string())?;

        let WatchOpen::Conversation { conversation, from } = opened else {
            return Err("the machine answered a conversation watch with a machine snapshot".into());
        };

        let (cancel, cancelled) = oneshot::channel();

        tokio::spawn(Self::pump(app, connection, watch, id.clone(), cancelled));

        self.live.lock().await.insert(id.as_str().to_owned(), cancel);

        Ok((conversation, from))
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
        id: ConversationId,
        mut cancelled: oneshot::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                // Either a deliberate stop or the sender dropped with the
                // registry. Both mean nobody is listening.
                _ = &mut cancelled => break,

                received = watch.next() => match received {
                    Ok(Some(event)) => {
                        let notice = ConversationEvent {
                            conversation: id.clone(),
                            event,
                        };

                        // A failed emit means the webview is gone, which is the
                        // end of anything this task could usefully do.
                        if app.emit(Self::CHANNEL, notice).is_err() {
                            break;
                        }
                    }
                    // The machine finished the stream. Not a failure: the
                    // conversation ended, or the machine is shutting down.
                    Ok(None) => break,
                    Err(error) => {
                        log::warn!("conversation watch ended: {error}");

                        // Said out loud, so the screen can open another one. A
                        // QUIC path closed for being idle is the ordinary fate
                        // of a phone left alone for a minute, and it must not be
                        // the end of the live tail for the life of the screen.
                        app.emit(Self::ENDED, id.as_str().to_owned()).ok();

                        break;
                    }
                },
            }
        }

        watch.close().await;
        drop(connection);
    }

    /// Ends a subscription if one is running. Silent when none is.
    pub async fn stop(&self, id: &ConversationId) {
        if let Some(cancel) = self.live.lock().await.remove(id.as_str()) {
            cancel.send(()).ok();
        }
    }
}
