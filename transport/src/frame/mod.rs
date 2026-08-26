mod codec;

pub use codec::FrameCodec;

use serde::{Deserialize, Serialize};

// One variant, replaced by the protocol agents. The codec, its size limit and
// its tests do not depend on which variants exist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Frame {
    Placeholder {
        #[serde(with = "serde_bytes")]
        payload: Vec<u8>,
    },
}
