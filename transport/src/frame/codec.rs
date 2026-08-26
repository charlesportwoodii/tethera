use crate::error::TransportError;
use crate::frame::Frame;

pub struct FrameCodec {
    max_frame_bytes: usize,
}

impl FrameCodec {
    pub const DEFAULT_MAX_FRAME_BYTES: usize = 1024 * 1024;
    pub const HEADER_BYTES: usize = 4;

    pub fn new(max_frame_bytes: usize) -> Self {
        Self { max_frame_bytes }
    }

    pub fn encode(&self, frame: &Frame) -> Result<Vec<u8>, TransportError> {
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

    // Checked before a body is read, so a hostile length header cannot make
    // the peer allocate for it.
    pub fn decode_length(&self, header: [u8; Self::HEADER_BYTES]) -> Result<usize, TransportError> {
        let len = u32::from_be_bytes(header) as usize;

        if len > self.max_frame_bytes {
            return Err(TransportError::FrameTooLarge {
                size: len,
                limit: self.max_frame_bytes,
            });
        }

        Ok(len)
    }

    pub fn decode_body(&self, body: &[u8]) -> Result<Frame, TransportError> {
        postcard::from_bytes(body).map_err(TransportError::Decode)
    }
}

impl Default for FrameCodec {
    fn default() -> Self {
        Self::new(Self::DEFAULT_MAX_FRAME_BYTES)
    }
}
