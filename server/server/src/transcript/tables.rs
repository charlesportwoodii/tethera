/// A run of agent prose that renders one way.
pub enum Segment {
    Prose(String),
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        source: String,
    },
}

/// Finds GitHub-flavoured tables in agent prose so they can travel as tables.
///
/// `Part::Table` and the client rendering for it both already existed; nothing
/// ever produced one. A table therefore reached the phone inside a text part,
/// where the markdown renderer has no table block: the pipes and dashes drew as
/// themselves and the newlines collapsed, turning a five-row comparison into one
/// unreadable sentence. Agents write tables constantly, so this is most of what
/// a long answer looks like on a phone.
///
/// It belongs here rather than in the client because the wire type is the seam
/// the client is already written against, and because a decision made here can
/// be tested without a device.
pub struct MarkdownTables;

impl MarkdownTables {
    /// Rows a table may have before it is left as prose.
    ///
    /// A guard against a paragraph of pipes rather than a real limit; no answer
    /// carries a table this long, and drawing one would cost more to read than
    /// the text it replaced.
    const MAX_ROWS: usize = 200;

    /// The prose in order, with each table lifted into its own segment.
    ///
    /// A table is a header row, a delimiter row of dashes directly beneath it,
    /// and the rows that follow while they still look like rows. **The
    /// delimiter is what decides.** Prose is full of pipes — a shell pipeline, a
    /// regex alternation — and only the dashes under a line make the line above
    /// it a header. Without that, a sentence becomes a one-column table and its
    /// words disappear into cells.
    pub fn split(source: &str) -> Vec<Segment> {
        let lines: Vec<&str> = source.lines().collect();
        let mut segments = Vec::new();
        let mut prose: Vec<&str> = Vec::new();
        let mut at = 0;

        while at < lines.len() {
            let header = lines[at];

            let table = lines
                .get(at + 1)
                .is_some_and(|next| Self::is_row(header) && Self::is_delimiter(next));

            if !table {
                prose.push(header);
                at += 1;

                continue;
            }

            let columns = Self::cells(header);

            if columns.is_empty() {
                prose.push(header);
                at += 1;

                continue;
            }

            let mut rows = Vec::new();
            let mut row = at + 2;

            while row < lines.len() && Self::is_row(lines[row]) && rows.len() < Self::MAX_ROWS {
                rows.push(Self::fit(Self::cells(lines[row]), columns.len()));
                row += 1;
            }

            if !prose.is_empty() {
                segments.push(Segment::Prose(prose.join("\n")));
                prose.clear();
            }

            segments.push(Segment::Table {
                columns,
                rows,
                source: lines[at..row].join("\n"),
            });

            at = row;
        }

        if !prose.is_empty() {
            segments.push(Segment::Prose(prose.join("\n")));
        }

        segments
    }

    /// A line that could be a row: it has a pipe and something else.
    fn is_row(line: &str) -> bool {
        line.contains('|') && line.trim().len() > 1
    }

    /// A delimiter row — `| --- | :--: |` and its variants.
    ///
    /// Every cell must be dashes with optional alignment colons. Judged cell by
    /// cell rather than over the whole line, so a row whose text merely contains
    /// a dash is not mistaken for one.
    fn is_delimiter(line: &str) -> bool {
        if !line.contains('-') {
            return false;
        }

        let cells = Self::cells(line);

        !cells.is_empty() && cells.iter().all(|cell| Self::is_dashes(cell))
    }

    fn is_dashes(cell: &str) -> bool {
        let body = cell.trim_start_matches(':').trim_end_matches(':');

        !body.is_empty() && body.chars().all(|character| character == '-')
    }

    /// The cells of a row, with the outer pipes dropped.
    ///
    /// Scanned rather than split, because `\|` is an escaped pipe inside a cell
    /// and must not end it.
    fn cells(line: &str) -> Vec<String> {
        let mut cells = Vec::new();
        let mut cell = String::new();
        let mut characters = line.chars().peekable();

        while let Some(character) = characters.next() {
            if character == '\\' && characters.peek() == Some(&'|') {
                characters.next();
                cell.push('|');

                continue;
            }

            if character == '|' {
                cells.push(std::mem::take(&mut cell));

                continue;
            }

            cell.push(character);
        }

        cells.push(cell);

        // A row is written `| a | b |`, so the pipe at each end leaves an empty
        // cell that was never a column. One at each end only: an empty cell in
        // the middle is a column somebody left blank.
        if cells.first().is_some_and(|cell| cell.trim().is_empty()) {
            cells.remove(0);
        }

        if cells.last().is_some_and(|cell| cell.trim().is_empty()) {
            cells.pop();
        }

        cells.iter().map(|cell| Self::flatten(cell.trim())).collect()
    }

    /// A cell as the words it renders as.
    ///
    /// The client draws a cell as text, so a cell that kept its backticks would
    /// show them — `` `pane.updated` `` complete with the marks. Only inline
    /// code is unwrapped, because it is what agents put in table cells and
    /// because stripping more would start mangling content that meant it.
    fn flatten(cell: &str) -> String {
        cell.replace('`', "")
    }

    /// A row padded or trimmed to the header's width, so the table stays square.
    fn fit(mut cells: Vec<String>, width: usize) -> Vec<String> {
        cells.truncate(width);
        cells.resize(width, String::new());

        cells
    }
}
