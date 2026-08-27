use super::{Record, SessionSummary};
use moka::sync::Cache;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tethera_common::structs::primitives::Timestamp;

/// Every session this machine holds, found on disk.
///
/// One file is one conversation for its whole life - a resume days later appends
/// to the same file under the same id - so a session found here is complete
/// history, not a fragment.
pub struct SessionCatalog {
    root: PathBuf,
    summaries: Cache<(PathBuf, i64), SessionSummary>,
}

impl SessionCatalog {
    /// The newest sessions to look at. Beyond this the older ones are not
    /// listed, and the truncation is logged rather than presented as "this
    /// machine has no more".
    pub const SCAN_LIMIT: usize = 200;

    /// How much of each end of a file is read to summarise it.
    pub const EDGE: u64 = 64 * 1024;

    /// Both bounded, because a cache with neither is a leak with a lookup
    /// method.
    const CACHE_CAPACITY: u64 = 512;
    const CACHE_TTL: Duration = Duration::from_secs(300);

    pub fn new(home: &Path) -> Self {
        Self {
            root: home.join(".claude").join("projects"),
            summaries: Cache::builder()
                .max_capacity(Self::CACHE_CAPACITY)
                .time_to_live(Self::CACHE_TTL)
                .build(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every session file, newest first, capped.
    pub fn discover(&self) -> Vec<PathBuf> {
        let mut found: Vec<(i64, PathBuf)> = Vec::new();

        let projects = match std::fs::read_dir(&self.root) {
            Ok(projects) => projects,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
            // An empty answer here would read as "this machine has no
            // conversations", which is a different statement from "this machine
            // could not look". Only the log can tell them apart.
            Err(error) => {
                tracing::warn!(%error, root = ?self.root, "could not read the agent's project directory");

                return Vec::new();
            }
        };

        for project in projects.flatten() {
            let Ok(sessions) = std::fs::read_dir(project.path()) else {
                continue;
            };

            for session in sessions.flatten() {
                let path = session.path();

                if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                    continue;
                }

                let modified = session
                    .metadata()
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|since| since.as_secs() as i64)
                    .unwrap_or_default();

                found.push((modified, path));
            }
        }

        found.sort_by(|left, right| right.0.cmp(&left.0));

        if found.len() > Self::SCAN_LIMIT {
            tracing::info!(
                found = found.len(),
                listed = Self::SCAN_LIMIT,
                "more sessions on disk than this listing carries"
            );
        }

        found
            .into_iter()
            .take(Self::SCAN_LIMIT)
            .map(|(_, path)| path)
            .collect()
    }

    /// One session's file, found by its id alone.
    ///
    /// The id is all herdr hands over - it records `kind: "id"` and discards the
    /// path - so this is the only way from a conversation to its records when no
    /// working directory is in hand.
    pub fn locate(&self, session_id: &str) -> Option<PathBuf> {
        let name = format!("{session_id}.jsonl");

        let projects = std::fs::read_dir(&self.root).ok()?;

        for project in projects.flatten() {
            let candidate = project.path().join(&name);

            if candidate.is_file() {
                return Some(candidate);
            }
        }

        None
    }

    pub fn summarise(&self, path: &Path) -> Option<SessionSummary> {
        let modified = std::fs::metadata(path)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_secs() as i64)
            .unwrap_or_default();

        // The mtime is part of the key, so a file that changed is a miss rather
        // than a stale hit, and the superseded entry ages out on its own.
        let key = (path.to_path_buf(), modified);

        if let Some(cached) = self.summaries.get(&key) {
            return Some(cached);
        }

        let summary = self.read_summary(path)?;
        self.summaries.insert(key, summary.clone());

        Some(summary)
    }

    /// The session id a file is named for.
    pub fn session_of(path: &Path) -> Option<String> {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
    }

    fn read_summary(&self, path: &Path) -> Option<SessionSummary> {
        let session_id = Self::session_of(path)?;

        let head = Self::records(&Self::head(path)?);
        let tail = Self::records(&Self::tail(path)?);

        let cwd = head
            .iter()
            .find_map(|record| record.cwd.clone())
            .or_else(|| tail.iter().find_map(|record| record.cwd.clone()));

        let started_at = head.iter().find_map(Record::at);

        // Read from the tail, and from the head when a session is short enough
        // that the two overlap or the tail carried no stamped record.
        let last_active = tail
            .iter()
            .rev()
            .find_map(Record::at)
            .or_else(|| head.iter().rev().find_map(Record::at));

        // A name a person typed beats one the harness wrote, however old it is.
        // Recency alone would be wrong here: the harness titles a session from
        // its first turn and never revises it, but it keeps writing records
        // after a rename, so the newest record carrying *a* title is routinely
        // still the machine's.
        let title = Self::newest(&head, &tail, |record| record.custom_title.clone())
            .or_else(|| Self::newest(&head, &tail, |record| record.ai_title.clone()));

        Some(SessionSummary {
            session_id,
            path: path.to_path_buf(),
            cwd,
            started_at,
            last_active,
            title,
            // Filled by the caller from mapped turns, because deciding what is
            // meaningful is the noise filter's judgement and not a text scan's.
            preview: None,
        })
    }

    /// The newest record either window holds that answers, tail before head.
    ///
    /// A session short enough that the two windows overlap, or whose tail
    /// carried nothing of the kind asked for, is why the head is consulted at
    /// all — a title written in the first few records would otherwise be
    /// invisible on any session long enough to have a separate tail.
    fn newest<T>(
        head: &[Record],
        tail: &[Record],
        of: impl Fn(&Record) -> Option<T> + Copy,
    ) -> Option<T> {
        tail.iter()
            .rev()
            .find_map(of)
            .or_else(|| head.iter().rev().find_map(of))
    }

    fn head(path: &Path) -> Option<String> {
        use std::io::Read;

        let mut file = std::fs::File::open(path).ok()?;
        let mut body = vec![0u8; Self::EDGE as usize];
        let read = file.read(&mut body).ok()?;

        body.truncate(read);

        Some(String::from_utf8_lossy(&body).into_owned())
    }

    fn tail(path: &Path) -> Option<String> {
        use std::io::{Read, Seek, SeekFrom};

        let mut file = std::fs::File::open(path).ok()?;
        let length = file.metadata().ok()?.len();
        let from = length.saturating_sub(Self::EDGE);

        file.seek(SeekFrom::Start(from)).ok()?;

        let mut body = Vec::new();
        file.read_to_end(&mut body).ok()?;

        let text = String::from_utf8_lossy(&body).into_owned();

        // A read that did not start at the beginning starts mid-record, and that
        // fragment is not a record.
        if from == 0 {
            return Some(text);
        }

        text.find('\n').map(|at| text[at + 1..].to_string())
    }

    fn records(text: &str) -> Vec<Record> {
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str::<Record>(line).ok())
            .collect()
    }

    pub fn to_millis(seconds: i64) -> Timestamp {
        Timestamp(seconds * 1_000)
    }
}
