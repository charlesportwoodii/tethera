import { inlineText, parseInline } from "$console";

/** A run of source that renders one way. */
export type MarkdownSegment =
  | { kind: "markdown"; text: string }
  | { kind: "table"; columns: string[]; rows: string[][] };

/**
 * Pulls GitHub-flavoured tables out of markdown so they can be drawn as tables.
 *
 * The console parser has no table block, so a table reaches the screen as a
 * paragraph: the pipes and dashes render as themselves and the newlines
 * collapse, which turns a five-row comparison into one unreadable sentence.
 * Agents write tables constantly, so this is most of what a long answer looks
 * like on a phone.
 *
 * Only the *finding* of a table happens here. The prose around it still goes
 * through the console renderer and the table itself through `TableView`,
 * because a second markdown renderer beside theirs is the duplication their own
 * code warns about.
 */
export class MarkdownTables {
  /**
   * Rows a table may have before it is left as prose.
   *
   * A guard against a paragraph of pipes rather than a real limit; no answer
   * carries a table this long, and drawing one would cost more than reading it.
   */
  private static readonly MAX_ROWS = 200;

  /** Whether a source is worth splitting at all. */
  static has(source: string): boolean {
    return source.includes("|") && this.split(source).some((part) => part.kind === "table");
  }

  /**
   * The source in order, with each table lifted into its own segment.
   *
   * A table is a header row, a delimiter row of dashes directly under it, and
   * the rows that follow while they still look like rows. The delimiter is what
   * decides: a line of pipes on its own is a sentence about pipes, and only the
   * dashes under it make the line above a header.
   */
  static split(source: string): MarkdownSegment[] {
    const lines = source.split("\n");
    const segments: MarkdownSegment[] = [];
    let prose: string[] = [];
    let at = 0;

    const flush = () => {
      if (prose.length > 0) {
        segments.push({ kind: "markdown", text: prose.join("\n") });
        prose = [];
      }
    };

    while (at < lines.length) {
      const header = lines[at];
      const delimiter = lines[at + 1];

      if (delimiter === undefined || !this.isRow(header) || !this.isDelimiter(delimiter)) {
        prose.push(header);
        at += 1;

        continue;
      }

      const columns = this.cells(header);

      if (columns.length === 0) {
        prose.push(header);
        at += 1;

        continue;
      }

      const rows: string[][] = [];
      let row = at + 2;

      while (row < lines.length && this.isRow(lines[row]) && rows.length < this.MAX_ROWS) {
        rows.push(this.fit(this.cells(lines[row]), columns.length));
        row += 1;
      }

      flush();
      segments.push({ kind: "table", columns, rows });
      at = row;
    }

    flush();

    return segments;
  }

  /** A line that could be a row: it has a pipe and something else. */
  private static isRow(line: string): boolean {
    return line.includes("|") && line.trim().length > 1;
  }

  /**
   * A delimiter row — `| --- | :--: |` and its variants.
   *
   * Every cell must be dashes with optional alignment colons. Checking the
   * cells rather than matching the whole line in one expression keeps a row
   * whose text merely contains a dash from being read as a delimiter.
   */
  private static isDelimiter(line: string): boolean {
    if (!line.includes("-")) {
      return false;
    }

    const cells = this.cells(line);

    return cells.length > 0 && cells.every((cell) => /^:?-+:?$/.test(cell));
  }

  /**
   * The cells of a row, with the outer pipes dropped.
   *
   * Scanned rather than split on a pattern: `\|` is an escaped pipe inside a
   * cell and must not end it, and the lookbehind that would express that in one
   * expression is not available on every webview this ships to.
   */
  private static cells(line: string): string[] {
    const cells: string[] = [];
    let cell = "";

    for (let at = 0; at < line.length; at += 1) {
      const character = line[at];

      if (character === "\\" && line[at + 1] === "|") {
        cell += "|";
        at += 1;

        continue;
      }

      if (character === "|") {
        cells.push(cell);
        cell = "";

        continue;
      }

      cell += character;
    }

    cells.push(cell);

    // A row is written `| a | b |`, so the pipes at each end produce an empty
    // cell that was never a column. One at each end only: an empty cell in the
    // middle is a column somebody left blank.
    if (cells.length > 0 && cells[0].trim() === "") {
      cells.shift();
    }

    if (cells.length > 0 && cells[cells.length - 1].trim() === "") {
      cells.pop();
    }

    return cells.map((text) => this.flatten(text.trim()));
  }

  /**
   * A cell as the words it renders as.
   *
   * `TableView` draws a cell as text, so inline markup would arrive with its
   * markers showing — `` `pane.updated` `` complete with backticks. Flattened
   * through the console parser rather than by stripping characters here, so a
   * cell keeps whatever that parser already understands.
   */
  private static flatten(text: string): string {
    if (text === "") {
      return text;
    }

    return inlineText(parseInline(text));
  }

  /** A row padded or trimmed to the header's width, so the table stays square. */
  private static fit(cells: string[], width: number): string[] {
    const row = cells.slice(0, width);

    while (row.length < width) {
      row.push("");
    }

    return row;
  }
}
