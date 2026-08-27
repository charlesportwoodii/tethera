use moka::sync::Cache;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tethera_common::structs::primitives::Sha256 as Digest256;

/// The digest of a file this machine has already read whole.
///
/// `FetchHead.sha256` covers the entire asset, so it is computed before the
/// first byte of the body goes out — and on a large file that is a wait a person
/// spends looking at a transfer that has not visibly started. A 403 MiB APK
/// takes about 685 ms warm and longer from cold disk.
///
/// **The cost lands hardest on exactly the transfer that can least afford it.**
/// A resumed download asks for the same digest again, so a phone that loses the
/// foreground three times pays three full reads of the file to move the last
/// few megabytes. Cached, a resume starts sending almost at once, which is what
/// makes resuming worth offering at all.
///
/// Bounded both ways, because a cache with neither is a leak with a lookup
/// method. Losing an entry costs one re-read.
pub struct AssetDigests {
    /// Keyed on the path together with the length and modification time behind
    /// it, never on the path alone. A digest is a claim about contents, and a
    /// file that changed under a remembered path would otherwise be served under
    /// the old file's digest — which every client checks its download against,
    /// so the transfer would arrive intact and be rejected as corrupt.
    known: Cache<(PathBuf, u64, u128), Digest256>,
}

impl AssetDigests {
    /// Enough for every file a long working session hands over, and each entry
    /// is a path and a hex string.
    const CAPACITY: u64 = 512;

    /// Matches how long `AssetIndex` keeps the path an id points at. A digest
    /// outliving the id that reaches it would never be asked for.
    const TTL: Duration = Duration::from_secs(6 * 60 * 60);

    pub fn new() -> Self {
        Self {
            known: Cache::builder()
                .max_capacity(Self::CAPACITY)
                .time_to_live(Self::TTL)
                .build(),
        }
    }

    pub fn new_shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// The digest of the whole file, hashed at most once per version of it.
    ///
    /// Blocking, and deliberately so: the caller runs it off the runtime's
    /// threads. Hashing half a gigabyte on a runtime thread stalls every other
    /// task sharing it, including the connection carrying this very transfer.
    pub fn of(&self, path: &Path) -> std::io::Result<Digest256> {
        let key = Self::fingerprint(path)?;

        if let Some(known) = self.known.get(&key) {
            return Ok(known);
        }

        let digest = Self::hash(path)?;

        self.known.insert(key, digest.clone());

        Ok(digest)
    }

    /// How many digests are held. A behavioural accessor: there is no other way
    /// to observe that a second read of the same file does not hash it again.
    pub fn len(&self) -> u64 {
        self.known.run_pending_tasks();

        self.known.entry_count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn fingerprint(path: &Path) -> std::io::Result<(PathBuf, u64, u128)> {
        let metadata = std::fs::metadata(path)?;

        // A clock that cannot answer is treated as a file that always looks
        // new, which costs a re-hash. The other way round would serve a stale
        // digest for a file that had changed.
        let modified = metadata
            .modified()
            .ok()
            .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_nanos())
            .unwrap_or(0);

        Ok((path.to_path_buf(), metadata.len(), modified))
    }

    fn hash(path: &Path) -> std::io::Result<Digest256> {
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();

        std::io::copy(&mut file, &mut hasher)?;

        Ok(Digest256(format!("{:x}", hasher.finalize())))
    }
}

impl Default for AssetDigests {
    fn default() -> Self {
        Self::new()
    }
}
