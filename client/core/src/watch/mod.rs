use crate::error::ClientError;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::watch::{WatchControl, WatchEvent, WatchOpen, WatchSpec};
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

/// A live subscription: one snapshot, then events until somebody stops.
///
/// The snapshot is not optional and not a separate request. Every subscription
/// is backed by a source the machine can re-read, so a reconnecting client
/// re-subscribes and is handed a fresh snapshot rather than replaying a log that
/// does not exist. That is why `open` returns the snapshot with the watch: a
/// caller cannot hold one without the other and cannot render events against a
/// state it never received.
pub struct Watch {
    send: SendStream,
    recv: RecvStream,
    codec: FrameCodec,
}

impl Watch {
    pub async fn open(
        connection: &Connection,
        spec: WatchSpec,
    ) -> Result<(WatchOpen, Self), ClientError> {
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        let codec = FrameCodec::default();

        FrameIo::write(&mut send, &codec, &StreamOpen::Watch(spec))
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        // The write half stays open, unlike an RPC. A watch is closed by sending
        // `WatchControl::Close`, and finishing here would take that channel with
        // it.
        let opened: Option<WatchOpen> = FrameIo::read(&mut recv, &codec)
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        let opened = opened.ok_or_else(|| {
            ClientError::Rpc("the machine closed the watch without a snapshot".to_string())
        })?;

        Ok((opened, Self { send, recv, codec }))
    }

    /// The next event, or `None` once the machine has finished the stream.
    ///
    /// `None` is an ending rather than a failure: the conversation ended, or the
    /// machine is shutting down. A caller that treated it as an error would
    /// report a fault for an agent that simply stopped.
    pub async fn next(&mut self) -> Result<Option<WatchEvent>, ClientError> {
        FrameIo::read(&mut self.recv, &self.codec)
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))
    }

    /// Ends the subscription in an orderly way.
    ///
    /// Dropping the streams would also end it, by reset, which the machine sees
    /// as a peer that went away. Saying so first lets it distinguish a screen
    /// somebody closed from a phone that lost its route.
    pub async fn close(mut self) {
        FrameIo::write(&mut self.send, &self.codec, &WatchControl::Close)
            .await
            .ok();

        self.send.finish().ok();
    }
}
