use super::{Record, RecordSpan, TurnGrouping, TurnMapper, TurnSpan};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Where every turn and every tool result sits in one session's file.
///
/// Offsets only. A page seeks and reads the lines it needs, which is what keeps
/// a multi-megabyte session costing a page rather than a file.
pub struct TranscriptIndex {
    path: PathBuf,
    /// How much of the file has been folded in. A line still being written is
    /// deliberately left outside this.
    indexed_len: u64,
    turns: Vec<TurnSpan>,
    /// A tool call's id to the record answering it, so a page can attach a
    /// result recorded long after the turn that asked for it.
    results: HashMap<String, RecordSpan>,
    /// Lines that were not JSON. Counted rather than fatal.
    skipped: u64,
    /// The last record folded in, because grouping asks whether the next record
    /// continues it and that question survives a refresh boundary.
    last: Option<Record>,
    /// Whether the newest turn is still accepting records.
    group_open: bool,
}

impl TranscriptIndex {
    pub fn open(path: PathBuf) -> Self {
        Self {
            path,
            indexed_len: 0,
            turns: Vec::new(),
            results: HashMap::new(),
            skipped: 0,
            last: None,
            group_open: false,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn turns(&self) -> &[TurnSpan] {
        &self.turns
    }

    pub fn turn_count(&self) -> usize {
        self.turns.len()
    }

    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    pub fn indexed_len(&self) -> u64 {
        self.indexed_len
    }

    pub fn result_span(&self, tool_use_id: &str) -> Option<RecordSpan> {
        self.results.get(tool_use_id).copied()
    }

    /// How many turns sit strictly before `offset`.
    ///
    /// The turns are scanned in file order, so their offsets ascend and this is
    /// a binary search.
    pub fn position_before(&self, offset: u64) -> usize {
        self.turns.partition_point(|turn| turn.offset < offset)
    }

    pub fn holds_offset(&self, offset: u64) -> bool {
        self.turns
            .binary_search_by_key(&offset, |turn| turn.offset)
            .is_ok()
    }

    pub fn first_after(&self, offset: u64) -> Option<u64> {
        self.turns
            .iter()
            .find(|turn| turn.offset > offset)
            .map(|turn| turn.offset)
    }

    pub fn newest_offset(&self) -> Option<u64> {
        self.turns.last().map(|turn| turn.offset)
    }

    /// Folds in whatever has been appended since the last call.
    ///
    /// Takes the mapper rather than deciding for itself what counts as a turn:
    /// an index that counted a record the mapper then dropped would hand back
    /// short pages and a `has_earlier` about entries rather than about turns.
    pub fn refresh(&mut self, mapper: &TurnMapper) -> std::io::Result<()> {
        let length = match std::fs::metadata(&self.path) {
            Ok(meta) => meta.len(),
            // A session whose file has not appeared yet is empty, not broken.
            // The hook fires before the harness writes its first record.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };

        if length == self.indexed_len {
            return Ok(());
        }

        // Shorter than what was folded in means the offsets held here no longer
        // point at what they pointed at. Nothing partial can be salvaged.
        if length < self.indexed_len {
            self.reset();
        }

        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.indexed_len))?;

        let mut reader = BufReader::new(file);
        let mut offset = self.indexed_len;
        let mut line = String::new();

        loop {
            line.clear();

            let read = reader.read_line(&mut line)?;

            if read == 0 {
                break;
            }

            // No newline means the writer is mid-record. Leaving it outside
            // `indexed_len` is what makes the next refresh pick it up whole,
            // and it is the ordinary steady state of a file being appended to.
            if !line.ends_with('\n') {
                break;
            }

            self.consume(&line, RecordSpan { offset, len: read }, mapper);
            offset += read as u64;
        }

        self.indexed_len = offset;

        Ok(())
    }

    fn reset(&mut self) {
        self.indexed_len = 0;
        self.turns.clear();
        self.results.clear();
        self.skipped = 0;
        self.last = None;
        self.group_open = false;
    }

    fn consume(&mut self, line: &str, span: RecordSpan, mapper: &TurnMapper) {
        let record: Record = match serde_json::from_str(line) {
            Ok(record) => record,
            // One unreadable line must not fail a page. A half-written record is
            // normal; a malformed one is rare and is still not worth losing the
            // rest of a conversation over.
            Err(_) => {
                self.skipped += 1;

                return;
            }
        };

        for id in record.answered_tool_ids() {
            self.results.insert(id, span);
        }

        let joins = self
            .last
            .as_ref()
            .is_some_and(|previous| TurnGrouping::joins(previous, &record));

        if joins && self.group_open {
            if let Some(turn) = self.turns.last_mut() {
                turn.records.push(span);
            }
        } else if mapper.yields_turn(&record) {
            self.turns.push(TurnSpan::new(span));
            self.group_open = true;
        } else {
            self.group_open = false;
        }

        self.last = Some(record);
    }
}
