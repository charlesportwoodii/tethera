use crate::error::TransportError;

pub struct FrameCodec {
    max_frame_bytes: usize,
}

impl FrameCodec {
    /// The cap for a control stream: far above any control frame, far below what
    /// is worth allocating for a peer that has not yet been authorised.
    ///
    /// Bulk transfer does not use this codec. After a head frame the remainder
    /// of that stream is raw bytes to FIN, so a file is never constrained by it.
    pub const CONTROL_MAX_FRAME_BYTES: usize = 64 * 1024;

    /// The control cap, because that is the only framed use. A default chosen
    /// for bulk transfer would be a default for something that turned out not to
    /// be framed at all.
    pub const DEFAULT_MAX_FRAME_BYTES: usize = Self::CONTROL_MAX_FRAME_BYTES;

    pub const HEADER_BYTES: usize = 4;

    pub fn new(max_frame_bytes: usize) -> Self {
        Self { max_frame_bytes }
    }

    pub fn encode<T: serde::Serialize>(&self, frame: &T) -> Result<Vec<u8>, TransportError> {
        let body = postcard::to_stdvec(frame).map_err(TransportError::Encode)?;

        if body.len() > self.max_frame_bytes {
            return Err(TransportError::FrameTooLarge {
                size: body.len(),
                limit: self.max_frame_bytes,
            });
        }

        let mut out = Vec::with_capacity(Self::HEADER_BYTES + body.len());
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(&body);

        Ok(out)
    }

    // Checked before a body is read, so a hostile length header cannot make the
    // peer allocate for it. Zero is refused alongside an oversized value: there
    // is no empty control frame, so zero means a confused or hostile sender
    // either way.
    pub fn decode_length(&self, header: [u8; Self::HEADER_BYTES]) -> Result<usize, TransportError> {
        let len = u32::from_be_bytes(header) as usize;

        if len == 0 || len > self.max_frame_bytes {
            return Err(TransportError::FrameTooLarge {
                size: len,
                limit: self.max_frame_bytes,
            });
        }

        Ok(len)
    }

    pub fn decode_body<T: serde::de::DeserializeOwned>(
        &self,
        body: &[u8],
    ) -> Result<T, TransportError> {
        postcard::from_bytes(body).map_err(TransportError::Decode)
    }
}

impl Default for FrameCodec {
    fn default() -> Self {
        Self::new(Self::DEFAULT_MAX_FRAME_BYTES)
    }
}
