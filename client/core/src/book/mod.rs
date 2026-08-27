use crate::error::ClientError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tethera_common::structs::client::ServerEntry;
use tethera_common::structs::ids::ServerId;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BookFile {
    install_id: String,
    servers: Vec<ServerEntry>,
}

/// The machines this device remembers.
///
/// JSON rather than a database: a person owns a handful of machines, and a
/// mobile SQLite dependency would cost build weight for a file that never grows.
///
/// It holds no secret. The credential is the key in the platform keychain, and
/// nothing here is worth protecting beyond the ordinary per-app sandbox.
pub struct ServerBook {
    path: PathBuf,
    state: Mutex<BookFile>,
}

impl ServerBook {
    pub const FILE_NAME: &'static str = "servers.json";

    /// An absent file is the ordinary first launch. An unreadable one is a
    /// fault, and it is reported rather than replaced: starting from empty would
    /// show a paired person the first-launch screen and invite them to re-pair
    /// machines that already know them.
    pub fn open(path: PathBuf) -> Result<Self, ClientError> {
        let state = match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(book) => book,
                // The cached conversations are the only part of this file that
                // can go stale against a newer build, so they are the only part
                // dropped before giving up.
                Err(first) => Self::without_conversations(&bytes)
                    .ok_or_else(|| Self::fault(&path, first))?,
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BookFile {
                install_id: uuid::Uuid::new_v4().to_string(),
                servers: Vec::new(),
            },
            Err(error) => return Err(Self::fault(&path, error)),
        };

        Ok(Self {
            path,
            state: Mutex::new(state),
        })
    }

    /// The same file with every cached conversation removed.
    ///
    /// `ServerEntry` embeds `Conversation`, which is a **wire** type: it gains
    /// fields whenever the protocol does, and a file written by an older build
    /// then fails to parse. Refusing to open over that is the right instinct for
    /// the machines - they cannot be reconstructed and somebody would be shown
    /// the first-launch screen - but it is the wrong price for a cache. These
    /// conversations are a paint-before-you-dial convenience that the next sweep
    /// refills within a second.
    ///
    /// So the machines survive a protocol change and the cache does not, which
    /// is the correct way round. Without this, adding one field to a wire struct
    /// stops the app starting at all for anybody who has ever paired.
    fn without_conversations(bytes: &[u8]) -> Option<BookFile> {
        let mut raw: serde_json::Value = serde_json::from_slice(bytes).ok()?;

        for server in raw.get_mut("servers")?.as_array_mut()? {
            if let Some(object) = server.as_object_mut() {
                object.remove("conversations");
            }
        }

        serde_json::from_value(raw).ok()
    }

    pub fn install_id(&self) -> String {
        self.state.lock().expect("book").install_id.clone()
    }

    pub fn entries(&self) -> Vec<ServerEntry> {
        self.state.lock().expect("book").servers.clone()
    }

    /// Keyed on the server id, not the label and not the endpoint id.
    ///
    /// A machine that is re-paired replaces its row, and its dial details are
    /// refreshed from the newer offer, because a relay and a set of direct
    /// addresses both move.
    pub fn upsert(&self, entry: ServerEntry) -> Result<(), ClientError> {
        let mut state = self.state.lock().expect("book");

        match state
            .servers
            .iter()
            .position(|held| held.server.id == entry.server.id)
        {
            Some(index) => state.servers[index] = entry,
            None => state.servers.push(entry),
        }

        Self::persist(&self.path, &state)
    }

    /// `false` when the machine was not in the book, so a caller can tell a
    /// no-op from a removal.
    pub fn forget(&self, id: &ServerId) -> Result<bool, ClientError> {
        let mut state = self.state.lock().expect("book");
        let before = state.servers.len();

        state.servers.retain(|held| &held.server.id != id);

        let removed = state.servers.len() != before;
        Self::persist(&self.path, &state)?;

        Ok(removed)
    }

    fn persist(path: &Path, state: &BookFile) -> Result<(), ClientError> {
        let bytes = serde_json::to_vec_pretty(state).map_err(|error| Self::fault(path, error))?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| Self::fault(path, error))?;
        }

        std::fs::write(path, bytes).map_err(|error| Self::fault(path, error))
    }

    fn fault(path: &Path, error: impl std::fmt::Display) -> ClientError {
        ClientError::Book {
            path: path.display().to_string(),
            reason: error.to_string(),
        }
    }
}
