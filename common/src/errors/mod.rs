#[derive(Debug, thiserror::Error)]
pub enum TetheraError {
    #[error("device not found: {0}")]
    DeviceNotFound(String),
    #[error("unknown device state: {0}")]
    UnknownDeviceState(String),
    #[error("pairing code expired")]
    PairingCodeExpired,
    #[error("pairing code has already been used")]
    PairingCodeAlreadyUsed,
    #[error("pairing code does not match")]
    PairingCodeMismatch,
    #[error("stored pairing code is not a 32 byte hex digest")]
    InvalidPairingCodeDigest,
    #[error("invalid pairing uri: {0}")]
    InvalidPairingUri(String),
}
