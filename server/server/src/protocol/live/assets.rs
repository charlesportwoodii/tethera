use crate::config::ApplicationConfig;
use crate::paths::PathName;
use crate::protocol::live::{AssetDigests, LiveConversations};
use crate::protocol::ports::{AssetPort, ConversationPort};
use crate::transcript::{AssetIndex, AssetNaming};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tethera_common::protocol::capability::{self, CapabilityId, CapabilitySet};
use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::protocol::response::Page;
use tethera_common::protocol::transfer::{FetchHead, PutReady, PutResult, PutSpec};
use tethera_common::structs::asset::{AssetCard, AssetScope};
use tethera_common::structs::ids::AssetId;
use tethera_common::structs::primitives::{Cursor, Sha256 as Digest256};
use tethera_common::structs::transcript::Part;

/// Files, in both directions.
///
/// **Down**: a file an agent handed over. The card comes from the records, and
/// the path behind its id comes from `AssetIndex`, which the same read filled —
/// so a card is fetchable the moment it arrives rather than after a listing.
///
/// **Up**: a file a person sent from a phone. It lands in this machine's own
/// upload directory and stays there until a prompt references it, because
/// getting bytes onto a machine and putting them in front of an agent are two
/// different acts and only the second is something a person asked for.
pub struct LiveAssets {
    conversations: Arc<LiveConversations>,
    index: Arc<AssetIndex>,
    digests: Arc<AssetDigests>,
    uploads: PathBuf,
}

impl LiveAssets {
    /// The largest upload this machine will take.
    ///
    /// Stated rather than left open, and stated in `Describe.limits` so a client
    /// refuses before the transfer rather than after it. A relayed link is slow
    /// enough that finding out at the end is finding out too late.
    pub const MAX_UPLOAD: u64 = 64 * 1024 * 1024;

    /// How many cards one listing carries. Bounded by `PageBudget` in the end,
    /// like every page, but a count keeps the transcript read short.
    const SCAN: u16 = 200;

    pub fn new(
        config: &ApplicationConfig,
        conversations: Arc<LiveConversations>,
        index: Arc<AssetIndex>,
    ) -> Self {
        let assets = Self {
            conversations,
            index,
            digests: AssetDigests::new_shared(),
            uploads: config.data_dir.join("uploads"),
        };

        assets.recover();

        assets
    }

    /// Puts every stored upload back in the index.
    ///
    /// **The index lives in memory and the uploads live on disk.** A file an
    /// agent handed over is registered again whenever its transcript is read, so
    /// it heals itself — but an upload is registered exactly once, at
    /// `put_finish`, and nothing ever re-derives it. Without this, every restart
    /// silently orphans every file a person had sent: the bytes are still there,
    /// and no id reaches them again.
    ///
    /// Re-derived rather than recorded, because the id is a pure function of the
    /// path and the paths are the directory listing. A sidecar file would be a
    /// second copy of something already true on disk, and the two would disagree
    /// the first time somebody deleted an upload by hand.
    fn recover(&self) {
        let Ok(stored) = std::fs::read_dir(&self.uploads) else {
            return;
        };

        let mut found = 0;

        for entry in stored.flatten() {
            let path = entry.path();

            // A `.part` is an upload that never finished. It has no id yet, and
            // giving it one would offer a file that is a prefix of itself.
            if !path.is_file() || path.extension().is_some_and(|kind| kind == "part") {
                continue;
            }

            let canonical = AssetNaming::canonical_of(&path);
            self.index
                .register(AssetNaming::id_for(&canonical), &canonical);

            found += 1;
        }

        if found > 0 {
            tracing::info!(uploads = found, "recovered stored uploads into the asset index");
        }
    }

    /// Both halves answer, so both are advertised.
    pub fn capabilities() -> CapabilitySet {
        [capability::ASSETS_READ, capability::ASSETS_WRITE]
            .into_iter()
            .map(CapabilityId::from)
            .collect()
    }

    /// Where an upload is staged while it arrives.
    ///
    /// Named for the digest the client declared, which is what makes a resumed
    /// upload find its own partial file rather than somebody else's. It is not
    /// yet a claim that the bytes match — `put_finish` is where that is checked.
    fn partial(&self, spec: &PutSpec) -> PathBuf {
        self.uploads.join(format!("{}.part", spec.sha256.0))
    }

