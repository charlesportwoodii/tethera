import type { Part } from "$bindings/Part";
import type { ToolStatus } from "$bindings/ToolStatus";
import type { Turn } from "$bindings/Turn";

/** Consecutive agent turns that did work without saying anything. */
export interface ActivityRun {
  /** In the order they happened. */
  turns: Turn[];
  /** Whether the agent is still adding to this run. */
  live: boolean;
}

/** One entry on the transcript timeline: a turn, or a fold over several. */
export type TimelineRow =
  | { kind: "turn"; key: string; turn: Turn }
  | { kind: "activity"; key: string; run: ActivityRun };

/**
 * Folds an agent's tool calls into one row so its words sit next to its words.
 *
 * The harness writes one response per tool call, so every call is its own turn
 * with its own timestamp. A run that reads eleven files draws eleven dated rows
 * and pushes the sentence before them off the screen — which is what makes an
 * agent look like it says nothing until it stops.
 *
 * Only turns that said nothing fold. Anything a person has to read or act on -
 * words, a question, a file, a table - keeps its own row, because a fold is a
 * tap and the point is to remove taps rather than move them.
 */
export class Activity {
  /**
   * The shortest run worth folding.
   *
   * A fold costs a tap and draws a row of its own, so collapsing two steps
   * saves nothing and hides them anyway.
   */
  static readonly MIN_RUN = 3;

  /** How many tool names the fold lists before it counts the rest. */
  private static readonly NAMED = 3;

  /**
   * Part kinds that are the agent working rather than the agent talking.
   *
   * `unknown` is here on purpose: it is a part this client is too old to draw,
   * and its fallback text is the raw source rows. A version-mismatched client
   * folds those away instead of drawing a wall of them.
   */
  private static readonly WORK = ["tool_use", "diff", "todo", "status", "unknown"];

  /**
   * The timeline, with each run of work folded into one row.
   *
   * `working` marks the last run live. Only the last one can be: a run with
   * anything after it has already ended, whatever the agent is doing now.
   */
  static rows(turns: Turn[], working: boolean): TimelineRow[] {
    const rows: TimelineRow[] = [];
    let run: Turn[] = [];

    const flush = () => {
      if (run.length === 0) {
        return;
      }

      if (run.length < this.MIN_RUN) {
        for (const turn of run) {
          rows.push({ kind: "turn", key: turn.id, turn });
        }
      } else {
        rows.push({ kind: "activity", key: `run:${run[0].id}`, run: { turns: run, live: false } });
      }

      run = [];
    };

    for (const turn of turns) {
      if (this.isWork(turn)) {
        run = [...run, turn];

        continue;
      }

      flush();
      rows.push({ kind: "turn", key: turn.id, turn });
    }

    flush();

    const last = rows[rows.length - 1];

    if (!working || last === undefined || last.kind !== "activity") {
      return rows;
    }

    return [...rows.slice(0, -1), { ...last, run: { ...last.run, live: true } }];
  }

  /** Whether a turn is the agent working and nothing else. */
  static isWork(turn: Turn): boolean {
    if (turn.role !== "agent" || turn.parts.length === 0) {
      return false;
    }

    return turn.parts.every((part) => this.WORK.includes(this.kind(part)));
  }

  /** Whether any tool in this turn came back failed. */
  static failed(turn: Turn): boolean {
    return turn.parts.some((part) => "tool_use" in part && part.tool_use.status === "failed");
  }

  /**
   * The steps a collapsed run still draws.
   *
   * A failure, always: it is the one step somebody needs without opening
   * anything. The newest step while the run is live, so a working agent shows
   * what it is doing now — and only while it is live, so the row folds away
   * when the turn ends rather than leaving a finished call sitting there.
   */
  static shown(run: ActivityRun, expanded: boolean): Turn[] {
    if (expanded) {
      return run.turns;
    }

    const newest = run.turns.length - 1;

    return run.turns.filter((turn, at) => this.failed(turn) || (run.live && at === newest));
  }

  static label(run: ActivityRun): string {
    return run.turns.length === 1 ? "1 step" : `${run.turns.length} steps`;
  }

  /**
   * The line beside the count.
   *
   * A fold that reads the same whatever is inside makes somebody open every
   * one, so it names what ran — and says how much of it broke instead, because
   * that is the reason to open this fold rather than the next.
   */
  static detail(run: ActivityRun): string | null {
    const failures = run.turns.filter((turn) => this.failed(turn)).length;

    if (failures > 0) {
      return failures === 1 ? "1 failed" : `${failures} failed`;
    }

    const names = this.tools(run);

    if (names.length === 0) {
      return null;
    }

    const head = names.slice(0, this.NAMED);

    return names.length > head.length
      ? `${head.join(", ")} +${names.length - head.length}`
      : head.join(", ");
  }

  static status(run: ActivityRun): ToolStatus {
    if (run.turns.some((turn) => this.failed(turn))) {
      return "failed";
    }

    return run.live ? "running" : "ok";
  }

  /** The turn a row opens on, which is where a date header is decided. */
  static leading(row: TimelineRow): Turn {
    return row.kind === "turn" ? row.turn : row.run.turns[0];
  }

  /** The turn a row closes on, which the next row's date header is measured against. */
  static trailing(row: TimelineRow): Turn {
    return row.kind === "turn" ? row.turn : row.run.turns[row.run.turns.length - 1];
  }

  /** The distinct tool names in a run, first use first. */
  private static tools(run: ActivityRun): string[] {
    const names: string[] = [];

    for (const turn of run.turns) {
      for (const part of turn.parts) {
        if ("tool_use" in part && !names.includes(part.tool_use.name)) {
          names.push(part.tool_use.name);
        }
      }
    }

    return names;
  }

  /** Which variant a part is, as the wire spells it. */
  private static kind(part: Part): string {
    return Object.keys(part)[0] ?? "";
  }
}
