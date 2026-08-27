use crate::error::ClientError;
use iroh::endpoint::SendStream;
use tethera_common::protocol::terminal::TerminalInput;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

/// The writing half of an attach: keys and text going back to the pane.
pub struct AttachInput {
    send: SendStream,
    codec: FrameCodec,
}

impl AttachInput {
    pub fn new(send: SendStream) -> Self {
        Self {
            send,
            codec: FrameCodec::default(),
        }
    }

    /// Sends one keypress or one piece of text.
    ///
    /// Intent, never bytes. The machine encodes, which is why a phone's control
    /// bar can offer CTRL+C without knowing a terminal encoding, and why there
    /// is no raw variant for anything here to smuggle an escape sequence
    /// through.
    pub async fn send(&mut self, input: TerminalInput) -> Result<(), ClientError> {
        FrameIo::write(&mut self.send, &self.codec, &input)
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))
    }

    /// Ends the attach in an orderly way. The pane keeps running.
    ///
    /// Dropping the stream would also end it, by reset, which the machine reads
    /// as a peer that went away. Finishing says a screen was closed, which is a
    /// different fact and the true one - and the machine already answers it by
    /// ending the stream and leaving the pane alone.
    pub fn detach(mut self) {
        self.send.finish().ok();
    }
}