    /// The path an id points at, once it is one this machine has read about.
    fn locate(&self, asset: &AssetId) -> Result<PathBuf, WireError> {
        self.index.locate(asset).ok_or_else(|| {
            // A client only ever holds an id this machine issued, so a miss is
            // this machine having forgotten rather than the client having
            // guessed. Said out loud, because the alternative is a refusal with
            // nothing anywhere to say which id went missing.
            tracing::warn!(
                asset = asset.as_str(),
                known = self.index.len(),
                "an asset id resolved to nothing"
            );

            WireError::NotFound {
                kind: EntityKind::Asset,
            }
        })
    }

    /// The digest of a whole file, whatever part of it is about to be sent.
    ///
    /// `FetchHead.sha256` is documented as the whole asset, and a client checks
    /// its download against it. A digest of the range would pass on a truncated
    /// file, which is the one failure the check exists to catch.
    /// Hashed off the runtime's threads and remembered between transfers. This
    /// is read before a single byte of the body moves, so on a large file it is
    /// the whole of what a person sees as a download that has not started - and
    /// a resumed transfer would otherwise pay it again in full to send the last
    /// few megabytes.
    async fn digest_of(&self, path: &Path) -> Result<Digest256, WireError> {
        let digests = self.digests.clone();
        let file = path.to_path_buf();

        tokio::task::spawn_blocking(move || digests.of(&file))
            .await
            .map_err(|error| {
                tracing::warn!(%error, ?path, "hashing a file did not finish");

                WireError::Backend {
                    message: "this machine could not hash that file".to_string(),
                }
            })?
            .map_err(|error| Self::unreadable(path, error))
    }

    fn unreadable(path: &Path, error: std::io::Error) -> WireError {
        tracing::warn!(%error, ?path, "could not read a file this machine offered");

        WireError::Backend {
            message: "this machine could not read that file; it may have moved since".to_string(),
        }
    }

    /// The cards a conversation's records carry, newest first.
    async fn cards_of(&self, scope: &AssetScope, limit: u16) -> Result<Vec<AssetCard>, WireError> {
        let AssetScope::Conversation(conversation) = scope else {
            // A tab is a pane, and a pane has no records. Nothing has ever
            // handed a file over through one.
            return Ok(Vec::new());
        };

        // Reading the transcript is also what fills the index, so a card listed
        // here is fetchable immediately afterwards.
        let page = self
            .conversations
            .transcript(conversation, None, limit.min(Self::SCAN))
            .await?;

        let mut cards = Vec::new();

        for turn in page.items.iter().rev() {
            for part in &turn.parts {
                let Part::File {
                    asset,
                    name,
                    mime,
                    size,
                    ..
                } = part
                else {
                    continue;
                };

                cards.push(AssetCard {
                    asset: asset.clone(),
                    name: name.clone(),
                    mime: mime.clone(),
                    size: *size,
                    // The turn the file was handed over in. The file's own mtime
                    // would be when it was last written, which for a file an
                    // agent produced days ago is a different fact.
                    modified: Some(turn.at),
                });
            }
        }

        Ok(cards)
    }
}

impl AssetPort for LiveAssets {
    /// A plain file handle, seeked to the offset. Nothing is held in memory.
    type Body = std::fs::File;

    async fn list(
        &self,
        scope: &AssetScope,
        _before: Option<Cursor>,
        limit: u16,
    ) -> Result<Page<AssetCard>, WireError> {
        let items = self.cards_of(scope, limit).await?;

        Ok(Page {
            items,
            // One scan of the newest records rather than a walk of every file
            // ever handed over. Its cursor would be a transcript's, and nothing
            // pages this.
            next_before: None,
            has_earlier: false,
        })
    }

