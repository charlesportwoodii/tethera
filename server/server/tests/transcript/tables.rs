use tethera_server_lib::transcript::{MarkdownTables, Segment};

fn kinds(segments: &[Segment]) -> Vec<&'static str> {
    segments
        .iter()
        .map(|segment| match segment {
            Segment::Prose(_) => "prose",
            Segment::Table { .. } => "table",
        })
        .collect()
}

fn first_table(source: &str) -> (Vec<String>, Vec<Vec<String>>) {
    for segment in MarkdownTables::split(source) {
        if let Segment::Table { columns, rows, .. } = segment {
            return (columns, rows);
        }
    }

    panic!("no table in {source:?}");
}

#[test]
fn a_table_is_lifted_out_of_the_prose_around_it() {
    let source =
        "Here is what happened:\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\nAnd that is all.";

    assert_eq!(kinds(&MarkdownTables::split(source)), vec!["prose", "table", "prose"]);
}

// The exact table an agent wrote, taken off a real session file rather than
// invented — the shape that reached a phone as one unreadable sentence.
#[test]
fn the_header_and_rows_are_read_off_a_real_table() {
    let source = "| Mechanism | Result |\n| --- | --- |\n| `pane.updated` | never fires on output |";

    let (columns, rows) = first_table(source);

    assert_eq!(columns, vec!["Mechanism", "Result"]);
    assert_eq!(rows, vec![vec!["pane.updated", "never fires on output"]]);
}

// **The guard that matters.** Prose is full of pipes — a shell pipeline, a regex
// alternation — and only the dashes underneath make the line above a header.
// Without this a sentence becomes a one-column table and its words disappear
// into cells.
#[test]
fn a_line_of_pipes_with_no_delimiter_under_it_stays_prose() {
    let source = "Run `herdr pane list | grep w7X` and read what it says.";

    assert_eq!(kinds(&MarkdownTables::split(source)), vec!["prose"]);
}

#[test]
fn a_row_of_ordinary_dashes_is_not_a_delimiter() {
    let source = "| a | b |\n| well - no | still - no |\n";

    assert_eq!(kinds(&MarkdownTables::split(source)), vec!["prose"]);
}

#[test]
fn the_alignment_forms_of_a_delimiter_are_accepted() {
    let source = "| A | B | C |\n|:---|:---:|---:|\n| 1 | 2 | 3 |";

    assert_eq!(kinds(&MarkdownTables::split(source)), vec!["table"]);
}

// An escaped pipe is content. Ending a cell on it would split one column into
// two and shift every cell after it under the wrong header.
#[test]
fn an_escaped_pipe_stays_inside_its_cell() {
    let source = "| Pattern | Means |\n| --- | --- |\n| a \\| b | either |";

    let (_, rows) = first_table(source);

    assert_eq!(rows, vec![vec!["a | b", "either"]]);
}

#[test]
fn a_short_row_is_padded_so_the_table_stays_square() {
    let source = "| A | B | C |\n| --- | --- | --- |\n| only one |";

    let (_, rows) = first_table(source);

    assert_eq!(rows, vec![vec!["only one", "", ""]]);
}

// The client draws a cell as text, so a cell that kept its backticks would show
// them.
#[test]
fn inline_code_is_unwrapped_in_a_cell() {
    let source = "| Call | Note |\n| --- | --- |\n| `pane.updated` | fires |";

    let (_, rows) = first_table(source);

    assert_eq!(rows[0][0], "pane.updated");
}

#[test]
fn an_empty_cell_in_the_middle_is_kept_as_a_column() {
    let source = "| A | B | C |\n| --- | --- | --- |\n| 1 |  | 3 |";

    let (_, rows) = first_table(source);

    assert_eq!(rows, vec![vec!["1", "", "3"]]);
}

// A peer that does not know the variant is sent the table as the text it always
// was. postcard encodes variants by index, so there is no recovering one it has
// never heard of — the sender has to carry the words itself.
#[test]
fn a_table_carries_the_markdown_it_came_from() {
    let source = "| A |\n| --- |\n| 1 |";

    let segments = MarkdownTables::split(source);

    let Segment::Table { source: carried, .. } = &segments[0] else {
        panic!("expected a table");
    };

    assert_eq!(carried, source);
}
