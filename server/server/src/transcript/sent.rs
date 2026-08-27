use super::AssetNaming;
use std::path::{Path, PathBuf};

/// The files a person sent, read back out of their own prompt.
///
/// **The history was one-sided.** A file an agent hands over becomes a
/// `Part::File`, so it draws as a card and opens in the viewer. A file the
/// person sent became a line of text inside their own turn:
///
/// ```text
/// Retry after the index fix
///
/// Attached: C:\Users\…\uploads\78d52cec5b5b-progress-probe.txt
/// ```
///
/// So they could see everything shared *with* them and nothing they shared
/// themselves — and the half they could not see was rendered as an absolute
/// path, which is the least readable thing on a phone.
///
/// This is not parsing a harness's output. `LiveConversations::naming` writes
/// those lines, in that exact form, so reading them back is reading our own
/// record. That is what makes it safe, and it is why the anchor below is not a
/// pattern match on the text.
///
/// Retroactive with no migration: the lines are already in every transcript that
/// ever carried an attachment, so old conversations gain their cards the next
/// time they are read.
pub struct SentFiles;

impl SentFiles {
    /// The exact prefix `naming` writes, including its trailing space.
    const MARKER: &'static str = "Attached: ";

    /// Splits a prompt into what the person wrote and the files named in it.
    ///
    /// **Only lines this machine wrote are lifted, and the upload directory is
    /// what settles it.** A path inside this machine's own store is one it put
    /// there; anything else — including a person who types the word "Attached:"
    /// about a file of their own — is prose, and prose belongs in the bubble
    /// untouched.
    ///
    /// The line is removed rather than left beside the card. A card and a raw
    /// path is the same file said twice, and only one of the two is readable.
    ///
    /// No filesystem access, deliberately. The mapper is a pure function of its
    /// records — a stat here would make every fixture path in every test fall
    /// down the absent branch, pinning the fallback and leaving the one that
    /// matters unexercised. Size comes from `FetchHead` when the card is opened,
    /// which is the only moment it is needed and the only moment it is true.
    pub fn split(text: &str, uploads: &Path) -> (String, Vec<PathBuf>) {
        if !text.contains(Self::MARKER) {
            return (text.to_string(), Vec::new());
        }

        let mut spoken: Vec<&str> = Vec::new();
        let mut files = Vec::new();

        for line in text.lines() {
            match Self::named(line, uploads) {
                Some(path) => files.push(path),
                None => spoken.push(line),
            }
        }

        if files.is_empty() {
            return (text.to_string(), Vec::new());
        }

        // `naming` separates the words from the first path with a blank line, so
        // lifting every path leaves that blank line trailing. Trimming the end
        // only: leading whitespace is the person's, and a prompt that is nothing
        // but attachments correctly becomes empty.
        (spoken.join("\n").trim_end().to_string(), files)
    }

    /// The path this line names, when the line is one this machine wrote.
    fn named(line: &str, uploads: &Path) -> Option<PathBuf> {
        let path = PathBuf::from(line.strip_prefix(Self::MARKER)?.trim());

        Self::inside(&path, uploads).then_some(path)
    }

    /// Whether a path sits inside this machine's upload directory.
    ///
    /// Compared as normalised text rather than with `starts_with` on components,
    /// because the two spellings genuinely differ: the stored path has been
    /// through `canonicalize` and then had Windows' extended-length prefix
    /// stripped for the prompt, while the directory here is built from
    /// configuration. Separators and case both vary between them.
    ///
    /// A prefix match on a normalised string would also admit a sibling
    /// directory whose name merely starts with the same characters, so the
    /// boundary is required to fall on a separator.
    pub fn is_stored_in(path: &Path, uploads: &Path) -> bool {
        Self::inside(path, uploads)
    }

    fn inside(path: &Path, uploads: &Path) -> bool {
        let normalise = |value: &Path| {
            value
                .to_string_lossy()
                .trim_end_matches(['/', '\\'])
                .replace('\\', "/")
                .to_lowercase()
        };

        let store = normalise(uploads);

        if store.is_empty() {
            return false;
        }

        normalise(path)
            .strip_prefix(&store)
            .is_some_and(|rest| rest.starts_with('/'))
    }

    /// The name to show for a stored upload.
    ///
    /// Files land in the store under a digest prefix so two people sending
    /// `screenshot.png` do not overwrite each other. **That prefix is storage,
    /// not identity** — a card reading `78d52cec5b5b-progress-probe.txt` shows
    /// somebody a name they never chose, in place of the one they did.
    ///
    /// Only the exact shape the store writes is stripped, so a file genuinely
    /// named with a leading hyphenated word keeps it.
    pub fn readable_name(path: &Path) -> String {
        let stored = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();

        let Some((prefix, rest)) = stored.split_once('-') else {
            return stored;
        };

        let stored_by_us = prefix.len() == AssetNaming::STORED_PREFIX_WIDTH
            && prefix.bytes().all(|byte| byte.is_ascii_hexdigit())
            && !rest.is_empty();

        if stored_by_us {
            rest.to_string()
        } else {
            stored
        }
    }
}
