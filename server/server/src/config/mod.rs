use directories::ProjectDirs;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone)]
pub struct ApplicationConfig {
    pub data_dir: PathBuf,
    pub relay_url: Option<String>,
    pub relay_token: Option<String>,
}

impl ApplicationConfig {
    pub const QUALIFIER: &'static str = "com";
    pub const ORGANIZATION: &'static str = "alaydriem";
    pub const APPLICATION: &'static str = "tethera";

    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            relay_url: None,
            relay_token: None,
        }
    }

    pub fn default_data_dir() -> PathBuf {
        ProjectDirs::from(Self::QUALIFIER, Self::ORGANIZATION, Self::APPLICATION)
            .map(|dirs| dirs.data_local_dir().to_path_buf())
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(".tethera")
            })
    }

    pub fn identity_path(&self) -> PathBuf {
        self.data_dir.join("identity.key")
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("tethera.sqlite3")
    }

    pub fn pid_path(&self) -> PathBuf {
        self.data_dir.join("tethera.pid")
    }

    // mode=rwc so a first run creates the file rather than failing on an
    // absent one. Zero config to start is a requirement, not a convenience.
    pub fn database_url(&self) -> String {
        format!(
            "sqlite://{}?mode=rwc",
            self.database_path().to_string_lossy().replace('\\', "/")
        )
    }

    pub fn ensure_data_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)
    }
}

// Hand written so a `tracing::debug!(?config)` cannot dump a live credential
// into the log.
impl fmt::Debug for ApplicationConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationConfig")
            .field("data_dir", &self.data_dir)
            .field("relay_url", &self.relay_url)
            .field("relay_token", &self.relay_token.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self::with_data_dir(Self::default_data_dir())
    }
}
