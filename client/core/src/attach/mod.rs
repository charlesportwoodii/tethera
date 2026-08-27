mod frames;
mod input;

pub use frames::AttachFrames;
pub use input::AttachInput;

use crate::error::ClientError;
use iroh::endpoint::Connection;
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::terminal::AttachSpec;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

/// A live pane: frames down, keys up, until either side stops.
pub struct Attach;

impl Attach {
    /// Opens the stream and hands back its two halves.
    ///
    /// Two halves rather than one struct, because the frames are read by a pump
    /// task while input is written by whatever the person is doing, at the same
    /// time. Holding both behind one lock would queue a keystroke behind a
    /// frame read, on the screen where that is most obvious.
    ///
    /// Unlike an RPC, the write half is not finished here: input flows on it for
    /// the life of the screen, and finishing it is how a detach is said.
    pub async fn open(
        connection: &Connection,
        spec: AttachSpec,
    ) -> Result<(AttachFrames, AttachInput), ClientError> {
        let (mut send, recv) = connection
            .open_bi()
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        let codec = FrameCodec::default();

        FrameIo::write(&mut send, &codec, &StreamOpen::Attach(spec))
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        Ok((AttachFrames::new(recv), AttachInput::new(send)))
    }
}
