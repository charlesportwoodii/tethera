import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import TerminalPane from "./TerminalPane.svelte";
import { TerminalGrid } from "$console/lib/terminal";
import { MOD } from "$console/components/KeyBar/KeyBar.types";
import type { PaneBox } from "$console/components/PaneMap/PaneMap.types";
import type { Style } from "$bindings/Style";
import type { Tab } from "$bindings/Tab";
import type { TerminalFrame } from "$bindings/TerminalFrame";

type Snapshot = Extract<TerminalFrame, { snapshot: unknown }>;

const plain: Style = { fg: "default", bg: "default", attrs: 0 };

const tab = (over: Partial<Tab> = {}): Tab => ({
  id: "t1",
  workspace_id: "w1",
  index: 1,
  title: "claude",
  conversation: "c1",
  foreground_command: null,
  ...over,
});

const TABS = [tab({ id: "a", index: 1, title: "claude" }), tab({ id: "b", index: 2, title: "build" })];

const PANES: PaneBox[] = [
  { id: "p1", label: "claude", x: 0, y: 0, w: 0.5, h: 1 },
  { id: "p2", label: "cargo watch", x: 0.5, y: 0, w: 0.5, h: 1 },
];

function loaded(text = "ready", cols = 20, rows = 2): TerminalGrid {
  const frame: Snapshot = {
    snapshot: {
      cols,
      rows,
      styles: [plain],
      rows_data: [{ y: 0, from_x: 0, spans: [{ style: 0, text }] }],
      cursor: null,
      alt_screen: false,
      scrollback_len: 0,
    },
  };
  const grid = new TerminalGrid();
  grid.apply(frame);
  return grid;
}

describe("TerminalPane", () => {
  it("shows the tabs, the screen and the keys once a tab is open", () => {
    const { getAllByRole, getByRole, container } = render(TerminalPane, {
      props: { tabs: TABS, activeTabId: "b", grid: loaded() },
    });
    expect(getAllByRole("tab")).toHaveLength(2);
    expect(getByRole("log").textContent).toContain("ready");
    expect(container.querySelector(".tc-keys")).toBeInTheDocument();
  });

  it("reports the pane's geometry as its label", () => {
    const { getByRole } = render(TerminalPane, {
      props: { tabs: TABS, activeTabId: "b", grid: loaded("x", 80, 24) },
    });
    expect(getByRole("log")).toHaveAccessibleName("80×24");
  });

  it("hides the map for a tab that was never split", () => {
    const { container } = render(TerminalPane, {
      props: { tabs: TABS, activeTabId: "b", panes: [PANES[0]], grid: loaded() },
    });
    expect(container.querySelector(".tc-panemap")).toBeNull();
  });

  it("shows the map once the tab has more than one pane", () => {
    const { container } = render(TerminalPane, {
      props: { tabs: TABS, activeTabId: "b", panes: PANES, activePaneId: "p1", grid: loaded() },
    });
    expect(container.querySelector(".tc-panemap")).toBeInTheDocument();
  });

  it("shows no screen and no keys when the workspace has no tabs", () => {
    const { queryByRole, container } = render(TerminalPane, {
      props: { tabs: [], grid: loaded() },
    });
    expect(queryByRole("log")).toBeNull();
    expect(container.querySelector(".tc-keys")).toBeNull();
  });

  it("says what making the first tab will do, not merely that there is none", () => {
    const { getByText } = render(TerminalPane, { props: { tabs: [], grid: loaded() } });
    // An empty screen is an invitation; "Nothing open here" alone tells somebody
    // what they can already see.
    expect(getByText("Nothing open here")).toBeInTheDocument();
    expect(getByText(/starts a shell/)).toBeInTheDocument();
  });

  it("names the machine in the empty action, so it is clear where the tab lands", () => {
    const { getByRole } = render(TerminalPane, {
      props: { tabs: [], grid: loaded(), onaddtab: () => {}, machine: "atlas" },
    });
    expect(getByRole("button", { name: /New tab on atlas/ })).toBeInTheDocument();
  });

  it("offers no way to make a tab when the machine will not take one", () => {
    const { queryByRole } = render(TerminalPane, { props: { tabs: [], grid: loaded() } });
    expect(queryByRole("button", { name: /New tab/ })).toBeNull();
  });

  it("passes a tab choice up rather than deciding locally", async () => {
    const onselecttab = vi.fn();
    const { getAllByRole } = render(TerminalPane, {
      props: { tabs: TABS, activeTabId: "a", grid: loaded(), onselecttab },
    });
    await userEvent.click(getAllByRole("tab")[1]);
    // Opening a tab is an RPC; deciding here would show a tab the machine does
    // not have.
    expect(onselecttab).toHaveBeenCalledWith("b");
  });

  it("passes a pane choice up", async () => {
    const onselectpane = vi.fn();
    const { getByRole } = render(TerminalPane, {
      props: {
        tabs: TABS,
        activeTabId: "b",
        panes: PANES,
        activePaneId: "p1",
        grid: loaded(),
        onselectpane,
      },
    });
    await userEvent.click(getByRole("button", { name: "cargo watch" }));
    expect(onselectpane).toHaveBeenCalledWith("p2");
  });

  it("sends a key as intent", async () => {
    const onkey = vi.fn();
    const { getByRole } = render(TerminalPane, {
      props: { tabs: TABS, activeTabId: "b", grid: loaded(), onkey },
    });
    await userEvent.click(getByRole("button", { name: "^C" }));
    expect(onkey).toHaveBeenCalledWith({ char: "c" }, MOD.ctrl);
  });

  it("omits splitting when the machine will not split", () => {
    const { queryByRole } = render(TerminalPane, {
      props: { tabs: TABS, activeTabId: "b", panes: PANES, activePaneId: "p1", grid: loaded() },
    });
    expect(queryByRole("button", { name: "Split beside" })).toBeNull();
  });

  it("repaints when the revision moves", async () => {
    const grid = loaded("first", 20, 1);
    const { getByRole, rerender } = render(TerminalPane, {
      props: { tabs: TABS, activeTabId: "b", grid, revision: 1 },
    });
    grid.apply({
      damage: {
        styles: [plain],
        rows_data: [{ y: 0, from_x: 0, spans: [{ style: 0, text: "second" }] }],
        cursor: null,
      },
    });
    await rerender({ tabs: TABS, activeTabId: "b", grid, revision: 2 });
    expect(getByRole("log").textContent).toContain("second");
  });
});
