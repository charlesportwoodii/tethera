use anyhow::Context;
use iroh::SecretKey;
use std::path::Path;

pub struct Identity;

impl Identity {
    pub fn load_or_create(path: &Path) -> anyhow::Result<SecretKey> {
        if path.exists() {
            return Self::load(path);
        }

        Self::create(path)
    }

    // A read-only query must not mint a cryptographic identity as a side
    // effect. Absence is reported, not repaired.
    pub fn load_or_report_absent(path: &Path) -> anyhow::Result<Option<SecretKey>> {
        if !path.exists() {
            return Ok(None);
        }

        Self::load(path).map(Some)
    }

    fn load(path: &Path) -> anyhow::Result<SecretKey> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("reading identity from {}", path.display()))?;

        let bytes: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .context("identity key is not 32 bytes")?;

        Ok(SecretKey::from_bytes(&bytes))
    }

    fn create(path: &Path) -> anyhow::Result<SecretKey> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let key = SecretKey::generate();
        std::fs::write(path, key.to_bytes())?;
        Self::restrict_permissions(path)?;

        Ok(key)
    }

    #[cfg(unix)]
    fn restrict_permissions(path: &Path) -> anyhow::Result<()> {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;

        Ok(())
    }

    // Windows inherits the parent ACL, and the data dir is already per-user
    // under %LOCALAPPDATA%.
    #[cfg(not(unix))]
    fn restrict_permissions(_path: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}
