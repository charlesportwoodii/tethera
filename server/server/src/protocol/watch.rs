use crate::protocol::ports::{ConversationPort, MachinePort, Ports};
use tethera_common::protocol::watch::{WatchEvent, WatchOpen, WatchSpec};
use tethera_transport::error::TransportError;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;
use tokio::sync::broadcast::error::RecvError;

pub struct Watch;

impl Watch {
    /// One snapshot, then events until either end stops.
    pub async fn serve<P: Ports>(
        ports: &P,
        codec: &FrameCodec,
        spec: WatchSpec,
        mut send: iroh::endpoint::SendStream,
        _recv: iroh::endpoint::RecvStream,
    ) -> Result<(), TransportError> {
        let mut events = match &spec {
            WatchSpec::Machine => {
                let tree = match ports.machine().tree().await {
                    Ok(tree) => tree,
                    Err(_) => return Ok(()),
                };

                FrameIo::write(
                    &mut send,
                    codec,
                    &WatchOpen::Machine {
                        workspaces: tree.workspaces,
                        tabs: tree.tabs,
                        panes: tree.panes,
                        conversations: tree.conversations,
                    },
                )
                .await?;

                ports.machine().tree_events()
            }
            WatchSpec::Conversation { id, after } => {
                let conversation = match ports.conversations().get(id).await {
                    Ok(conversation) => conversation,
                    Err(_) => return Ok(()),
                };

                let (from, events) = match ports.conversations().subscribe(id, after.clone()).await
                {
                    Ok(pair) => pair,
                    Err(_) => return Ok(()),
                };

                FrameIo::write(
                    &mut send,
                    codec,
                    &WatchOpen::Conversation { conversation, from },
                )
                .await?;

                events
            }
        };

        loop {
            match events.recv().await {
                Ok(event) => FrameIo::write(&mut send, codec, &event).await?,

                // A slow consumer that missed events has a stale tree and no way
                // to know it. Streaming on would leave it permanently wrong, so
                // the answer is a fresh snapshot rather than a logged warning.
                Err(RecvError::Lagged(_)) => {
                    Self::resnapshot(ports, codec, &spec, &mut send).await?
                }

                // The sender is gone: the machine is shutting down or the
                // conversation ended. Either way there is nothing further to
                // send, and finishing is not a failure.
                Err(RecvError::Closed) => {
                    send.finish().ok();

                    return Ok(());
                }
            }
        }
    }

    async fn resnapshot<P: Ports>(
        ports: &P,
        codec: &FrameCodec,
        spec: &WatchSpec,
        send: &mut iroh::endpoint::SendStream,
    ) -> Result<(), TransportError> {
        match spec {
            WatchSpec::Machine => {
                if let Ok(tree) = ports.machine().tree().await {
                    FrameIo::write(
                        send,
                        codec,
                        &WatchOpen::Machine {
                            workspaces: tree.workspaces,
                            tabs: tree.tabs,
                            panes: tree.panes,
                            conversations: tree.conversations,
                        },
                    )
                    .await?;
                }
            }
            WatchSpec::Conversation { id, .. } => {
                if let Ok(conversation) = ports.conversations().get(id).await {
                    // Resuming from the newest cursor the client could hold: it
                    // missed events, so anything older would be re-sent.
                    if let Ok((from, _)) = ports.conversations().subscribe(id, None).await {
                        FrameIo::write(
                            send,
                            codec,
                            &WatchOpen::Conversation { conversation, from },
                        )
                        .await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Whether an event belongs on a conversation watch rather than a machine
    /// watch.
    ///
    /// Stated here so the two watch kinds cannot drift into sending each other's
    /// events, which a client would render in the wrong screen.
    pub fn is_conversation_event(event: &WatchEvent) -> bool {
        matches!(
            event,
            WatchEvent::Turn(_) | WatchEvent::Blocked { .. } | WatchEvent::Unblocked { .. }
        )
    }
}
