import { describe, expect, it } from "vitest";
import { ATTR, charWidth, TerminalGrid, toRuns } from "./terminal";
import { cssColor, indexedColor, runStyle } from "./terminal-color";
import type { Style } from "$bindings/Style";
import type { TerminalFrame } from "$bindings/TerminalFrame";

/** The snapshot variant alone. TerminalFrame also includes the bare string "bell". */
type Snapshot = Extract<TerminalFrame, { snapshot: unknown }>;

const plain: Style = { fg: "default", bg: "default", attrs: 0 };
const red: Style = { fg: { indexed: 1 }, bg: "default", attrs: 0 };
const bold: Style = { fg: "default", bg: "default", attrs: ATTR.bold };

/** A snapshot writing one row of text at column zero. */
function snapshot(text: string, styles: Style[] = [plain], cols = 20, rows = 3): Snapshot {
  return {
    snapshot: {
      cols,
      rows,
      styles,
      rows_data: [{ y: 0, from_x: 0, spans: [{ style: 0, text }] }],
      cursor: { x: text.length, y: 0, visible: true, shape: "block" },
      alt_screen: false,
      scrollback_len: 100,
    },
  };
}

const text = (grid: TerminalGrid, y: number) =>
  grid
    .line(y)
    .map((run) => run.text)
    .join("");

describe("TerminalGrid — snapshot", () => {
  it("takes its dimensions from the snapshot", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("hello", [plain], 20, 3));
    expect(grid.cols).toBe(20);
    expect(grid.rows).toBe(3);
  });

  it("writes the text and pads the rest of the row with blanks", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("hi", [plain], 5, 1));
    expect(text(grid, 0)).toBe("hi   ");
  });

  it("carries the cursor, alt screen and scrollback", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("hi"));
    expect(grid.cursor).toEqual({ x: 2, y: 0, visible: true, shape: "block" });
    expect(grid.altScreen).toBe(false);
    expect(grid.scrollbackLen).toBe(100);
  });

  it("discards everything before it rather than merging", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("first line here", [plain], 20, 3));
    grid.apply(snapshot("second", [plain], 20, 3));
    // Merging would leave the tail of the old line behind, which is exactly the
    // wrong thing after reattaching across a gap.
    expect(text(grid, 0).trimEnd()).toBe("second");
  });

  it("keeps a null scrollback null rather than turning it into zero", () => {
    const grid = new TerminalGrid();
    const frame = snapshot("x");
    frame.snapshot.scrollback_len = null;
    grid.apply(frame);
    expect(grid.scrollbackLen).toBeNull();
  });
});

describe("TerminalGrid — damage", () => {
  it("replaces only the cells it covers", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("abcdefgh", [plain], 8, 1));
    grid.apply({
      damage: {
        styles: [plain],
        rows_data: [{ y: 0, from_x: 2, spans: [{ style: 0, text: "XY" }] }],
        cursor: null,
      },
    });
    expect(text(grid, 0)).toBe("abXYefgh");
  });

  it("leaves the cursor alone when the frame does not carry one", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("abc"));
    const before = grid.cursor;
    grid.apply({
      damage: { styles: [plain], rows_data: [], cursor: null },
    });
    // Absent means unchanged, not hidden.
    expect(grid.cursor).toEqual(before);
  });

  it("moves the cursor when the frame does carry one", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("abc"));
    grid.apply({
      damage: {
        styles: [plain],
        rows_data: [],
        cursor: { x: 7, y: 1, visible: false, shape: "bar" },
      },
    });
    expect(grid.cursor).toEqual({ x: 7, y: 1, visible: false, shape: "bar" });
  });

  it("resolves a style against the frame it arrived in, not a later one", () => {
    const grid = new TerminalGrid();
    // Index 0 is plain in the snapshot and red in the damage frame. A grid
    // storing indices would recolour the snapshot's text when the next table
    // arrived.
    grid.apply(snapshot("aa", [plain], 4, 1));
    grid.apply({
      damage: {
        styles: [red],
        rows_data: [{ y: 0, from_x: 2, spans: [{ style: 0, text: "bb" }] }],
        cursor: null,
      },
    });
    const runs = grid.line(0);
    expect(runs[0].style.fg).toBe("default");
    expect(runs[1].style.fg).toEqual({ indexed: 1 });
  });

  it("ignores a row outside the grid rather than throwing", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("x", [plain], 4, 1));
    expect(() =>
      grid.apply({
        damage: {
          styles: [plain],
          rows_data: [{ y: 99, from_x: 0, spans: [{ style: 0, text: "y" }] }],
          cursor: null,
        },
      }),
    ).not.toThrow();
  });

  it("falls back to the default style for an index the table does not have", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("x", [plain], 4, 1));
    grid.apply({
      damage: {
        styles: [],
        rows_data: [{ y: 0, from_x: 0, spans: [{ style: 9, text: "z" }] }],
        cursor: null,
      },
    });
    // A bad index is a server bug; throwing away the frame is worse than drawing
    // it unstyled.
    expect(text(grid, 0)).toBe("z   ");
  });
});

