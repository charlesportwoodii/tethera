use super::{
    AssetIndex, ContentBlock, PageBudget, Record, RecordSpan, StatsRule, ToolOutcome,
    TranscriptIndex, TurnMapper,
};
use std::sync::Arc;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use tethera_common::protocol::error::WireError;
use tethera_common::protocol::response::Page;
use tethera_common::structs::agent::Agent;
use tethera_common::structs::conversation::AgentStats;
use tethera_common::structs::primitives::Cursor;
use tethera_common::structs::transcript::Turn;

/// One session's records, as pages of turns and as a tail.
pub struct TranscriptReader {
    index: TranscriptIndex,
    mapper: TurnMapper,
}

impl TranscriptReader {
    /// The prefix on every cursor this reader mints.
    ///
    /// Present so a cursor from some later encoding is distinguishable from this
    /// one rather than parsing as a small number.
    const CURSOR_PREFIX: char = 'o';

    /// How many turns at the tail the figures are computed over.
    ///
    /// Enough to reach back past the newest model response to the thing the
    /// person said that started this turn, which is what the elapsed count
    /// measures from. Reading the whole file to add up one request would be work
    /// for nothing.
    const STATS_TAIL: u16 = 8;

    pub fn open(path: PathBuf, agent: Agent) -> Self {
        Self {
            index: TranscriptIndex::open(path),
            mapper: TurnMapper::new(agent),
        }
    }

    /// The same reader, recording where every file it reads about can be found.
    ///
    /// This is what makes a `Part::File` card openable: the id on it is a
    /// one-way hash, and the read that produces the card is the only moment the
    /// path is in hand.
    pub fn indexing(
        path: PathBuf,
        agent: Agent,
        assets: Arc<AssetIndex>,
        uploads: PathBuf,
    ) -> Self {
        Self {
            index: TranscriptIndex::open(path),
            mapper: TurnMapper::indexing(agent, assets).from_store(uploads),
        }
    }

    pub fn cursor_of(offset: u64) -> Cursor {
        Cursor(format!("{}{offset}", Self::CURSOR_PREFIX))
    }

    pub fn offset_of(cursor: &Cursor) -> Option<u64> {
        cursor
            .as_str()
            .strip_prefix(Self::CURSOR_PREFIX)?
            .parse()
            .ok()
    }

    pub fn turn_count(&self) -> usize {
        self.index.turn_count()
    }

    pub fn skipped(&self) -> u64 {
        self.index.skipped()
    }

    /// Whether this session's file exists and holds anything readable.
    pub fn is_readable(&mut self) -> bool {
        self.refresh().is_ok() && self.index.path().exists()
    }

    fn refresh(&mut self) -> Result<(), WireError> {
        self.index.refresh(&self.mapper).map_err(|error| {
            tracing::warn!(%error, path = ?self.index.path(), "could not read a transcript");

            WireError::Backend {
                message: "this machine could not read the conversation's records".to_string(),
            }
        })
    }

    /// One page, newest last, ending strictly before `before`.
    pub fn page(&mut self, before: Option<&Cursor>, limit: u16) -> Result<Page<Turn>, WireError> {
        self.refresh()?;

        let end = match before {
            None => self.index.turn_count(),
            Some(cursor) => {
                let offset = Self::offset_of(cursor).ok_or(WireError::Stale)?;

                self.index.position_before(offset)
            }
        };

        let take = usize::from(limit).min(end);
        let floor = end - take;

        // Built newest-first and stopped as soon as the next turn would not fit,
        // so only what is sent is ever read. The count the caller gave is a
        // ceiling; what decides is how many turns fit in one frame, because the
        // page is delivered as one and a page bounded only by a count is a frame
        // of unbounded size.
        let mut items: Vec<Turn> = Vec::new();
        let mut used = 0;
        let mut start = end;

        while start > floor {
            let span = self.index.turns()[start - 1].clone();
            start -= 1;

            let Some(turn) = self.turns_of(&[span])?.pop() else {
                continue;
            };

            let size = PageBudget::size_of(&turn);

            // At least one turn always survives. A page that came back empty
            // because its newest turn was large would make the client page
            // forever without advancing.
            if !items.is_empty() && used + size > PageBudget::MAX_PAGE_BYTES {
                start += 1;

                break;
            }

            used += size;
            items.push(turn);
        }

        items.reverse();

        // Paging cannot help a single turn that alone exceeds the budget: the
        // next page would carry the same turn and fail the same way. Shrinking it
        // keeps the conversation openable at that point in its history.
        if items.len() == 1 && used > PageBudget::MAX_PAGE_BYTES {
            let only = items.remove(0);
            items.push(PageBudget::shrink(only));
        }

        Ok(Page {
            items,
            next_before: if start == 0 {
                None
            } else {
                Some(Self::cursor_of(self.index.turns()[start].offset))
            },
            // The source's own answer, never a scroll metric: an agent owning
            // the alternate screen reports no scrollback while its transcript
            // runs to megabytes.
            has_earlier: start > 0,
        })
    }

