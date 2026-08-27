use moka::sync::Cache;
use std::path::PathBuf;
use std::time::Duration;
use tethera_common::structs::ids::AssetId;

/// Where a file an agent handed over actually lives.
///
/// The id a client holds is a one-way hash of a canonical path — deliberately,
/// so nobody can bend one id into a different file — which means the way back
/// has to be remembered rather than computed. This is that memory.
///
/// **Populated by reading transcripts, never by walking a directory.** The paths
/// exist only in the records, so the same read that produces a `Part::File` card
/// is the read that registers where it points. A card is therefore fetchable the
/// moment it arrives, with no separate listing first.
///
/// Bounded both ways, because a cache with neither is a leak with a lookup
/// method. Losing an entry costs a re-read of the conversation that produced it,
/// which is what a client tapping the card does anyway.
pub struct AssetIndex {
    known: Cache<AssetId, PathBuf>,
}

impl AssetIndex {
    /// Enough for every file in a long working session, and small beside the
    /// transcripts it is derived from.
    const CAPACITY: u64 = 2_048;

    /// Long enough that a person scrolling back to a card from this morning can
    /// still open it, short enough that a machine left running for weeks does
    /// not hold paths to files that have since moved.
    const TTL: Duration = Duration::from_secs(6 * 60 * 60);

    pub fn new() -> Self {
        Self {
            known: Cache::builder()
                .max_capacity(Self::CAPACITY)
                .time_to_live(Self::TTL)
                .build(),
        }
    }

    pub fn new_shared() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self::new())
    }

    /// Records where an id points.
    ///
    /// Takes the canonical spelling the id was minted from, not the path as it
    /// was written down: on Windows `canonicalize` returns the extended-length
    /// form, so the same file hashes two different ways depending on which
    /// spelling the caller started from, and an entry filed under the other one
    /// opens onto nothing.
    pub fn register(&self, asset: AssetId, canonical: &str) {
        // An empty path is not a location, and every empty path hashes to the
        // same id — so one record with a blank field would claim an id and hand
        // back nothing to whoever asked for it. Refused at the door rather than
        // discovered as a blank line in somebody's prompt.
        if canonical.is_empty() {
            return;
        }

        self.known.insert(asset, PathBuf::from(canonical));
    }

    /// Where an id points, if this machine has read a record that said.
    pub fn locate(&self, asset: &AssetId) -> Option<PathBuf> {
        self.known
            .get(asset)
            .filter(|path| !path.as_os_str().is_empty())
    }

    /// How many paths are held. A behavioural accessor: there is no other way to
    /// observe that reading a transcript is what fills this.
    pub fn len(&self) -> u64 {
        self.known.run_pending_tasks();

        self.known.entry_count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for AssetIndex {
    fn default() -> Self {
        Self::new()
    }
}