describe("TerminalGrid — wide glyphs", () => {
  it("claims the second column of a double-width glyph", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("漢a", [plain], 5, 1));
    // The server emits no spacer, so without claiming the column every later
    // one on the row is shifted by one.
    expect(text(grid, 0)).toBe("漢a  ");
  });

  it("keeps later columns aligned after a wide glyph", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("漢字", [plain], 6, 1));
    grid.apply({
      damage: {
        styles: [plain],
        rows_data: [{ y: 0, from_x: 4, spans: [{ style: 0, text: "ok" }] }],
        cursor: null,
      },
    });
    expect(text(grid, 0)).toBe("漢字ok");
  });

  it("attaches a combining mark to the cell before it", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("éx", [plain], 4, 1));
    // A mark takes no column of its own.
    expect(text(grid, 0)).toBe("éx  ");
  });

  it("handles a surrogate pair as one glyph", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("\u{1F600}a", [plain], 5, 1));
    expect(text(grid, 0)).toBe("\u{1F600}a  ");
  });
});

describe("TerminalGrid — other frames", () => {
  it("counts bells without repainting", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("x"));
    expect(grid.apply("bell")).toBe(false);
    expect(grid.bells).toBe(1);
  });

  it("keeps what still fits on a resize", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("abcdefgh", [plain], 8, 2));
    grid.apply({ resized: { cols: 4, rows: 2 } });
    // A resize is observed, not requested: it arrives with content on screen and
    // clearing would blank a pane that only got narrower.
    expect(text(grid, 0)).toBe("abcd");
  });

  it("pulls a cursor left outside the new grid back inside it", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("abcdefgh", [plain], 8, 2));
    grid.apply({ resized: { cols: 3, rows: 1 } });
    expect(grid.cursor?.x).toBeLessThanOrEqual(2);
    expect(grid.cursor?.y).toBe(0);
  });

  it("records why a pane closed", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("x"));
    grid.apply({ closed: { reason: "exited" } });
    expect(grid.closed).toBe("exited");
  });

  it("clears a close when a new snapshot arrives", () => {
    const grid = new TerminalGrid();
    grid.apply({ closed: { reason: "pane_gone" } });
    grid.apply(snapshot("back"));
    expect(grid.closed).toBeNull();
  });
});

