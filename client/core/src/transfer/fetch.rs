use crate::error::ClientError;
use iroh::endpoint::{Connection, RecvStream};
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::transfer::{FetchHead, FetchSpec};
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

/// A download: one head frame, then raw bytes to FIN.
///
/// The body is deliberately not framed. A file is bulk, and framing it would
/// pay a header per chunk and put every transfer under the control frame cap,
/// which is 64 KiB - smaller than most of what anybody would send.
///
/// Chunks are handed out rather than accumulated. This runs on a phone, and a
/// carrier that returned a whole file would decide, on behalf of every caller,
/// that the file fits in memory.
pub struct Fetch {
    recv: RecvStream,
    /// Bytes of the body still owed, counted down from what the head declared.
    remaining: u64,
}

impl Fetch {
    /// How much is read in one call.
    ///
    /// Bounded so a declared length cannot make this allocate for it. The head
    /// is the peer's claim about a file, and a claim of forty gigabytes must
    /// cost forty gigabytes of transfer to make good on, not one allocation.
    pub const CHUNK: usize = 64 * 1024;

    pub async fn open(
        connection: &Connection,
        spec: FetchSpec,
    ) -> Result<(FetchHead, Self), ClientError> {
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        let codec = FrameCodec::default();

        FrameIo::write(&mut send, &codec, &StreamOpen::Fetch(spec))
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        // Nothing more is ever sent on a download, so the write half closes.
        // A machine that reads to FIN would otherwise wait for bytes that are
        // not coming.
        send.finish().ok();

        let head: Option<FetchHead> = FrameIo::read(&mut recv, &codec)
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        let head = head.ok_or_else(|| {
            ClientError::Rpc("the machine closed the download without a head".to_string())
        })?;

        // The machine says where it actually starts, which is believed over the
        // offset that was asked for. Subtracting the wrong one here would set
        // this counter above what is really coming and every transfer would end
        // as a truncation.
        let remaining = head.len.saturating_sub(head.offset);

        Ok((head, Self { recv, remaining }))
    }

    /// The next piece of the body, or `None` at the end of a complete transfer.
    ///
    /// A stream that ends with bytes still owed is an error rather than an
    /// ending. That is the one failure this type exists to catch: a truncated
    /// download is a shorter file that reads perfectly well, and without this it
    /// would reach the disk as a silent corruption rather than as a message.
    pub async fn next(&mut self) -> Result<Option<Vec<u8>>, ClientError> {
        if self.remaining == 0 {
            return Ok(None);
        }

        let want = Self::CHUNK.min(self.remaining as usize);
        let mut chunk = vec![0u8; want];

        match self.recv.read_exact(&mut chunk).await {
            Ok(()) => {}
            Err(error) => {
                return Err(ClientError::Rpc(format!(
                    "the download ended {} bytes early: {}",
                    self.remaining,
                    Self::because(&error)
                )))
            }
        }

        self.remaining -= want as u64;

        Ok(Some(chunk))
    }

    /// A failure with every layer under it named.
    ///
    /// `ReadError::ConnectionLost` is declared as the bare words "connection
    /// lost" with no field in its message, so the `ConnectionError` saying
    /// *why* - timed out, closed by the peer, a transport code - is reachable
    /// only through `source`. Printing the outer error alone is what made a
    /// real transfer failure impossible to diagnose from its own message.
    fn because(error: &dyn std::error::Error) -> String {
        let mut said = error.to_string();
        let mut under = error.source();

        while let Some(cause) = under {
            let layer = cause.to_string();

            // A layer that only restates the one above it adds nothing but
            // length, and these chains do repeat themselves.
            if !said.ends_with(&layer) {
                said.push_str(": ");
                said.push_str(&layer);
            }

            under = cause.source();
        }

        said
    }

    /// Bytes of the body still owed.
    ///
    /// A behavioural accessor rather than a public field, because the number
    /// anything outside actually wants is `head.len - remaining` - what has
    /// arrived, counting the half a resumed transfer never sees. Exposing the
    /// count on its own invites a bar drawn from what crossed the wire, which
    /// finishes early on every resume.
    pub fn remaining(&self) -> u64 {
        self.remaining
    }

    /// Whether every byte the head promised has been handed out.
    ///
    /// Read by a caller that stops early of its own accord, which `next`
    /// returning `None` cannot distinguish from a transfer that finished.
    pub fn complete(&self) -> bool {
        self.remaining == 0
    }
}
