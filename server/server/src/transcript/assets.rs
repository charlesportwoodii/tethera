use sha2::{Digest, Sha256};
use std::path::Path;
use tethera_common::structs::ids::AssetId;

/// How a file on disk becomes an asset id.
///
/// Server-issued and opaque: the id carries no path a caller could bend into a
/// different file, and it is resolvable only because the server's own scan has
/// seen the file.
///
/// This is the only implementation of the rule. When `AssetPort`'s scan lands it
/// must call `id_for`, not re-derive it: two derivations that ever disagree
/// break every `File` card already delivered, and nothing fails loudly when they
/// do.
pub struct AssetNaming;

impl AssetNaming {
    /// Hex characters of the digest that reach the id. Enough that two files on
    /// one machine will not collide, short enough to read in a log line.
    const WIDTH: usize = 12;

    /// Hex characters an upload is stored behind, as in
    /// 78d52cec5b5b-progress-probe.txt.
    ///
    /// Shared rather than written twice: the code that puts the prefix on and
    /// the code that takes it off again to show somebody a name are in
    /// different modules, and two literals that drifted apart would show every
    /// person the digest instead of their own filename.
    pub const STORED_PREFIX_WIDTH: usize = 12;

    /// The id of an already-canonicalised path.
    ///
    /// Takes the resolved string rather than resolving here, so the mapper stays
    /// a pure function: a mapper that stat-ed the filesystem would fall back on
    /// every fixture path in every test, pinning the fallback and leaving the
    /// branch that matters unexercised.
    pub fn id_for(canonical: &str) -> AssetId {
        let digest = Sha256::digest(canonical.as_bytes());
        let hex: String = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            .chars()
            .take(Self::WIDTH)
            .collect();

        AssetId::mint(&hex)
    }

    /// The spelling of a path that `id_for` must be given.
    ///
    /// On Windows `canonicalize` returns the extended-length form, so the same
    /// file hashes two different ways depending on which spelling the caller
    /// started from - and every card built from the other spelling opens onto
    /// nothing. A path that cannot be resolved keeps its own spelling, which is
    /// stable for as long as the file is absent.
    pub fn canonical_of(path: &Path) -> String {
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .into_owned()
    }

    /// The media type of a file, from its name alone.
    ///
    /// **A guess a client is entitled to make itself, made once here instead.**
    /// Nothing reads the bytes: an extension is what every other tool on the
    /// machine goes by, and reading a header would cost a filesystem round trip
    /// in a mapper that deliberately makes none.
    ///
    /// It matters because a viewer that is told nothing has to decide anyway,
    /// and the way it decides wrong is by rendering a PNG's header bytes as
    /// text. `None` is an honest absence — an unknown type — and a client
    /// treating it as "probably text" is the case this exists to shrink.
    pub fn mime_for(path: &Path) -> Option<String> {
        let extension = path.extension()?.to_string_lossy().to_lowercase();

        let mime = match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "bmp" => "image/bmp",
            "svg" => "image/svg+xml",
            "heic" => "image/heic",
            "avif" => "image/avif",
            "pdf" => "application/pdf",
            "txt" | "log" => "text/plain",
            "md" => "text/markdown",
            "csv" => "text/csv",
            "json" => "application/json",
            "toml" => "application/toml",
            "yaml" | "yml" => "application/yaml",
            "html" | "htm" => "text/html",
            "zip" => "application/zip",
            "gz" | "tgz" => "application/gzip",
            "apk" => "application/vnd.android.package-archive",
            "mp4" => "video/mp4",
            "mov" => "video/quicktime",
            "webm" => "video/webm",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "m4a" => "audio/mp4",
            _ => return None,
        };

        Some(mime.to_string())
    }

    /// The same path as a person would write it.
    ///
    /// `canonicalize` returns Windows' extended-length form, `\\?\C:\…`. That
    /// spelling is what the id is minted from and must stay so — but it is not
    /// what belongs in text somebody reads, and it reaches an agent inside a
    /// prompt where some tools do not accept it.
    ///
    /// Stripped only when the plain form still names the same file: the prefix
    /// exists to express paths the ordinary syntax cannot, so a path that is long
    /// enough to need it, or that is a UNC share, keeps it. Handing back a
    /// shorter string that no longer opens would be worse than the noise.
    pub fn plain(path: &str) -> String {
        const PREFIX: &str = r"\\?\";
        const UNC: &str = r"\\?\UNC\";

        // The limit the prefix exists to escape. A path at or past it is only
        // openable in the long form.
        const MAX_PATH: usize = 260;

        if let Some(share) = path.strip_prefix(UNC) {
            return format!(r"\\{share}");
        }

        let Some(rest) = path.strip_prefix(PREFIX) else {
            return path.to_string();
        };

        // A drive letter, a colon and a separator. Anything else behind the
        // prefix is a device path with no plain spelling at all.
        let drive = rest.as_bytes();
        let ordinary = drive.len() > 2
            && drive[0].is_ascii_alphabetic()
            && drive[1] == b':'
            && (drive[2] == b'\\' || drive[2] == b'/');

        if ordinary && rest.len() < MAX_PATH {
            return rest.to_string();
        }

        path.to_string()
    }
}
