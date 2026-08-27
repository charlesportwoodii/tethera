import { describe, expect, test } from "vitest";
import { MarkdownTables } from "./tables";

describe("MarkdownTables", () => {
  test("lifts a table out of the prose around it", () => {
    const source = ["Here is what happened:", "", "| A | B |", "| --- | --- |", "| 1 | 2 |", "", "And that is all."].join("\n");

    const segments = MarkdownTables.split(source);

    expect(segments.map((part) => part.kind)).toEqual(["markdown", "table", "markdown"]);
  });

  test("reads the header and the rows", () => {
    const source = "| Mechanism | Result |\n| --- | --- |\n| pane.updated | never fires |";

    const [table] = MarkdownTables.split(source);

    expect(table).toEqual({
      kind: "table",
      columns: ["Mechanism", "Result"],
      rows: [["pane.updated", "never fires"]],
    });
  });

  // The guard that matters. Prose is full of pipes — a shell command, a regex
  // alternation — and only the dashes underneath make the line above a header.
  // Without this, a sentence becomes a one-column table and its words vanish
  // into cells.
  test("leaves a line of pipes alone when no delimiter follows it", () => {
    const source = "Run `herdr pane list | grep w7X` and read what it says.";

    expect(MarkdownTables.has(source)).toBe(false);
    expect(MarkdownTables.split(source)).toEqual([{ kind: "markdown", text: source }]);
  });

  test("does not read a row of ordinary dashes as a delimiter", () => {
    const source = "| a | b |\n| well - no | still - no |\n";

    expect(MarkdownTables.has(source)).toBe(false);
  });

  test("accepts the alignment forms of a delimiter", () => {
    const source = "| A | B | C |\n|:---|:---:|---:|\n| 1 | 2 | 3 |";

    const [table] = MarkdownTables.split(source);

    expect(table.kind).toBe("table");
  });

  // An escaped pipe is content. Ending a cell on it would split one column into
  // two and shift every cell after it into the wrong header.
  test("an escaped pipe stays inside its cell", () => {
    const source = "| Pattern | Means |\n| --- | --- |\n| a \\| b | either |";

    const [table] = MarkdownTables.split(source);

    expect(table).toMatchObject({ rows: [["a | b", "either"]] });
  });

  test("a short row is padded so the table stays square", () => {
    const source = "| A | B | C |\n| --- | --- | --- |\n| only one |";

    const [table] = MarkdownTables.split(source);

    expect(table).toMatchObject({ rows: [["only one", "", ""]] });
  });

  // TableView draws a cell as text, so a cell that kept its markers would show
  // them: `pane.updated` complete with backticks.
  test("inline markers are flattened out of cells", () => {
    const source = "| Call | Note |\n| --- | --- |\n| `pane.updated` | **never** fires |";

    const [table] = MarkdownTables.split(source);

    expect(table).toMatchObject({ rows: [["pane.updated", "never fires"]] });
  });

  test("an empty cell in the middle is kept as a column", () => {
    const source = "| A | B | C |\n| --- | --- | --- |\n| 1 |  | 3 |";

    const [table] = MarkdownTables.split(source);

    expect(table).toMatchObject({ rows: [["1", "", "3"]] });
  });

  test("a table that runs to the end of the source needs nothing after it", () => {
    const source = "| A |\n| --- |\n| 1 |";

    expect(MarkdownTables.split(source).map((part) => part.kind)).toEqual(["table"]);
  });
});
