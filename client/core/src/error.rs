/// What can go wrong inside this crate.
///
/// Kept here rather than in `common/` because nothing outside this crate matches
/// on it structurally, which is the same reason `TransportError` lives in
/// `transport/`.
///
/// Note what is deliberately absent: a wrong pairing code, a closed window, a
/// revoked device. Those are expected outcomes of a working exchange, so they
/// are variants of `BeginOutcome` and `PairOutcome` and a screen can act on
/// them. Only a genuine fault reaches this type.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error(
        "the stored identity is {found} bytes; a key is {expected}. refusing to mint a new one, \
         because that would change this device's identity and every paired machine would stop \
         recognising it"
    )]
    CorruptIdentity { found: usize, expected: usize },

    #[error("the secret store failed: {0}")]
    SecretStore(String),

    #[error(
        "the server list at {path} could not be read: {reason}. it has not been replaced; \
         move it aside to start over"
    )]
    Book { path: String, reason: String },

    #[error(
        "the part-finished download at {path} could not be used: {reason}. delete it to start \
         that download over"
    )]
    Partial { path: String, reason: String },

    #[error("{value} is not an endpoint id: {reason}")]
    BadEndpointId { value: String, reason: String },

    #[error("{value} is not a usable relay url: {reason}")]
    BadRelayUrl { value: String, reason: String },

    #[error("could not bind this device's endpoint: {0}")]
    Bind(String),

    #[error("could not reach the machine: {0}")]
    Dial(String),

    #[error("that is not a tethera pairing link: {0}")]
    BadOffer(String),

    #[error(
        "that pairing link names no endpoint to reach. the machine may not have started its \
         server yet; run `tethera server start` on it"
    )]
    OfferHasNoEndpoint,

    #[error("the request could not be carried: {0}")]
    Rpc(String),

    /// The machine understood the request and refused it.
    ///
    /// Kept as the wire type rather than flattened to a string, so a caller can
    /// still branch on why it was refused.
    #[error("the machine refused the request: {0:?}")]
    Wire(tethera_common::protocol::WireError),
}
