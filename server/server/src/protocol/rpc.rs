use crate::protocol::dispatch::Session;
use crate::protocol::ports::{
    AssetPort, ConversationPort, MachinePort, Ports, TerminalPort,
};
use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::protocol::request::Request;
use tethera_common::protocol::response::{Payload, Progress, ProgressStage, Response};
use tethera_transport::error::TransportError;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

pub struct Rpc;

impl Rpc {
    /// One request, one terminal response.
    ///
    /// Zero or more `Progress` frames may precede it. Every port error becomes a
    /// `Response::Err` rather than a dropped stream: a handler that returned
    /// early without writing would leave a client waiting on a stream that will
    /// never speak, which is the failure this protocol exists to remove.
    pub async fn serve<P: Ports>(
        ports: &P,
        codec: &FrameCodec,
        request: Request,
        mut send: iroh::endpoint::SendStream,
        session: &Session,
    ) -> Result<(), TransportError> {
        // These are the calls that can take tens of seconds, so they prove they
        // are alive before the port is entered rather than after.
        if matches!(
            request,
            Request::StartConversation { .. } | Request::ResumeConversation { .. }
        ) {
            FrameIo::write(
                &mut send,
                codec,
                &Response::Progress(Progress {
                    stage: ProgressStage::StartingAgent,
                    detail: None,
                }),
            )
            .await?;
        }

        let result = Self::handle(ports, request, session).await;

        let response = match result {
            Ok(payload) => Response::Ok(payload),
            Err(error) => Response::Err(error),
        };

        Self::answer(&mut send, codec, response).await?;
        send.finish().ok();

        Ok(())
    }

    /// Writes the terminal response, or says why it could not be written.
    ///
    /// A payload too large for a control frame is the one failure that would
    /// otherwise leave no trace anywhere. `FrameTooLarge` is a `TransportError`,
    /// so propagating it ends the stream with no frame on it: the machine
    /// believes it answered, the client sees a clean close with nothing in it,
    /// and neither end has a line to read. Every paged response can reach this,
    /// because a page is bounded by a count of items and a frame is bounded by
    /// bytes, and the two are independent.
    ///
    /// The encode happens before any byte is written, so a refused frame has put
    /// nothing on the stream and the smaller answer below is the first thing the
    /// client sees.
    async fn answer(
        send: &mut iroh::endpoint::SendStream,
        codec: &FrameCodec,
        response: Response,
    ) -> Result<(), TransportError> {
        let Err(error) = FrameIo::write(send, codec, &response).await else {
            return Ok(());
        };

        let TransportError::FrameTooLarge { size, limit } = error else {
            return Err(error);
        };

        tracing::warn!(
            size,
            limit,
            "a response was too large for one frame; answering with its size instead"
        );

        FrameIo::write(
            send,
            codec,
            &Response::Err(WireError::TooLarge {
                size: size as u64,
                limit: limit as u64,
            }),
        )
        .await
    }

    async fn handle<P: Ports>(
        ports: &P,
        request: Request,
        session: &Session,
    ) -> Result<Payload, WireError> {
        match request {
            Request::Describe => Ok(Payload::Describe(ports.machine().describe().await)),

            Request::ListAgentProfiles => {
                Ok(Payload::AgentProfiles(ports.machine().agent_profiles().await))
            }

            Request::RecentCwds { limit } => {
                Ok(Payload::RecentCwds(ports.machine().recent_cwds(limit).await))
            }

            Request::ListConversations {
                filter,
                before,
                limit,
            } => Ok(Payload::Conversations(
                ports.conversations().list(filter, before, limit).await,
            )),

            Request::GetConversation { conversation } => Ok(Payload::Conversation(
                ports.conversations().get(&conversation).await?,
            )),

            Request::StartConversation {
                profile,
                cwd,
                prompt,
                attachments,
            } => Ok(Payload::Conversation(
                ports
                    .conversations()
                    .start(&profile, &cwd, prompt.as_deref(), &attachments)
                    .await?,
            )),

            Request::ResumeConversation { conversation, cwd } => Ok(Payload::Conversation(
                ports
                    .conversations()
                    .resume(&conversation, cwd.as_deref())
                    .await?,
            )),

            Request::PreviewConversation {
                profile,
                cwd,
                workspace,
            } => Ok(Payload::ConversationPreview(
                ports
                    .conversations()
                    .preview(&profile, &cwd, workspace.as_ref())
                    .await?,
            )),

            Request::SendPrompt {
                conversation,
                text,
                attachments,
            } => {
                ports
                    .conversations()
                    .send_prompt(&conversation, &text, &attachments)
                    .await?;

                Ok(Payload::Ack)
            }

            Request::Interrupt { conversation } => {
                ports.conversations().interrupt(&conversation).await?;

                Ok(Payload::Ack)
            }

            Request::StopConversation { conversation } => {
                ports.conversations().stop(&conversation).await?;

                Ok(Payload::Ack)
            }

            Request::AnswerQuestion {
                conversation,
                question,
                fingerprint,
                answers,
            } => {
                ports
                    .conversations()
                    .answer(&conversation, &question, &fingerprint, &answers)
                    .await?;

                Ok(Payload::Ack)
            }

            Request::Transcript {
                conversation,
                before,
                limit,
            } => Ok(Payload::Transcript(
                ports
                    .conversations()
                    .transcript(&conversation, before, limit)
                    .await?,
            )),

            Request::ListWorkspaces => {
                Ok(Payload::Workspaces(ports.machine().tree().await?.workspaces))
            }

            Request::ListTabs { workspace } => {
                Ok(Payload::Tabs(ports.terminals().list_tabs(&workspace).await?))
            }

            Request::ListPanes { tab } => {
                Ok(Payload::Panes(ports.terminals().list_panes(&tab).await?))
            }

            Request::OpenTerminal { workspace, cwd } => Ok(Payload::Pane(
                ports
                    .terminals()
                    .open(workspace.as_ref(), cwd.as_deref())
                    .await?,
            )),

            Request::SplitPane { pane, direction } => {
                Ok(Payload::Pane(ports.terminals().split(&pane, direction).await?))
            }

            Request::ClosePane { pane } => {
                ports.terminals().close(&pane).await?;

                Ok(Payload::Ack)
            }

            Request::TerminalScrollback {
                pane,
                before_line,
                limit,
            } => {
                let (styles, rows, next_before_line, has_earlier) = ports
                    .terminals()
                    .scrollback(&pane, before_line, limit)
                    .await?;

                Ok(Payload::Scrollback {
                    styles,
                    rows,
                    next_before_line,
                    has_earlier,
                })
            }

            Request::ListAssets {
                scope,
                before,
                limit,
            } => Ok(Payload::Assets(
                ports.assets().list(&scope, before, limit).await?,
            )),

            // Push delivery is a separate design. The frames exist so the wire
            // never has to change for it; a machine with no credential reports
            // the capability absent and these answer honestly rather than
            // pretending to have registered something.
            Request::RegisterPushToken { .. }
            | Request::RevokePushToken { .. }
            | Request::SetNotifyPolicy { .. } => Err(WireError::Unsupported {
                capability: tethera_common::protocol::capability::CapabilityId::from(
                    tethera_common::protocol::capability::PUSH_FCM,
                ),
            }),

            Request::RevokeThisDevice => {
                ports
                    .machine()
                    .revoke(session.device.id.as_str())
                    .await
                    .map_err(|_| WireError::NotFound {
                        kind: EntityKind::Device,
                    })?;

                Ok(Payload::Ack)
            }
        }
    }
}