describe("toRuns", () => {
  it("merges cells that share a style", () => {
    const grid = new TerminalGrid();
    grid.apply(snapshot("aaaa", [plain], 4, 1));
    // One node per style change rather than per cell is the difference between a
    // repaint a phone can do at speed and one it cannot.
    expect(grid.line(0)).toHaveLength(1);
  });

  it("breaks a run where the style changes", () => {
    const grid = new TerminalGrid();
    grid.apply({
      snapshot: {
        cols: 4,
        rows: 1,
        styles: [plain, red],
        rows_data: [
          {
            y: 0,
            from_x: 0,
            spans: [
              { style: 0, text: "ab" },
              { style: 1, text: "cd" },
            ],
          },
        ],
        cursor: null,
        alt_screen: false,
        scrollback_len: 0,
      },
    });
    expect(grid.line(0).map((r) => r.text)).toEqual(["ab", "cd"]);
  });

  it("merges styles that are equal by value, not only by reference", () => {
    const runs = toRuns([
      { char: "a", style: { fg: "default", bg: "default", attrs: 0 }, continuation: false },
      { char: "b", style: { fg: "default", bg: "default", attrs: 0 }, continuation: false },
    ]);
    expect(runs).toHaveLength(1);
  });

  it("drops the continuation cell of a wide glyph", () => {
    const runs = toRuns([
      { char: "漢", style: plain, continuation: false },
      { char: "", style: plain, continuation: true },
    ]);
    expect(runs[0].text).toBe("漢");
  });
});

describe("charWidth", () => {
  it("counts a latin character as one", () => {
    expect(charWidth("a")).toBe(1);
  });

  it("counts CJK and emoji as two", () => {
    expect(charWidth("漢")).toBe(2);
    expect(charWidth("\u{1F600}")).toBe(2);
  });

  it("counts a combining mark as none", () => {
    expect(charWidth("́")).toBe(0);
  });
});

describe("colour", () => {
  it("leaves default as the pane's own colour", () => {
    expect(cssColor("default")).toBeNull();
  });

  it("reads the first sixteen slots from theme tokens", () => {
    // A pane should look like it belongs to the app, not like a screenshot of a
    // different program.
    expect(indexedColor(1)).toContain("--tc-term-red");
  });

  it("computes the 6x6x6 cube", () => {
    expect(indexedColor(16)).toBe("rgb(0 0 0)");
    expect(indexedColor(231)).toBe("rgb(255 255 255)");
  });

  it("computes the greyscale ramp", () => {
    expect(indexedColor(232)).toBe("rgb(8 8 8)");
    expect(indexedColor(255)).toBe("rgb(238 238 238)");
  });

  it("clamps an index outside the palette", () => {
    expect(indexedColor(-5)).toBe(indexedColor(0));
    expect(indexedColor(999)).toBe(indexedColor(255));
  });

  it("passes rgb through", () => {
    expect(cssColor({ rgb: [10, 20, 30] })).toBe("rgb(10 20 30)");
  });
});

describe("runStyle", () => {
  it("emits nothing for a plain run", () => {
    expect(runStyle(plain)).toEqual({ fg: null, bg: null, classes: "" });
  });

  it("names attributes as classes", () => {
    expect(runStyle(bold).classes).toBe("is-bold");
  });

  it("combines attributes", () => {
    const style: Style = {
      fg: "default",
      bg: "default",
      attrs: ATTR.bold | ATTR.underline | ATTR.strike,
    };
    expect(runStyle(style).classes).toBe("is-bold is-underline is-strike");
  });

  it("swaps the colours for reverse", () => {
    const style: Style = { fg: { rgb: [1, 2, 3] }, bg: { rgb: [4, 5, 6] }, attrs: ATTR.reverse };
    const { fg, bg } = runStyle(style);
    expect(fg).toBe("rgb(4 5 6)");
    expect(bg).toBe("rgb(1 2 3)");
  });

  it("falls back to the pane's ground and ink when one side of a reverse is unset", () => {
    const style: Style = { fg: { rgb: [1, 2, 3] }, bg: "default", attrs: ATTR.reverse };
    const { fg, bg } = runStyle(style);
    expect(fg).toBe("var(--tc-term-bg)");
    expect(bg).toBe("rgb(1 2 3)");
  });
});
