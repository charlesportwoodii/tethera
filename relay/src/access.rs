use iroh_relay::server::{Access, AccessControl, ClientRequest};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

#[derive(Debug)]
pub struct SharedSecretAccess {
    digest: [u8; 32],
}

impl SharedSecretAccess {
    pub fn new(secret: String) -> Self {
        Self {
            digest: Self::hash(&secret),
        }
    }

    fn hash(value: &str) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        hasher.finalize().into()
    }

    // Hashed then compared in constant time. Comparing the raw strings would
    // leak the shared secret's prefix through timing, and hashing first also
    // makes the comparison length-independent.
    pub fn admits(&self, token: Option<&str>) -> bool {
        match token {
            Some(token) => self.digest.ct_eq(&Self::hash(token)).into(),
            None => false,
        }
    }
}

impl AccessControl for SharedSecretAccess {
    async fn on_connect(&self, request: &ClientRequest) -> Access {
        if self.admits(request.auth_token().as_deref()) {
            return Access::Allow;
        }

        Access::Deny {
            reason: Some("invalid relay token".to_string()),
        }
    }
}
