/// Paging a pane's history against a source that only counts from the bottom.
///
/// `herdr pane read --source recent --lines N` answers the newest `N` lines it
/// holds, oldest first, and answers fewer when it holds fewer. It has no cursor
/// and no length. So the cursor this protocol carries is defined here, and it
/// counts from the newest line: `before_line` is how many lines the client has
/// already been shown.
///
/// Nothing is derived from `PaneInfo.scroll`.
/// `max_offset_from_bottom + viewport_rows` is an upper bound and not a length —
/// a pane reporting a 36-row viewport with one line of content answers one line
/// — and a window planned from it claims pages that do not exist. Depth comes
/// from what the read returned, which is the only number that cannot be wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbackWindow {
    /// How many lines the client has already been shown, from the newest.
    pub before: u32,
    /// What to ask `pane read --lines` for.
    pub lines_to_request: u32,
    /// The page, capped by the caller's limit.
    pub limit: u32,
}

/// One planned page, resolved against the lines that actually came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollbackPageOf<T> {
    pub lines: Vec<T>,
    pub next_before_line: Option<u32>,
    pub has_earlier: bool,
}

impl ScrollbackWindow {
    /// The most lines one request will ever ask herdr for.
    ///
    /// Paging deeper re-reads from the bottom each time, so an unbounded
    /// `before` would let a client ask for the whole buffer one page at a time
    /// and pay for all of it on every page.
    pub const MAX_LINES: u32 = 4096;

    pub fn plan(before_line: Option<u32>, limit: u16) -> Self {
        let before = before_line.unwrap_or(0);
        let limit = u32::from(limit);

        Self {
            before,
            lines_to_request: before.saturating_add(limit).min(Self::MAX_LINES),
            limit,
        }
    }

    /// The page out of what came back.
    ///
    /// The read is anchored at the bottom, so the newest `before` lines are the
    /// tail of `lines` and the page is what sits in front of them. When herdr
    /// returned fewer lines than were asked for, there is nothing earlier — that
    /// is the whole buffer, and saying `has_earlier` here would invite a client
    /// to page into a hole.
    pub fn resolve<T>(&self, lines: Vec<T>) -> ScrollbackPageOf<T> {
        let returned = u32::try_from(lines.len()).unwrap_or(u32::MAX);
        let keep = returned.saturating_sub(self.before).min(self.limit) as usize;

        let mut lines = lines;
        lines.truncate(keep);

        // Everything asked for came back, so the buffer may hold more. Anything
        // short of that is the bottom of what herdr has.
        let has_earlier = returned >= self.lines_to_request && keep > 0;

        ScrollbackPageOf {
            next_before_line: has_earlier
                .then(|| self.before.saturating_add(keep as u32)),
            has_earlier,
            lines,
        }
    }
}
