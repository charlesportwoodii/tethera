use crate::errors::TetheraError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingCode {
    digest: [u8; 32],
}

impl PairingCode {
    pub const DIGEST_BYTES: usize = 32;

    pub fn from_plaintext(code: &str) -> Self {
        Self {
            digest: Self::hash(code),
        }
    }

    pub fn from_digest(digest: [u8; Self::DIGEST_BYTES]) -> Self {
        Self { digest }
    }

    // Storage holds the hex digest, not the type's serialized form: a column
    // that carries a serde envelope is unreadable by anything but this exact
    // struct definition, and the digest itself is the whole of the value.
    pub fn from_hex(hex: &str) -> Result<Self, TetheraError> {
        // from_str_radix would accept a leading sign, so the alphabet is
        // checked here rather than left to the per-byte parse.
        if hex.len() != Self::DIGEST_BYTES * 2 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(TetheraError::InvalidPairingCodeDigest);
        }

        let mut digest = [0u8; Self::DIGEST_BYTES];

        for (index, byte) in digest.iter_mut().enumerate() {
            let pair = hex
                .get(index * 2..index * 2 + 2)
                .ok_or(TetheraError::InvalidPairingCodeDigest)?;

            *byte = u8::from_str_radix(pair, 16)
                .map_err(|_| TetheraError::InvalidPairingCodeDigest)?;
        }

        Ok(Self::from_digest(digest))
    }

    pub fn to_hex(&self) -> String {
        self.digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    pub fn hash(code: &str) -> [u8; Self::DIGEST_BYTES] {
        let mut hasher = Sha256::new();
        hasher.update(code.as_bytes());
        hasher.finalize().into()
    }

    // Constant time. The code is a secret, and a timing-variable compare on a
    // secret is a defect regardless of how remote the attack is.
    pub fn verify(&self, candidate: &str) -> bool {
        self.digest.ct_eq(&Self::hash(candidate)).into()
    }
}
