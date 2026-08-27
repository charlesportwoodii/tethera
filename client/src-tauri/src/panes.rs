use std::collections::HashMap;

use iroh::endpoint::Connection;
use tauri::{AppHandle, Emitter};
use tethera_client_core::attach::{Attach, AttachFrames, AttachInput};
use tethera_common::protocol::terminal::{AttachSpec, TerminalInput};
use tethera_common::structs::client::PaneFrame;
use tethera_common::structs::ids::PaneId;
use tokio::sync::{oneshot, Mutex};

/// Every live pane this app is attached to.
///
/// One per pane, keyed by its id. Attaching a second time to the same pane
/// replaces the first: two streams would deliver every frame twice, and a grid
/// that applied both would look right for a snapshot and drift on damage, which
/// is the hardest kind of wrong to notice.
pub struct PaneAttachments {
    live: Mutex<HashMap<String, Live>>,
}

struct Live {
    input: AttachInput,
    cancel: oneshot::Sender<()>,
}

impl PaneAttachments {
    /// The Tauri event every attach emits frames on.
    pub const CHANNEL: &'static str = "terminal";

    /// Announces an attach that stopped for a reason nobody asked for.
    ///
    /// A separate channel rather than a `TerminalFrame` variant, because
    /// `TerminalFrame` is a wire type encoded with postcard, whose variants are
    /// positional, and this fact never crosses the wire. It is one process
    /// telling its own webview that the stream it was reading has gone.
    ///
    /// It exists because a silent end is indistinguishable from a quiet pane.
    /// The screen would keep saying it was following, the last output would stay
    /// on it, and every key pressed afterwards would go into a stream that
    /// stopped minutes earlier.
    pub const ENDED: &'static str = "terminal_attach_ended";

    pub fn new() -> Self {
        Self {
            live: Mutex::new(HashMap::new()),
        }
    }

    /// Attaches, and starts pumping frames at the webview.
    ///
    /// The connection is shared with every other request to this machine and is
    /// owned by `AppState`, so this holds a clone to keep it alive and never
    /// closes it. Closing here would take every other screen's requests down
    /// with the attach.
    pub async fn start(
        &self,
        app: AppHandle,
        connection: Connection,
        spec: AttachSpec,
    ) -> Result<(), String> {
        let pane = spec.pane.clone();

        self.stop(&pane).await;

        log::info!("attaching to {} as {:?}", pane.as_str(), spec.view);

        let (frames, input) = Attach::open(&connection, spec)
            .await
            .inspect_err(|error| log::warn!("attach to {} refused: {error}", pane.as_str()))
            .map_err(|error| error.to_string())?;

        let (cancel, cancelled) = oneshot::channel();

        tokio::spawn(Self::pump(app, connection, frames, pane.clone(), cancelled));

        self.live
            .lock()
            .await
            .insert(pane.as_str().to_owned(), Live { input, cancel });

        Ok(())
    }

    /// Sends one keypress or one piece of text to an attached pane.
    ///
    /// A pane with no live attach is an error naming that, never a silent
    /// success. The stream is the only route input has, so returning `Ok` here
    /// would leave somebody pressing a control that does nothing and no way to
    /// find out why.
    pub async fn send(&self, pane: &PaneId, input: TerminalInput) -> Result<(), String> {
        let mut live = self.live.lock().await;

        let held = live
            .get_mut(pane.as_str())
            .ok_or_else(|| format!("nothing is attached to {}", pane.as_str()))?;

        held.input
            .send(input)
            .await
            .map_err(|error| error.to_string())
    }

    /// Reads frames until the machine finishes the stream or the screen closes.
    ///
    /// Cancellation is a signal the loop selects on rather than an abort. An
    /// aborted task never reaches the detach below, so the machine would see
    /// every closed screen as a peer that vanished and could not tell that from
    /// a phone that lost its route.
    async fn pump(
        app: AppHandle,
        connection: Connection,
        mut frames: AttachFrames,
        pane: PaneId,
        mut cancelled: oneshot::Receiver<()>,
    ) {
        // Counted so the log distinguishes a stream that never carried anything
        // from one that carried a screen and then stopped. Those are different
        // faults and they look identical on the phone.
        let mut carried = 0_u64;

        loop {
            tokio::select! {
                // Either a deliberate detach or the sender dropped with the
                // registry. Both mean nobody is reading this pane.
                _ = &mut cancelled => {
                    log::info!("detached from {} after {carried} frames", pane.as_str());

                    break;
                }

                received = frames.next() => match received {
                    Ok(Some(frame)) => {
                        carried += 1;

                        let addressed = PaneFrame {
                            pane: pane.clone(),
                            frame,
                        };

                        // A failed emit means the webview is gone, which is the
                        // end of anything this task could usefully do.
                        if app.emit(Self::CHANNEL, addressed).is_err() {
                            log::warn!(
                                "the webview stopped taking frames for {} after {carried}",
                                pane.as_str()
                            );

                            break;
                        }
                    }
                    // The machine finished the stream. Not a failure: the pane
                    // closed, or the machine is shutting down, and it said which
                    // in a `Closed` frame before ending.
                    Ok(None) => {
                        log::info!(
                            "the machine finished the stream for {} after {carried} frames",
                            pane.as_str()
                        );

                        break;
                    }
                    Err(error) => {
                        log::warn!(
                            "terminal attach for {} ended after {carried} frames: {error}",
                            pane.as_str()
                        );

                        // Said out loud, so the screen can open another one. A
                        // QUIC path closed for being idle is the ordinary fate
                        // of a phone left alone for a minute, and it must not be
                        // the end of a terminal for the life of the screen.
                        app.emit(Self::ENDED, pane.as_str().to_owned()).ok();

                        break;
                    }
                },
            }
        }

        drop(connection);
    }

    /// Ends an attach if one is running. Silent when none is.
    ///
    /// The pane keeps running on the machine: detaching is not closing.
    pub async fn stop(&self, pane: &PaneId) {
        if let Some(held) = self.live.lock().await.remove(pane.as_str()) {
            held.cancel.send(()).ok();
            held.input.detach();
        }
    }
}