    /// What the agent is doing right now, in figures.
    ///
    /// Read from the records rather than from the turns, because the numbers are
    /// on the records and the mapper drops them: token counts belong to a model
    /// request, and a turn is a rejoined group of however many records that
    /// request happened to produce.
    ///
    /// `running` is the tool call in flight, which the caller has from the turns
    /// it already read — this does not go looking for it a second time.
    pub fn stats(&mut self, running: Option<String>) -> Result<Option<AgentStats>, WireError> {
        self.refresh()?;

        let turns = self.index.turns();
        let from = turns.len().saturating_sub(usize::from(Self::STATS_TAIL));

        let spans: Vec<RecordSpan> = turns[from..]
            .iter()
            .flat_map(|turn| turn.records.iter().copied())
            .collect();

        let records = self.records_of(&spans)?;

        Ok(StatsRule::of(&records, running))
    }

    /// Every turn after `cursor`, oldest first.
    pub fn turns_after(&mut self, cursor: &Cursor) -> Result<Vec<Turn>, WireError> {
        self.refresh()?;

        let Some(offset) = Self::offset_of(cursor) else {
            return Ok(Vec::new());
        };

        let spans: Vec<_> = self
            .index
            .turns()
            .iter()
            .filter(|turn| turn.offset > offset)
            .cloned()
            .collect();

        self.turns_of(&spans)
    }

    /// Where a stream asked to resume at `after` can actually start.
    ///
    /// Not always the `after` it was given. A later answer is the signal that
    /// tells a client to refetch the gap rather than render a hole it cannot
    /// see; `Stale` is the answer when the cursor belongs to a different file
    /// altogether, which is what a resumed session into a new file produces.
    pub fn open_from(&mut self, after: Option<&Cursor>) -> Result<Cursor, WireError> {
        self.refresh()?;

        let Some(cursor) = after else {
            return Ok(Self::cursor_of(self.index.newest_offset().unwrap_or(0)));
        };

        // Unparseable cannot be positioned against, so it cannot be honoured and
        // cannot be corrected either.
        let offset = Self::offset_of(cursor).ok_or(WireError::Stale)?;

        if self.index.holds_offset(offset) {
            return Ok(Self::cursor_of(offset));
        }

        match self.index.first_after(offset) {
            Some(later) => Ok(Self::cursor_of(later)),
            None => Err(WireError::Stale),
        }
    }

    fn turns_of(&mut self, spans: &[super::TurnSpan]) -> Result<Vec<Turn>, WireError> {
        let mut turns = Vec::with_capacity(spans.len());

        for span in spans {
            let records = self.records_of(&span.records)?;
            let outcomes = self.outcomes_for(&records)?;

            if let Some(turn) = self
                .mapper
                .turn(&records, Self::cursor_of(span.offset), &outcomes)
            {
                turns.push(turn);
            }
        }

        Ok(turns)
    }

    fn records_of(&self, spans: &[RecordSpan]) -> Result<Vec<Record>, WireError> {
        let mut file = self.file()?;

        spans
            .iter()
            .map(|span| Self::record_at(&mut file, *span))
            .collect()
    }

    /// The results for every tool call these records make.
    ///
    /// Read from wherever in the file the answer landed, which is why a call on
    /// an early page still shows the result recorded much later.
    fn outcomes_for(&self, records: &[Record]) -> Result<HashMap<String, ToolOutcome>, WireError> {
        let mut outcomes = HashMap::new();
        let mut file = self.file()?;

        for record in records {
            for block in record.blocks() {
                let ContentBlock::ToolUse { id, .. } = block else {
                    continue;
                };

                let Some(span) = self.index.result_span(id) else {
                    continue;
                };

                let answer = Self::record_at(&mut file, span)?;

                for (answered, outcome) in ToolOutcome::from_record(&answer) {
                    outcomes.insert(answered, outcome);
                }
            }
        }

        Ok(outcomes)
    }

    fn file(&self) -> Result<File, WireError> {
        File::open(self.index.path()).map_err(|error| {
            tracing::warn!(%error, path = ?self.index.path(), "could not open a transcript");

            WireError::Backend {
                message: "this machine could not read the conversation's records".to_string(),
            }
        })
    }

    fn record_at(file: &mut File, span: RecordSpan) -> Result<Record, WireError> {
        let mut body = vec![0u8; span.len];

        file.seek(SeekFrom::Start(span.offset))
            .and_then(|_| file.read_exact(&mut body))
            .map_err(|error| {
                tracing::warn!(%error, offset = span.offset, "could not read a transcript record");

                WireError::Backend {
                    message: "this machine could not read the conversation's records".to_string(),
                }
            })?;

        // The index only ever records spans it parsed, so a failure here is the
        // file having changed underneath a page rather than bad input.
        serde_json::from_slice(&body).map_err(|error| {
            tracing::warn!(%error, offset = span.offset, "a transcript record changed while it was being read");

            WireError::Backend {
                message: "the conversation's records changed while they were being read".to_string(),
            }
        })
    }
}
