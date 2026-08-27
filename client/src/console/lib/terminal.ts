import type { CloseReason } from "$bindings/CloseReason";
import type { Color } from "$bindings/Color";
import type { CursorState } from "$bindings/CursorState";
import type { RowUpdate } from "$bindings/RowUpdate";
import type { Style } from "$bindings/Style";
import type { TerminalFrame } from "$bindings/TerminalFrame";

/**
 * A pane's screen, and the frames that change it.
 *
 * A class rather than component state on purpose: every other component in this
 * library is a pure function of its props, and a terminal is not — `damage`
 * frames only make sense against what came before. Keeping the grid here means
 * damage application is unit-testable without a DOM, and `TerminalView` stays a
 * renderer.
 *
 * Two things the wire is strict about and this honours:
 *
 * - **A style table belongs to the frame it arrived in.** Cells therefore store
 *   the resolved style, never an index. Carrying an index across frames reads
 *   the next frame's table and colours the screen at random.
 * - **Columns are cell columns.** A double-width glyph occupies two and the
 *   server emits no spacer, so writing one has to claim the second column or
 *   every later column on that row is off by one.
 */

/** Attribute bits on `Style.attrs`. */
export const ATTR = {
  bold: 1,
  dim: 2,
  italic: 4,
  underline: 8,
  blink: 16,
  reverse: 32,
  hidden: 64,
  strike: 128,
} as const;

export interface Cell {
  /** Empty string for the second column of a double-width glyph. */
  char: string;
  style: Style;
  /** True where this cell is the tail of a wide glyph and draws nothing. */
  continuation: boolean;
}

/** A run of cells sharing one style, ready to render. */
export interface Run {
  text: string;
  style: Style;
}

export const DEFAULT_STYLE: Style = { fg: "default", bg: "default", attrs: 0 };

/**
 * Display width of one code point.
 *
 * Deliberately coarse: the ranges below are the ones that actually appear in
 * terminal output — CJK, Hangul, and the emoji planes. Getting this wrong shifts
 * a row, so it is better to cover the common cases exactly than to approximate
 * the whole of Unicode.
 */
export function charWidth(char: string): number {
  const code = char.codePointAt(0);
  if (code === undefined) return 0;
  // Combining marks attach to the previous cell rather than taking one.
  if (code >= 0x0300 && code <= 0x036f) return 0;
  if (
    (code >= 0x1100 && code <= 0x115f) ||
    (code >= 0x2e80 && code <= 0xa4cf) ||
    (code >= 0xac00 && code <= 0xd7a3) ||
    (code >= 0xf900 && code <= 0xfaff) ||
    (code >= 0xfe30 && code <= 0xfe6f) ||
    (code >= 0xff00 && code <= 0xff60) ||
    (code >= 0xffe0 && code <= 0xffe6) ||
    (code >= 0x1f300 && code <= 0x1f64f) ||
    (code >= 0x1f900 && code <= 0x1f9ff) ||
    (code >= 0x20000 && code <= 0x3fffd)
  ) {
    return 2;
  }
  return 1;
}

export class TerminalGrid {
  cols = 0;
  rows = 0;
  cursor: CursorState | null = null;
  altScreen = false;
  /** Null when the pane owns the alternate screen. Not zero. */
  scrollbackLen: number | null = null;
  closed: CloseReason | null = null;
  /** Increments on every bell, so a caller can react without a callback. */
  bells = 0;

  private cells: Cell[][] = [];

  constructor(cols = 0, rows = 0) {
    this.resize(cols, rows);
  }

  /** The screen as runs, one array per row. */
  lines(): Run[][] {
    return this.cells.map((row) => toRuns(row));
  }

  /** One row as runs. */
  line(y: number): Run[] {
    const row = this.cells[y];
    return row === undefined ? [] : toRuns(row);
  }

