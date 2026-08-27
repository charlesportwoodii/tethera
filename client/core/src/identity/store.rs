use crate::error::ClientError;

/// Where this device's secret key is kept.
///
/// A trait rather than a direct call to the platform keychain, so the
/// mint-once decision below it can be tested without a credential store.
/// Synchronous, because every implementation of it is.
pub trait SecretStore {
    /// `None` means nothing has been stored yet. Every other failure is an
    /// error: a store that cannot be read is not the same as an empty one, and
    /// conflating them mints a second identity.
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, ClientError>;

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), ClientError>;
}

#[cfg(any(test, feature = "testing"))]
pub struct MemoryStore {
    entries: std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>,
}

#[cfg(any(test, feature = "testing"))]
impl MemoryStore {
    pub fn new() -> Self {
        Self {
            entries: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Puts a value in place without going through `write`, so a test can set up
    /// a store that already holds something unreadable.
    pub fn seed(&self, name: &str, bytes: &[u8]) {
        self.entries
            .lock()
            .expect("memory store")
            .insert(name.to_string(), bytes.to_vec());
    }
}

#[cfg(any(test, feature = "testing"))]
impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "testing"))]
impl SecretStore for MemoryStore {
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, ClientError> {
        Ok(self.entries.lock().expect("memory store").get(name).cloned())
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), ClientError> {
        self.entries
            .lock()
            .expect("memory store")
            .insert(name.to_string(), bytes.to_vec());

        Ok(())
    }
}
