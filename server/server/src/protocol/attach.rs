use crate::protocol::ports::{Ports, TerminalPort, TerminalSession};
use tethera_common::protocol::terminal::{AttachSpec, CloseReason, TerminalFrame, TerminalInput};
use tethera_transport::error::TransportError;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

pub struct Attach;

impl Attach {
    /// Terminal frames down, input up, until either side stops.
    pub async fn serve<P: Ports>(
        ports: &P,
        codec: &FrameCodec,
        spec: AttachSpec,
        mut send: iroh::endpoint::SendStream,
        mut recv: iroh::endpoint::RecvStream,
    ) -> Result<(), TransportError> {
        let mut session = match ports.terminals().attach(&spec).await {
            Ok(session) => session,
            Err(_) => {
                FrameIo::write(
                    &mut send,
                    codec,
                    &TerminalFrame::Closed {
                        reason: CloseReason::PaneGone,
                    },
                )
                .await?;
                send.finish().ok();

                return Ok(());
            }
        };

        loop {
            tokio::select! {
                frame = session.next_frame() => {
                    match frame {
                        Some(frame) => FrameIo::write(&mut send, codec, &frame).await?,
                        None => {
                            send.finish().ok();

                            return Ok(());
                        }
                    }
                }

                input = FrameIo::read::<TerminalInput>(&mut recv, codec) => {
                    match input {
                        // The client finished its side. The pane keeps running -
                        // detaching is not closing - so this ends the stream
                        // and nothing else.
                        Ok(None) => {
                            send.finish().ok();

                            return Ok(());
                        }
                        Ok(Some(input)) => {
                            if session.send_input(input).await.is_err() {
                                FrameIo::write(
                                    &mut send,
                                    codec,
                                    &TerminalFrame::Closed { reason: CloseReason::PaneGone },
                                )
                                .await?;
                                send.finish().ok();

                                return Ok(());
                            }
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
        }
    }
}
