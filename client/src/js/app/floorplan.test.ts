import { describe, expect, test } from "vitest";
import { Floorplan } from "./floorplan";
import type { Pane } from "$bindings/Pane";
import type { PaneRect } from "$bindings/PaneRect";
import type { TabLayout } from "$bindings/TabLayout";

function aPane(id: string, over: Partial<Pane> = {}): Pane {
  return {
    id: id as unknown as Pane["id"],
    tab_id: "tb_1" as unknown as Pane["tab_id"],
    workspace_id: "ws_1" as unknown as Pane["workspace_id"],
    label: id,
    title: null,
    cwd: null,
    size: { cols: 80, rows: 24 },
    focused: false,
    foreground_command: null,
    conversation: null,
    agent: null,
    ...over,
  };
}

function aLayout(
  slots: Array<[string, PaneRect]>,
  zoomed: string | null = null,
): TabLayout {
  return {
    tab: "tb_1" as unknown as TabLayout["tab"],
    slots: slots.map(([pane, rect]) => ({
      pane: pane as unknown as TabLayout["slots"][number]["pane"],
      rect,
    })),
    zoomed: zoomed as unknown as TabLayout["zoomed"],
  };
}

function rect(x: number, y: number, width: number, height: number): PaneRect {
  return { x, y, width, height };
}

describe("Floorplan.place", () => {
  // The whole reason this returns a list rather than placing what it can. The
  // rects are normalised against the area they cover, so dropping one pane
  // stretches its neighbours over the gap and produces a map that looks right.
  test("a layout missing one of the tab's panes draws nothing", () => {
    const panes = [aPane("pn_a"), aPane("pn_b")];
    const layout = aLayout([["pn_a", rect(0, 0, 40, 20)]]);

    expect(Floorplan.place(panes, layout)).toEqual([]);
  });

  test("a slot naming a pane nobody listed draws nothing", () => {
    const panes = [aPane("pn_a"), aPane("pn_b")];
    const layout = aLayout([
      ["pn_a", rect(0, 0, 40, 20)],
      ["pn_ghost", rect(40, 0, 40, 20)],
    ]);

    expect(Floorplan.place(panes, layout)).toEqual([]);
  });

  test("no layout at all draws nothing", () => {
    expect(Floorplan.place([aPane("pn_a")], null)).toEqual([]);
  });

  // Real herdr output: a tab whose area starts at column 29, because the desk's
  // window has a sidebar. Normalising against the raw origin rather than the
  // tab's own would push every rect off the left edge of the map.
  test("rects normalise against the tab's own origin, not the screen's", () => {
    const panes = [aPane("pn_a"), aPane("pn_b")];
    const layout = aLayout([
      ["pn_a", rect(29, 1, 31, 40)],
      ["pn_b", rect(60, 1, 70, 40)],
    ]);

    const placed = Floorplan.place(panes, layout);

    expect(placed.map((p) => p.left)).toEqual([0, (31 / 101) * 100]);
    expect(placed[0].top).toBe(0);
    expect(placed[0].height).toBe(100);
    expect(placed[0].width + placed[1].width).toBeCloseTo(100);
  });

  // The ordinals are what a person reads off the map and says out loud, so they
  // have to follow the map. Numbering by list position would call the bottom
  // pane C1 whenever the backend happened to report it first.
  test("agent ordinals follow reading order rather than list order", () => {
    const panes = [
      aPane("pn_low", { agent: "claude" as unknown as Pane["agent"] }),
      aPane("pn_high", { agent: "claude" as unknown as Pane["agent"] }),
      aPane("pn_shell"),
    ];
    const layout = aLayout([
      ["pn_low", rect(0, 20, 30, 20)],
      ["pn_high", rect(0, 0, 30, 20)],
      ["pn_shell", rect(30, 0, 70, 40)],
    ]);

    const placed = Floorplan.place(panes, layout);
    const named = new Map(placed.map((p) => [p.pane.id as unknown as string, p.ordinal]));

    expect(named.get("pn_high")).toBe("C1");
    expect(named.get("pn_low")).toBe("C2");
    expect(named.get("pn_shell")).toBeNull();
  });

  test("a pane running no agent is named by its command", () => {
    const panes = [aPane("pn_a", { foreground_command: "pwsh" })];
    const layout = aLayout([["pn_a", rect(0, 0, 80, 24)]]);

    const [placed] = Floorplan.place(panes, layout);

    expect(placed.name).toBe("pwsh");
    // Not repeated underneath itself.
    expect(placed.detail).toBeNull();
  });

  test("an agent's rectangle says what it is running under its ordinal", () => {
    const panes = [
      aPane("pn_a", {
        agent: "claude" as unknown as Pane["agent"],
        title: "migration",
      }),
    ];
    const layout = aLayout([["pn_a", rect(0, 0, 80, 24)]]);

    const [placed] = Floorplan.place(panes, layout);

    expect(placed.name).toBe("C1");
    expect(placed.detail).toBe("migration");
  });

  test("a tab with no extent draws nothing rather than dividing by zero", () => {
    const panes = [aPane("pn_a")];
    const layout = aLayout([["pn_a", rect(0, 0, 0, 0)]]);

    expect(Floorplan.place(panes, layout)).toEqual([]);
  });

  test("the zoomed pane is marked, and only that one", () => {
    const panes = [aPane("pn_a"), aPane("pn_b")];
    const layout = aLayout(
      [
        ["pn_a", rect(0, 0, 40, 20)],
        ["pn_b", rect(40, 0, 40, 20)],
      ],
      "pn_b",
    );

    const placed = Floorplan.place(panes, layout);

    expect(placed.map((p) => p.zoomed)).toEqual([false, true]);
  });

  test("status is carried through per pane", () => {
    const panes = [aPane("pn_a", { agent: "claude" as unknown as Pane["agent"] })];
    const layout = aLayout([["pn_a", rect(0, 0, 40, 20)]]);

    const [placed] = Floorplan.place(panes, layout, { pn_a: "blocked" });

    expect(placed.status).toBe("blocked");
  });
});

describe("Floorplan.shell", () => {
  // The opinionated rule: chat is the better window onto an agent, so the
  // terminal never opens into one.
  test("the terminal opens into the first pane running no agent", () => {
    const panes = [
      aPane("pn_a", { agent: "claude" as unknown as Pane["agent"] }),
      aPane("pn_b", { foreground_command: "pwsh" }),
    ];

    expect(Floorplan.shell(panes)?.id).toBe("pn_b");
  });

  // Null rather than the agent's pane. The caller offers a split; making one
  // silently would change the desk's layout because somebody tapped Terminal.
  test("a tab of nothing but agents has no shell to open", () => {
    const panes = [aPane("pn_a", { agent: "claude" as unknown as Pane["agent"] })];

    expect(Floorplan.shell(panes)).toBeNull();
  });
});
