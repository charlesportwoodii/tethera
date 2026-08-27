use crate::error::ClientError;
use crate::transfer::Digest;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tethera_common::structs::primitives::Sha256;

/// An open partial, appending and hashing the same bytes.
///
/// The two are held together on purpose. `FetchHead::sha256` covers the whole
/// asset, and a resumed download never sees its own first half - so the digest
/// has to be primed from the file and then kept in step with every write. Two
/// separate objects would let a caller write without hashing, and the result is
/// a file that fails a check it should have passed, deleted for being intact.
pub struct Sink {
    path: PathBuf,
    file: File,
    digest: Digest,
    /// The whole file's length, counting the prefix kept from a previous
    /// attempt. This is what a progress bar shows.
    written: u64,
}

impl Sink {
    /// How much is read back per pass when priming the digest.
    pub const CHUNK: usize = 64 * 1024;

    pub(crate) fn new(path: PathBuf, file: File, digest: Digest, written: u64) -> Self {
        Self {
            path,
            file,
            digest,
            written,
        }
    }

    pub fn write(&mut self, chunk: &[u8]) -> Result<(), ClientError> {
        self.file.write_all(chunk).map_err(|error| self.blame(error))?;

        self.digest.eat(chunk);
        self.written += chunk.len() as u64;

        Ok(())
    }

    /// Bytes on disk, counting what a previous attempt left.
    pub fn bytes(&self) -> u64 {
        self.written
    }

    /// Flushes, and answers the digest of everything in the file.
    ///
    /// Compared against `FetchHead::sha256` by the caller. A resumed transfer is
    /// checkable here and nowhere else: the only place the whole file exists is
    /// this one.
    pub fn finish(mut self) -> Result<Sha256, ClientError> {
        if let Err(error) = self.file.flush() {
            return Err(self.blame(error));
        }

        Ok(self.digest.finish())
    }

    fn blame(&self, error: std::io::Error) -> ClientError {
        ClientError::Partial {
            path: self.path.display().to_string(),
            reason: error.to_string(),
        }
    }
}
