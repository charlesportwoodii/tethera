use crate::error::ClientError;
use iroh::endpoint::Connection;
use std::time::Duration;
use tethera_common::protocol::response::{Payload, Response};
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::Request;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

/// One request, one stream.
///
/// There is no request id and no correlation table: QUIC is the multiplexer, so
/// the stream *is* the correlation. A handler writes zero or more progress
/// frames and then exactly one terminal frame, which is what lets this read
/// until `is_terminal` rather than guess when a call has finished.
pub struct Rpc;

impl Rpc {
    /// How long one request may take before it is abandoned.
    ///
    /// Generous, because a machine reading a large transcript off disk over a
    /// relayed path is slow rather than broken. Present at all because the
    /// alternative is worse than any failure: without a deadline a peer that
    /// accepts a stream and never answers leaves the screen showing "loading"
    /// for as long as the app is open, with nothing logged and nothing to act
    /// on. A stall must become an error somebody can read.
    pub const DEADLINE: Duration = Duration::from_secs(20);

    pub async fn request(
        connection: &Connection,
        request: Request,
    ) -> Result<Payload, ClientError> {
        tokio::time::timeout(Self::DEADLINE, Self::carry(connection, request))
            .await
            .unwrap_or_else(|_| {
                Err(ClientError::Rpc(format!(
                    "the machine did not answer within {} seconds",
                    Self::DEADLINE.as_secs()
                )))
            })
    }

    async fn carry(
        connection: &Connection,
        request: Request,
    ) -> Result<Payload, ClientError> {
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        let codec = FrameCodec::default();

        FrameIo::write(&mut send, &codec, &StreamOpen::Rpc(request))
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        // The request is complete, so the write half closes. A handler that
        // reads to FIN would otherwise wait for bytes that are never coming.
        send.finish().ok();

        loop {
            let frame: Option<Response> = FrameIo::read(&mut recv, &codec)
                .await
                .map_err(|error| ClientError::Rpc(error.to_string()))?;

            // The stream ended without a terminal frame, which is a peer that
            // went away mid-call rather than a call that failed.
            let Some(frame) = frame else {
                return Err(ClientError::Rpc(
                    "the machine closed the stream without answering".to_string(),
                ));
            };

            match frame {
                Response::Ok(payload) => return Ok(payload),
                Response::Err(error) => return Err(ClientError::Wire(error)),
                // Progress carries no result. Discarded rather than surfaced,
                // because nothing on these screens draws a progress bar yet and
                // a caller that ignored it would still have to loop.
                Response::Progress(_) => continue,
            }
        }
    }
}
