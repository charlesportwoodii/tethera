/// What a read added to what was already seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Advance {
    /// The new text, which is empty when nothing was produced.
    Appended(String),
    /// The overlap is gone: more arrived between two reads than the read window
    /// holds, or the pane was cleared. The caller must say so on screen rather
    /// than joining two pieces of output that were never adjacent.
    Jumped,
}

/// Successive reads of a pane, turned back into the stream they came from.
///
/// A backend that publishes no bytes can still be read, and each read returns the
/// last N lines rather than what changed. Two consecutive reads therefore
/// overlap, and the overlap is what says where one ends and the next begins.
///
/// **This never splices.** When the overlap cannot be found the join is unknown,
/// and joining anyway produces output that reads correctly and is wrong - the
/// hardest kind of wrong to notice on a terminal, because output looks like
/// output.
pub struct OutputDelta {
    held: String,
}

impl OutputDelta {
    pub fn new() -> Self {
        Self {
            held: String::new(),
        }
    }

    pub fn advance(&mut self, text: &str) -> Advance {
        if self.held.is_empty() {
            self.held = text.to_string();

            return Advance::Appended(text.to_string());
        }

        // The window did not slide, which is the common case and the only one
        // that can be answered exactly.
        if let Some(rest) = text.strip_prefix(self.held.as_str()) {
            let added = rest.to_string();
            self.held = text.to_string();

            return Advance::Appended(added);
        }

        match self.rejoin(text) {
            Some(added) => {
                self.held = text.to_string();

                Advance::Appended(added)
            }
            None => {
                // Resynchronised on what was handed over, so the next read
                // appends normally instead of jumping for ever.
                self.held = text.to_string();

                Advance::Jumped
            }
        }
    }

    /// How much of the held text is still visible at the start of a new read.
    ///
    /// The longest suffix of what was held that is a prefix of what arrived. A
    /// read window that slid by two lines leaves the rest of the previous read
    /// at the top of this one, and that shared run is the join.
    ///
    /// Longest first, so a screen that repeats itself - blank lines, an
    /// unchanged prompt - rejoins on the longest agreement rather than the first
    /// coincidence. A shorter match would re-emit everything between the two,
    /// which is duplicated output rather than a gap.
    ///
    /// Matched by line rather than by byte: a byte-wise search is quadratic in
    /// the read size, and at four reads a second over a five-hundred-line window
    /// that is real work for an answer the line structure gives directly.
    fn rejoin(&self, text: &str) -> Option<String> {
        let held: Vec<&str> = self.held.lines().collect();
        let fresh: Vec<&str> = text.lines().collect();

        let longest = held.len().min(fresh.len());

        for overlap in (1..=longest).rev() {
            if held[held.len() - overlap..] != fresh[..overlap] {
                continue;
            }

            let after = &fresh[overlap..];

            if after.is_empty() {
                return Some(String::new());
            }

            let mut added = after.join("\n");

            // A read that ended with a newline described a completed line, and
            // dropping it would join the next read's first line onto this one.
            if text.ends_with('\n') {
                added.push('\n');
            }

            return Some(added);
        }

        None
    }
}

impl Default for OutputDelta {
    fn default() -> Self {
        Self::new()
    }
}
