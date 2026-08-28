import { describe, expect, test } from "vitest";
import { Activity } from "./activity";
import type { Part } from "$bindings/Part";
import type { Role } from "$bindings/Role";
import type { ToolStatus } from "$bindings/ToolStatus";
import type { Turn } from "$bindings/Turn";

let sequence = 0;

function turn(role: Role, parts: Part[]): Turn {
  sequence += 1;

  return {
    cursor: `c${sequence}`,
    id: `t${sequence}`,
    at: 1_700_000_000_000 + sequence * 1_000,
    role,
    parts,
  };
}

function tool(name: string, status: ToolStatus = "ok"): Turn {
  return turn("agent", [
    { tool_use: { name, input: "", result: null, status, fallback_text: "" } },
  ]);
}

function said(text: string): Turn {
  return turn("agent", [{ text: { text } }]);
}

function asked(): Turn {
  return turn("agent", [
    {
      question: {
        question: { id: "q1", fingerprint: "f1", asks: [] } as never,
        answered: null,
        fallback_text: "",
      },
    },
  ]);
}

function run(rows: ReturnType<typeof Activity.rows>, at: number) {
  const row = rows[at];

  if (row.kind !== "activity") {
    throw new Error(`row ${at} is a ${row.kind}, not an activity run`);
  }

  return row.run;
}

describe("Activity", () => {
  test("folds a run of consecutive working turns into one row", () => {
    const turns = [said("here goes"), tool("Bash"), tool("Read"), tool("Grep"), said("done")];

    const rows = Activity.rows(turns, false);

    expect(rows.map((row) => row.kind)).toEqual(["turn", "activity", "turn"]);
    expect(run(rows, 1).turns).toHaveLength(3);
  });

  // The whole point of the fold: prose lands next to prose instead of a screen
  // of chevrons between two sentences.
  test("puts two prose turns beside each other once the work between them is folded", () => {
    const turns = [said("first"), tool("Bash"), tool("Read"), tool("Grep"), said("second")];

    const rows = Activity.rows(turns, false);

    expect(rows[0].kind).toBe("turn");
    expect(rows[2].kind).toBe("turn");
  });

  // A fold costs a tap. Below the threshold it saves fewer rows than it costs,
  // so the steps stay where they are.
  test("leaves a short run drawn as ordinary turns", () => {
    const rows = Activity.rows([tool("Bash"), tool("Read")], false);

    expect(rows.map((row) => row.kind)).toEqual(["turn", "turn"]);
  });

  // The thing a person has to act on. Hiding it behind a fold makes a blocked
  // agent look idle.
  test("never folds a turn carrying a question", () => {
    const rows = Activity.rows([tool("Bash"), asked(), tool("Read"), tool("Grep")], false);

    expect(rows.map((row) => row.kind)).toEqual(["turn", "turn", "turn", "turn"]);
  });

  test("does not fold a turn that says something alongside its tool call", () => {
    const mixed = turn("agent", [
      { text: { text: "checking" } },
      { tool_use: { name: "Bash", input: "", result: null, status: "ok", fallback_text: "" } },
    ]);

    const rows = Activity.rows([tool("Read"), mixed, tool("Grep"), tool("Glob"), tool("Bash")], false);

    expect(rows.map((row) => row.kind)).toEqual(["turn", "turn", "activity"]);
  });

  test("never folds what a person said", () => {
    const rows = Activity.rows([turn("operator", [{ text: { text: "go" } }])], false);

    expect(rows.map((row) => row.kind)).toEqual(["turn"]);
  });

  test("only the last run is live, and only while the agent is working", () => {
    const turns = [tool("A"), tool("B"), tool("C"), said("a word"), tool("D"), tool("E"), tool("F")];

    const working = Activity.rows(turns, true);

    expect(run(working, 0).live).toBe(false);
    expect(run(working, 2).live).toBe(true);

    const stopped = Activity.rows(turns, false);

    expect(run(stopped, 2).live).toBe(false);
  });

  // Folding away the step in flight would leave a working agent showing one
  // static row, which reads as nothing happening.
  test("a live run shows its newest step and a finished run shows none", () => {
    const turns = [tool("A"), tool("B"), tool("C")];

    const live = run(Activity.rows(turns, true), 0);

    expect(Activity.shown(live, false).map((step) => step.id)).toEqual([turns[2].id]);

    const settled = run(Activity.rows(turns, false), 0);

    expect(Activity.shown(settled, false)).toEqual([]);
  });

  // A failure is the one step somebody needs without opening anything.
  test("keeps a failed step visible in a collapsed run", () => {
    const turns = [tool("A"), tool("B", "failed"), tool("C")];

    const folded = run(Activity.rows(turns, false), 0);

    expect(Activity.shown(folded, false).map((step) => step.id)).toEqual([turns[1].id]);
  });

  test("expanding shows every step", () => {
    const turns = [tool("A"), tool("B"), tool("C")];

    const folded = run(Activity.rows(turns, false), 0);

    expect(Activity.shown(folded, true)).toHaveLength(3);
  });

  // A fold that reads the same whatever is inside makes somebody open every one.
  test("names the tools in the fold, and counts the failures instead when there are any", () => {
    const named = run(Activity.rows([tool("Bash"), tool("Read"), tool("Bash")], false), 0);

    expect(Activity.label(named)).toBe("3 steps");
    expect(Activity.detail(named)).toBe("Bash, Read");

    const broken = run(Activity.rows([tool("Bash"), tool("Read", "failed"), tool("Bash")], false), 0);

    expect(Activity.detail(broken)).toBe("1 failed");
  });

  test("counts the tools it did not name", () => {
    const many = run(Activity.rows([tool("A"), tool("B"), tool("C"), tool("D"), tool("E")], false), 0);

    expect(Activity.detail(many)).toBe("A, B, C +2");
  });

  test("a live fold spins, and a failed one says so", () => {
    const turns = [tool("A"), tool("B"), tool("C")];

    expect(Activity.status(run(Activity.rows(turns, true), 0))).toBe("running");
    expect(Activity.status(run(Activity.rows(turns, false), 0))).toBe("ok");

    const broken = [tool("A"), tool("B", "failed"), tool("C")];

    expect(Activity.status(run(Activity.rows(broken, true), 0))).toBe("failed");
  });

  // The date header is drawn between rows, so it needs the turn each row starts
  // and ends on rather than the flat list it came from.
  test("reports the turn a row opens and closes on", () => {
    const turns = [said("hello"), tool("A"), tool("B"), tool("C")];

    const rows = Activity.rows(turns, false);

    expect(Activity.leading(rows[0]).id).toBe(turns[0].id);
    expect(Activity.trailing(rows[0]).id).toBe(turns[0].id);
    expect(Activity.leading(rows[1]).id).toBe(turns[1].id);
    expect(Activity.trailing(rows[1]).id).toBe(turns[3].id);
  });
});