    async fn fetch(
        &self,
        asset: &AssetId,
        offset: u64,
    ) -> Result<(FetchHead, Self::Body), WireError> {
        let path = self.locate(asset)?;
        let metadata =
            std::fs::metadata(&path).map_err(|error| Self::unreadable(&path, error))?;

        let len = metadata.len();

        // A client that asks past the end gets an empty body rather than an
        // error: it already has everything, and the head still carries the
        // digest it needs to check what it has.
        let from = offset.min(len);

        let mut body =
            std::fs::File::open(&path).map_err(|error| Self::unreadable(&path, error))?;

        std::io::Seek::seek(&mut body, std::io::SeekFrom::Start(from))
            .map_err(|error| Self::unreadable(&path, error))?;

        let sha256 = self.digest_of(&path).await?;

        let head = FetchHead {
            len,
            // From the extension, because that is all this machine knows and it
            // is what every other tool goes by. A field that is always absent is
            // a field that lies: a viewer told nothing still has to decide, and
            // the way it decides wrong is by drawing a PNG header as text.
            mime: AssetNaming::mime_for(&path),
            sha256,
            // What this machine actually starts at, which the client believes
            // over the offset it asked for.
            offset: from,
        };

        Ok((head, body))
    }

    async fn put_ready(&self, spec: &PutSpec) -> Result<PutReady, WireError> {
        if spec.len > Self::MAX_UPLOAD {
            return Err(WireError::TooLarge {
                size: spec.len,
                limit: Self::MAX_UPLOAD,
            });
        }

        std::fs::create_dir_all(&self.uploads).map_err(|error| {
            tracing::error!(%error, "could not make the upload directory");

            WireError::Backend {
                message: "this machine has nowhere to put an upload".to_string(),
            }
        })?;

        // How much of a previous attempt actually reached disk. Only this
        // machine knows, which is why the client seeks to this rather than to
        // the offset it proposed: an optimistic answer here silently truncates
        // the file and the digest check at the end is what would catch it.
        let held = std::fs::metadata(self.partial(spec))
            .map(|found| found.len())
            .unwrap_or(0);

        Ok(PutReady {
            offset: held.min(spec.len),
        })
    }

    async fn put_finish(&self, spec: &PutSpec, body: &[u8]) -> Result<PutResult, WireError> {
        let partial = self.partial(spec);

        // Appended, because `put_ready` told the client where to start and it
        // sent only the remainder. Truncating here would discard everything an
        // interrupted attempt had already delivered.
        {
            use std::io::Write;

            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&partial)
                .map_err(|error| Self::unreadable(&partial, error))?;

            file.write_all(body)
                .map_err(|error| Self::unreadable(&partial, error))?;
        }

        // The declared digest against the whole file. A mismatch is a corrupt
        // attachment, and issuing an id for it would put a file in front of an
        // agent that is not the file the person sent.
        let found = self.digest_of(&partial).await?;

        if found != spec.sha256 {
            let _ = std::fs::remove_file(&partial);

            return Err(WireError::Backend {
                message: "the upload did not match the digest it declared; nothing was kept"
                    .to_string(),
            });
        }

        let stored = self.uploads.join(Self::stored_name(spec));

        std::fs::rename(&partial, &stored).map_err(|error| Self::unreadable(&partial, error))?;

        let canonical = AssetNaming::canonical_of(&stored);
        let asset = AssetNaming::id_for(&canonical);

        self.index.register(asset.clone(), &canonical);

        // The id and the spelling it was minted from, together. An upload that
        // stores fine and then cannot be found again is invisible without this:
        // the bytes are on disk, the client holds an id, and nothing on either
        // side says which of the two the machine disagrees about.
        tracing::info!(
            asset = asset.as_str(),
            path = %canonical,
            bytes = spec.len,
            "stored an upload"
        );

        Ok(PutResult { asset })
    }
}

impl LiveAssets {
    /// What an upload is called on disk.
    ///
    /// The digest leads, so two uploads of different files never collide and the
    /// same file twice is the same entry. The person's own filename follows it,
    /// because that is what a prompt will name and what an agent will read.
    fn stored_name(spec: &PutSpec) -> String {
        let short: String = spec
            .sha256
            .0
            .chars()
            .take(AssetNaming::STORED_PREFIX_WIDTH)
            .collect();
        let offered = match PathName::basename(&spec.name) {
            "" => "upload",
            name => name,
        };
        let safe: String = offered
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '-'
            })
            .collect();

        format!("{short}-{safe}")
    }
}
