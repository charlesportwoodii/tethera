use sha2::{Digest as _, Sha256 as Hasher};
use tethera_common::structs::primitives::Sha256;

/// SHA-256 over bytes that arrive in pieces.
///
/// Both transfer directions need this and neither can hold a whole file to hash
/// it: an upload is read from disk in chunks, and a download is verified against
/// what was written rather than against what crossed the wire. A resumed
/// download never sees its own first half, so hashing the stream would check
/// nothing.
pub struct Digest {
    hasher: Hasher,
}

impl Digest {
    pub fn new() -> Self {
        Self {
            hasher: Hasher::new(),
        }
    }

    pub fn eat(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
    }

    pub fn finish(self) -> Sha256 {
        let out = self.hasher.finalize();
        let mut hex = String::with_capacity(out.len() * 2);

        for byte in out {
            hex.push_str(&format!("{byte:02x}"));
        }

        Sha256(hex)
    }

    pub fn of(bytes: &[u8]) -> Sha256 {
        let mut digest = Self::new();
        digest.eat(bytes);

        digest.finish()
    }
}

impl Default for Digest {
    fn default() -> Self {
        Self::new()
    }
}
