use crate::error::ClientError;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::transfer::{PutReady, PutResult, PutSpec};
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

/// An upload: a ready frame, then raw bytes to FIN, then a result frame.
///
/// The ready frame is not a formality. Only the machine knows how much of a
/// previous attempt reached its disk, so a resumed upload seeks to the offset it
/// is given rather than to the one it proposed. A client that trusted its own
/// proposal would write the same bytes twice, or skip a gap, and either way the
/// file is corrupt with no error anywhere.
pub struct Put {
    send: SendStream,
    recv: RecvStream,
    codec: FrameCodec,
    /// Bytes still owed, counted down from the length that was declared.
    remaining: u64,
}

impl Put {
    /// Opens the upload and answers where the machine wants the body to start.
    ///
    /// The caller seeks its own reader to `PutReady::offset` before the first
    /// `write`. Nothing here can do that for it: this type never sees the file,
    /// only the bytes it is handed.
    pub async fn open(
        connection: &Connection,
        spec: PutSpec,
    ) -> Result<(PutReady, Self), ClientError> {
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        let codec = FrameCodec::default();
        let declared = spec.len;

        FrameIo::write(&mut send, &codec, &StreamOpen::Put(spec))
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        let ready: Option<PutReady> = FrameIo::read(&mut recv, &codec)
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        let ready = ready.ok_or_else(|| {
            ClientError::Rpc("the machine refused the upload without saying where to start".to_string())
        })?;

        let remaining = declared.saturating_sub(ready.offset);

        Ok((
            ready,
            Self {
                send,
                recv,
                codec,
                remaining,
            },
        ))
    }

    /// Hands over the next piece of the body.
    ///
    /// Refuses to send more than was declared. The length is what the machine
    /// allocated and accounted against a quota, so overrunning it is the client
    /// breaking its own contract, and it is caught here rather than becoming a
    /// reset the far side has to interpret.
    pub async fn write(&mut self, chunk: &[u8]) -> Result<(), ClientError> {
        if chunk.is_empty() {
            return Ok(());
        }

        let size = chunk.len() as u64;

        if size > self.remaining {
            return Err(ClientError::Rpc(format!(
                "the upload tried to send {size} more bytes with only {} left to send",
                self.remaining
            )));
        }

        self.send
            .write_all(chunk)
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        self.remaining -= size;

        Ok(())
    }

    /// Closes the body and reads the id the upload became.
    ///
    /// A short upload is refused here rather than finished. Finishing the stream
    /// early tells the machine the file is complete, and it would store a
    /// truncated file under an id a prompt then references as though it were
    /// whole.
    pub async fn finish(mut self) -> Result<PutResult, ClientError> {
        if self.remaining > 0 {
            return Err(ClientError::Rpc(format!(
                "the upload stopped with {} bytes never sent",
                self.remaining
            )));
        }

        self.send
            .finish()
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        let result: Option<PutResult> = FrameIo::read(&mut self.recv, &self.codec)
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        result.ok_or_else(|| {
            ClientError::Rpc(
                "the machine took the whole file and never said what it became".to_string(),
            )
        })
    }
}
