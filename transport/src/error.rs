#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("frame of {size} bytes exceeds the {limit} byte limit")]
    FrameTooLarge { size: usize, limit: usize },
    #[error("frame encoding failed: {0}")]
    Encode(postcard::Error),
    #[error("frame decoding failed: {0}")]
    Decode(postcard::Error),
    #[error("endpoint bind failed: {0}")]
    Bind(String),
    #[error("connection failed: {0}")]
    Connection(String),
    // Distinct from Connection because it is permanent. An accept loop that
    // treats it as transient spins at 100% of a core for the life of the
    // process.
    #[error("endpoint closed")]
    EndpointClosed,
}