  /**
   * Apply one frame.
   *
   * Returns false for a frame that changed nothing renderable — `bell` — so a
   * caller can skip a repaint it does not need.
   */
  apply(frame: TerminalFrame): boolean {
    if (frame === "bell") {
      this.bells += 1;
      return false;
    }

    if ("snapshot" in frame) {
      const snap = frame.snapshot;
      // A snapshot is the whole truth: everything before it is discarded rather
      // than merged, which is what makes reattaching after a gap correct.
      // reset, not resize — resize preserves what still fits, which is right for
      // a 'resized' frame and wrong here: a snapshot of the same size would keep
      // the tail of the previous screen.
      this.reset(snap.cols, snap.rows);
      this.write(snap.rows_data, snap.styles);
      this.cursor = snap.cursor;
      this.altScreen = snap.alt_screen;
      this.scrollbackLen = snap.scrollback_len;
      this.closed = null;
      return true;
    }

    if ("damage" in frame) {
      this.write(frame.damage.rows_data, frame.damage.styles);
      // Absent means unchanged, not hidden.
      if (frame.damage.cursor !== null) {
        this.cursor = frame.damage.cursor;
      }
      return true;
    }

    if ("resized" in frame) {
      this.resize(frame.resized.cols, frame.resized.rows);
      return true;
    }

    if ("closed" in frame) {
      this.closed = frame.closed.reason;
      return true;
    }

    return false;
  }

  /** Blank the screen at a given size. */
  reset(cols: number, rows: number): void {
    this.cells = [];
    this.resize(cols, rows);
    for (const row of this.cells) {
      for (let x = 0; x < row.length; x += 1) {
        row[x] = blank();
      }
    }
  }

  /**
   * Change dimensions, keeping what still fits.
   *
   * A resize is observed rather than requested, so it arrives with content
   * already on screen; clearing would blank a pane that only got narrower.
   */
  resize(cols: number, rows: number): void {
    const width = Math.max(0, Math.floor(cols));
    const height = Math.max(0, Math.floor(rows));
    const next: Cell[][] = [];

    for (let y = 0; y < height; y += 1) {
      const old = this.cells[y];
      const row: Cell[] = [];
      for (let x = 0; x < width; x += 1) {
        row.push(old?.[x] ?? blank());
      }
      next.push(row);
    }

    this.cells = next;
    this.cols = width;
    this.rows = height;

    // A cursor left outside the new grid would draw off the edge.
    if (this.cursor !== null && (this.cursor.x >= width || this.cursor.y >= height)) {
      this.cursor = {
        ...this.cursor,
        x: Math.min(this.cursor.x, Math.max(0, width - 1)),
        y: Math.min(this.cursor.y, Math.max(0, height - 1)),
      };
    }
  }

  private write(updates: RowUpdate[], styles: Style[]): void {
    for (const update of updates) {
      const row = this.cells[update.y];
      if (row === undefined) continue;

      let x = update.from_x;
      for (const span of update.spans) {
        // Out of range indexes the frame's own table, so a bad index is a server
        // bug; falling back to the default style keeps the screen readable
        // instead of throwing away the frame.
        const style = styles[span.style] ?? DEFAULT_STYLE;

        for (const char of Array.from(span.text)) {
          if (x >= row.length) break;
          const width = charWidth(char);

          if (width === 0) {
            // A combining mark joins the cell before it rather than taking one.
            const previous = row[x - 1];
            if (previous !== undefined) {
              previous.char += char;
            }
            continue;
          }

          row[x] = { char, style, continuation: false };
          x += 1;

          if (width === 2 && x < row.length) {
            // Claim the second column. Without this every later column on the
            // row is shifted by one.
            row[x] = { char: "", style, continuation: true };
            x += 1;
          }
        }
      }
    }
  }
}

function blank(): Cell {
  return { char: " ", style: DEFAULT_STYLE, continuation: false };
}

function sameStyle(a: Style, b: Style): boolean {
  return a === b || (sameColor(a.fg, b.fg) && sameColor(a.bg, b.bg) && a.attrs === b.attrs);
}

function sameColor(a: Color, b: Color): boolean {
  if (a === b) return true;
  if (typeof a === "string" || typeof b === "string") return false;
  if ("indexed" in a && "indexed" in b) return a.indexed === b.indexed;
  if ("rgb" in a && "rgb" in b) {
    return a.rgb[0] === b.rgb[0] && a.rgb[1] === b.rgb[1] && a.rgb[2] === b.rgb[2];
  }
  return false;
}

/**
 * Merge a row into runs.
 *
 * One element per style change rather than per cell: a full row of 80 cells
 * becomes two or three nodes, which is the difference between a repaint a phone
 * can do at speed and one it cannot.
 */
export function toRuns(row: Cell[]): Run[] {
  const runs: Run[] = [];
  for (const cell of row) {
    // A continuation draws nothing: its glyph was written in the cell before it.
    if (cell.continuation) continue;
    const last = runs[runs.length - 1];
    if (last !== undefined && sameStyle(last.style, cell.style)) {
      last.text += cell.char;
      continue;
    }
    runs.push({ text: cell.char, style: cell.style });
  }
  return runs;
}
