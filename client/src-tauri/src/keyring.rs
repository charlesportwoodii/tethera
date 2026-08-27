use tauri::AppHandle;
use tauri_plugin_keyring::{CredentialType, CredentialValue, Error as KeyringError, KeyringExt};
use tethera_client_core::identity::SecretStore;
use tethera_client_core::ClientError;

/// This device's key, in the platform's own credential store.
///
/// Apple Keychain, Windows Credential Manager, Android Keystore, or the D-Bus
/// Secret Service, whichever the platform provides.
///
/// Read and written only from Rust. The JavaScript keyring permissions are
/// deliberately not granted in `capabilities/default.json`, so a compromised
/// webview cannot reach the credential that identifies this device to every
/// machine it is paired with.
pub struct KeyringStore {
    app: AppHandle,
}

impl KeyringStore {
    pub const SERVICE: &'static str = "com.alaydriem.tethera";

    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub fn initialise(&self) -> Result<(), ClientError> {
        self.app
            .keyring()
            .initialize_service(Self::SERVICE.to_string())
            .map_err(|error| ClientError::SecretStore(error.to_string()))
    }
}

impl SecretStore for KeyringStore {
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, ClientError> {
        // A key is binary. Stored as a password it would take a UTF-8 round
        // trip, which is lossy for most 32-byte values.
        match self.app.keyring().get(name, CredentialType::Secret) {
            Ok(CredentialValue::Secret(bytes)) => Ok(Some(bytes)),
            Ok(CredentialValue::Password(_)) => Err(ClientError::SecretStore(
                "the stored identity is a password, not a secret".to_string(),
            )),
            // The only error that means "nothing stored yet". Every other
            // failure propagates: a keychain that cannot be read, reported as an
            // absent key, would mint a second identity and every paired machine
            // would stop recognising this device.
            Err(KeyringError::EntryNotFound) => Ok(None),
            Err(error) => Err(ClientError::SecretStore(error.to_string())),
        }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), ClientError> {
        self.app
            .keyring()
            .set(
                name,
                CredentialType::Secret,
                CredentialValue::Secret(bytes.to_vec()),
            )
            .map_err(|error| ClientError::SecretStore(error.to_string()))
    }
}
