import { describe, expect, test, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import LayoutSheet from "./LayoutSheet.svelte";
import type { Pane } from "$bindings/Pane";
import type { TabLayout } from "$bindings/TabLayout";
import type { TerminalControls } from "$bindings/TerminalControls";

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
    streamed: true,
    ...over,
  };
}

function anAgent(id: string, conversation: string): Pane {
  return aPane(id, {
    agent: "claude" as unknown as Pane["agent"],
    streamed: true,
    conversation: conversation as unknown as Pane["conversation"],
  });
}

function aLayout(ids: string[]): TabLayout {
  return {
    tab: "tb_1" as unknown as TabLayout["tab"],
    slots: ids.map((pane, at) => ({
      pane: pane as unknown as TabLayout["slots"][number]["pane"],
      rect: { x: at * 40, y: 0, width: 40, height: 20 },
    })),
    zoomed: null,
  };
}

const ALL: TerminalControls = {
  attach: true,
  input: true,
  scrollback: true,
  open: true,
  split: true,
  close: true,
  layout: true,
  focus_tab: true,
};

function props(over: Record<string, unknown> = {}) {
  return {
    panes: [anAgent("pn_a", "cv_1"), aPane("pn_b", { foreground_command: "pwsh" })],
    layout: aLayout(["pn_a", "pn_b"]),
    selected: "pn_a",
    controls: ALL,
    onselect: vi.fn(),
    onenter: vi.fn(),
    onchat: vi.fn(),
    onsplit: vi.fn(),
    onclosepane: vi.fn(),
    ...over,
  };
}

describe("LayoutSheet", () => {
  // The whole point of putting the floorplan in chat mode: three claudes in one
  // tab, and this is how you reach the second one.
  test("an agent's pane leads with its chat, named as the map names it", () => {
    const { getByText } = render(LayoutSheet, { props: props() });

    expect(getByText("Open C1 chat")).toBeTruthy();
  });

  test("choosing an agent's chat hands back the conversation, not the pane", () => {
    const onchat = vi.fn();

    const { getByText } = render(LayoutSheet, { props: props({ onchat }) });

    void fireEvent.click(getByText("Open C1 chat"));

    expect(onchat).toHaveBeenCalledWith("cv_1");
  });

  // A shell has no chat to open, so going in means the pane itself.
  test("a shell leads with the pane and offers both split directions", () => {
    const { getByText, queryByText } = render(LayoutSheet, {
      props: props({ selected: "pn_b" }),
    });

    expect(queryByText(/Open .* chat/)).toBeNull();
    expect(getByText("Enter the pane")).toBeTruthy();
    expect(getByText("Split right")).toBeTruthy();
    expect(getByText("Split down")).toBeTruthy();
  });

  // The whole point of a fixed grid: the slot a control sits in is a property of
  // the control, not of what happens to be selected. Switching panes must not
  // move a button under somebody's thumb.
  test("every action keeps its slot whichever kind of pane is held", () => {
    const slots = (selected: string) => {
      const { container } = render(LayoutSheet, { props: props({ selected }) });

      return [...container.querySelectorAll(".a")].map((held) => [
        getComputedStyle(held).gridRow,
        getComputedStyle(held).gridColumn,
        (held.textContent ?? "").trim(),
      ]);
    };

    const agent = slots("pn_a");
    const shell = slots("pn_b");

    // Same count, same cells, in the same order. Only the first label differs.
    expect(agent.map(([row, col]) => `${row}/${col}`)).toEqual(
      shell.map(([row, col]) => `${row}/${col}`),
    );
    expect(agent[0][2]).toBe("Open C1 chat");
    expect(shell[0][2]).toBe("Enter the pane");
  });

  // Going in is one action with two destinations, not two actions. An agent's
  // pane offering both put "Open C1 chat" and "Enter the pane" on the same
  // sheet, which reads as a choice when it is the same intent.
  test("an agent's pane offers its chat and no second way in", () => {
    const { queryByText } = render(LayoutSheet, { props: props({ selected: "pn_a" }) });

    expect(queryByText("Open C1 chat")).toBeTruthy();
    expect(queryByText("Enter the pane")).toBeNull();
  });

  test("splitting right asks for a horizontal division of the selected pane", () => {
    const onsplit = vi.fn();

    const { getByText } = render(LayoutSheet, {
      props: props({ selected: "pn_b", onsplit }),
    });

    void fireEvent.click(getByText("Split right"));

    expect(onsplit).toHaveBeenCalledWith("pn_b", "horizontal");
  });

  test("splitting down asks for a vertical division", () => {
    const onsplit = vi.fn();

    const { getByText } = render(LayoutSheet, {
      props: props({ selected: "pn_b", onsplit }),
    });

    void fireEvent.click(getByText("Split down"));

    expect(onsplit).toHaveBeenCalledWith("pn_b", "vertical");
  });

  // Absent, not disabled. A control drawn and then refused on press teaches
  // somebody the app is unreliable.
  test("a machine that will not split shows no split", () => {
    const { queryByText } = render(LayoutSheet, {
      props: props({ selected: "pn_b", controls: { ...ALL, split: false } }),
    });

    expect(queryByText("Split right")).toBeNull();
  });

  test("a machine that will not show a pane offers no way into a shell", () => {
    const { queryByText } = render(LayoutSheet, {
      props: props({ selected: "pn_b", controls: { ...ALL, attach: false } }),
    });

    expect(queryByText("Enter the pane")).toBeNull();
  });

  // Chat does not go through the terminal, so a machine that will not stream a
  // pane can still hand somebody the agent running in it.
  test("an agent's chat survives a machine that will not stream a pane", () => {
    const { queryByText } = render(LayoutSheet, {
      props: props({ selected: "pn_a", controls: { ...ALL, attach: false } }),
    });

    expect(queryByText("Open C1 chat")).toBeTruthy();
  });

  // Closing the only pane in a tab closes the tab, which is a different act
  // with a different consequence. The tab strip owns that one.
  test("the last pane in a tab cannot be closed from here", () => {
    const { queryByText } = render(LayoutSheet, {
      props: props({
        panes: [aPane("pn_b", { foreground_command: "pwsh" })],
        layout: aLayout(["pn_b"]),
        selected: "pn_b",
      }),
    });

    expect(queryByText("Close pane")).toBeNull();
  });

  test("nothing selected offers no actions at all", () => {
    const { queryByText } = render(LayoutSheet, { props: props({ selected: null }) });

    expect(queryByText("Enter the pane")).toBeNull();
    expect(queryByText("Split right")).toBeNull();
  });
});
