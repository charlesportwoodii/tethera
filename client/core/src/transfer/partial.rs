use crate::error::ClientError;
use crate::transfer::{Digest, Sink};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tethera_common::structs::ids::AssetId;

/// A download's bytes on disk between one attempt and the next.
///
/// This is the whole of what makes backgrounding survivable. A phone that
/// switches apps loses its connection, and without somewhere for the bytes to
/// wait, every kill discards the transfer rather than pausing it.
///
/// The file lives where this app can reach it without asking anybody: a
/// destination the person picked is a `content://` URI on Android, whose write
/// permission is not persistable and does not survive a restart, and whose
/// length cannot be read back reliably. So bytes land here, and only a transfer
/// that finished and matched its digest is copied out to where it was asked for.
pub struct Partial {
    path: PathBuf,
}

impl Partial {
    pub const SUFFIX: &'static str = ".part";

    /// How long an abandoned partial is kept.
    ///
    /// Long enough that a person who put their phone down over a weekend still
    /// resumes rather than starting again, short enough that a directory of
    /// half-finished gigabytes is not permanent.
    pub const STALE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

    /// The partial for one asset, whether or not anything is written yet.
    ///
    /// Named by the digest of the id rather than by the id. `AssetId::parse`
    /// checks the prefix and that something follows it, nothing more, and the
    /// id arrives from a peer - so `as_../../` spelled straight into a filename
    /// writes wherever that peer says. Hashing also fixes the length, which a
    /// filesystem with a name limit would otherwise decide for us.
    pub fn new(dir: &Path, asset: &AssetId) -> Self {
        let name = Digest::of(asset.as_str().as_bytes());

        Self {
            path: dir.join(format!("{}{}", name.as_str(), Self::SUFFIX)),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Bytes already here, and the offset the next attempt asks the machine for.
    ///
    /// An absent partial reads as zero rather than as a failure: no previous
    /// attempt is the ordinary case, and it is a fresh download rather than an
    /// error to report.
    pub fn bytes(&self) -> u64 {
        std::fs::metadata(&self.path).map(|held| held.len()).unwrap_or(0)
    }

    /// Cuts the file back to `offset` and answers something that appends there.
    ///
    /// `offset` is the machine's `FetchHead::offset`, never the one that was
    /// asked for. A machine is free to start earlier than requested - it may
    /// hold a different file under that id now - and keeping bytes it will not
    /// vouch for produces a file of exactly the right length with a wrong
    /// middle, which is the one corruption a digest catches too late to be
    /// cheap.
    ///
    /// Starting *past* what is here is refused instead. Appending at nine with
    /// three bytes on disk leaves five bytes of hole under a length that looks
    /// correct.
    pub fn resume(&self, offset: u64) -> Result<Sink, ClientError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| self.blame(error))?;
        }

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
            .map_err(|error| self.blame(error))?;

        let held = file.metadata().map_err(|error| self.blame(error))?.len();

        if offset > held {
            return Err(ClientError::Partial {
                path: self.path.display().to_string(),
                reason: format!(
                    "the machine wants to start at {offset} and only {held} bytes are here; \
                     delete it and start the download again"
                ),
            });
        }

        if held > offset {
            file.set_len(offset).map_err(|error| self.blame(error))?;
        }

        // Hashed from the file rather than from the wire. A resumed transfer
        // never sees its own first half, so a digest fed only what arrives
        // checks nothing - and `FetchHead::sha256` covers the whole asset.
        let mut digest = Digest::new();

        file.seek(SeekFrom::Start(0)).map_err(|error| self.blame(error))?;

        let mut chunk = vec![0u8; Sink::CHUNK];
        let mut read = 0u64;

        while read < offset {
            let want = usize::try_from(offset - read).unwrap_or(Sink::CHUNK).min(Sink::CHUNK);

            file.read_exact(&mut chunk[..want])
                .map_err(|error| self.blame(error))?;

            digest.eat(&chunk[..want]);
            read += want as u64;
        }

        file.seek(SeekFrom::End(0)).map_err(|error| self.blame(error))?;

        Ok(Sink::new(self.path.clone(), file, digest, offset))
    }

    /// Throws the bytes away.
    ///
    /// Called when a download is abandoned on purpose, never when one fails: a
    /// failure is what the file exists for.
    pub fn discard(&self) {
        std::fs::remove_file(&self.path).ok();
    }

    /// Removes partials nobody has touched for `older_than`.
    ///
    /// Silent about every failure. This runs at the start of an unrelated
    /// download, and a directory that cannot be read is not a reason to refuse
    /// the transfer somebody just asked for.
    pub fn sweep(dir: &Path, older_than: Duration) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };

        let now = SystemTime::now();

        for entry in entries.flatten() {
            let path = entry.path();

            if path.extension().and_then(|part| part.to_str()) != Some(&Self::SUFFIX[1..]) {
                continue;
            }

            let stale = entry
                .metadata()
                .and_then(|held| held.modified())
                .map(|touched| {
                    now.duration_since(touched)
                        .map(|since| since >= older_than)
                        .unwrap_or(false)
                })
                .unwrap_or(false);

            if stale {
                std::fs::remove_file(&path).ok();
            }
        }
    }

    fn blame(&self, error: std::io::Error) -> ClientError {
        ClientError::Partial {
            path: self.path.display().to_string(),
            reason: error.to_string(),
        }
    }
}
