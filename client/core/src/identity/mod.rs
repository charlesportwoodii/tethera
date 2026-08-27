mod store;

pub use store::SecretStore;

#[cfg(any(test, feature = "testing"))]
pub use store::MemoryStore;

use crate::error::ClientError;
use iroh::SecretKey;

/// This device's own key.
///
/// The endpoint id derived from it is what every paired machine holds in its
/// allow-list, so this key *is* the credential. There are no bearer tokens
/// anywhere in this protocol; QUIC TLS proves the endpoint id, and the endpoint
/// id is the identity.
///
/// One key serves every machine. Iroh binds a UDP socket per endpoint, so a key
/// per paired machine would mean a socket and a set of NAT keepalives per
/// machine on a phone.
pub struct Identity;

impl Identity {
    pub const KEY_NAME: &'static str = "identity";
    pub const KEY_BYTES: usize = 32;

    pub fn load_or_create<S: SecretStore>(store: &S) -> Result<SecretKey, ClientError> {
        if let Some(bytes) = store.read(Self::KEY_NAME)? {
            return Self::parse(&bytes);
        }

        let key = SecretKey::generate();
        store.write(Self::KEY_NAME, &key.to_bytes())?;

        Ok(key)
    }

    // An error, never a fresh key. Minting over an unreadable value changes this
    // device's identity, and every paired machine then refuses it as unenrolled
    // - which reads on screen as "not paired" and sends somebody to re-pair
    // every machine they own.
    fn parse(bytes: &[u8]) -> Result<SecretKey, ClientError> {
        let sized: [u8; Self::KEY_BYTES] =
            bytes
                .try_into()
                .map_err(|_| ClientError::CorruptIdentity {
                    found: bytes.len(),
                    expected: Self::KEY_BYTES,
                })?;

        Ok(SecretKey::from_bytes(&sized))
    }
}
