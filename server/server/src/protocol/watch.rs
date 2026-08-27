use crate::protocol::ports::{ConversationPort, MachinePort, Ports};
use tethera_common::protocol::watch::{WatchEvent, WatchOpen, WatchSpec};
use tethera_transport::error::TransportError;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;
use tokio::sync::broadcast::error::RecvError;

pub struct Watch;

impl Watch {
    /// How often a machine watch re-reads the tree.
    ///
    /// The tree is not push-based: `TreeWatcher` diffs successive reads, and
    /// nothing reads unprompted except the address heartbeat, thirty seconds
    /// apart. A pane that exits is invisible for that whole window, which is
    /// what made the tab strip read as broken.
    ///
    /// Two seconds, and only while somebody is watching. A read is one backend
    /// call under the same admission gate as every other, so an idle machine
    /// spends nothing and a watched one spends one call per interval. The cost
    /// is that `WireError::Busy` stops being exceptional: every consumer of a
    /// terminal call has to keep meaning "this machine could not tell" rather
    /// than "there is nothing there".
    const TREE_POLL: std::time::Duration = std::time::Duration::from_secs(2);

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
                        layouts: tree.layouts,
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

        // A conversation has a real event source and needs no poll; the tree
        // does not, and that is what the branch below exists for.
        let polls = matches!(spec, WatchSpec::Machine);

        let mut poll = tokio::time::interval(Self::TREE_POLL);

        // The default would fire a backlog of ticks at once after a slow backend
        // call, which is a burst of subprocess calls at exactly the moment the
        // machine is already struggling.
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                received = events.recv() => match received {
                    Ok(event) => FrameIo::write(&mut send, codec, &event).await?,

                    // A slow consumer that missed events has a stale tree and no
                    // way to know it. Streaming on would leave it permanently
                    // wrong, so the answer is a fresh snapshot rather than a
                    // logged warning.
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
                },

                // The read is what makes the diff happen; whatever it changed
                // arrives on the branch above. Nothing is written here, and the
                // error is dropped on purpose - a tree that could not be read is
                // reported by the request that asked for it, not by a watch that
                // would otherwise close on one slow poll.
                _ = poll.tick(), if polls => {
                    let _ = ports.machine().tree().await;
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
                            layouts: tree.layouts,
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
