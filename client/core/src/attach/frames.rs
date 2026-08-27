use crate::error::ClientError;
use iroh::endpoint::RecvStream;
use tethera_common::protocol::terminal::TerminalFrame;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

/// The reading half of an attach: a pane's screen, frame by frame.
pub struct AttachFrames {
    recv: RecvStream,
    codec: FrameCodec,
}

impl AttachFrames {
    pub fn new(recv: RecvStream) -> Self {
        Self {
            recv,
            codec: FrameCodec::default(),
        }
    }

    /// The next frame, or `None` once the machine has finished the stream.
    ///
    /// `None` is an ending rather than a failure: the pane closed, the machine
    /// is shutting down, or it accepted a detach. A caller that treated it as an
    /// error would report a fault for a pane that simply stopped - and the pane
    /// itself says which of those it was, in a `Closed` frame, before the stream
    /// ends.
    pub async fn next(&mut self) -> Result<Option<TerminalFrame>, ClientError> {
        FrameIo::read(&mut self.recv, &self.codec)
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))
    }
}
