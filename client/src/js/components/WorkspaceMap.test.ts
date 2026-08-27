import { describe, expect, test, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import WorkspaceMap from "./WorkspaceMap.svelte";
import type { Pane } from "$bindings/Pane";
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

describe("WorkspaceMap", () => {
  // A machine that will not report geometry gets no box at all. An empty
  // bordered rectangle reads as a workspace with nothing in it, which is a
  // different and wrong statement.
  test("a machine that reports no layout draws nothing", () => {
    const { container } = render(WorkspaceMap, {
      props: { panes: [aPane("pn_a")], layout: null },
    });

    expect(container.querySelector(".map")).toBeNull();
  });

  test("a layout that does not cover every pane draws nothing", () => {
    const { container } = render(WorkspaceMap, {
      props: { panes: [aPane("pn_a"), aPane("pn_b")], layout: aLayout(["pn_a"]) },
    });

    expect(container.querySelector(".map")).toBeNull();
  });

  // Select, then enter. A single tap that switched panes would move the screen
  // under a reader every time a thumb brushed the map.
  test("the first tap selects and does not enter", () => {
    const onselect = vi.fn();
    const onenter = vi.fn();

    const { getByLabelText } = render(WorkspaceMap, {
      props: {
        panes: [aPane("pn_a"), aPane("pn_b")],
        layout: aLayout(["pn_a", "pn_b"]),
        selected: "pn_a",
        onselect,
        onenter,
      },
    });

    void fireEvent.click(getByLabelText("pn_b"));

    expect(onselect).toHaveBeenCalledWith("pn_b");
    expect(onenter).not.toHaveBeenCalled();
  });

  test("tapping the pane already selected enters it", () => {
    const onselect = vi.fn();
    const onenter = vi.fn();

    const { getByLabelText } = render(WorkspaceMap, {
      props: {
        panes: [aPane("pn_a"), aPane("pn_b")],
        layout: aLayout(["pn_a", "pn_b"]),
        selected: "pn_a",
        onselect,
        onenter,
      },
    });

    void fireEvent.click(getByLabelText("pn_a"));

    expect(onenter).toHaveBeenCalledWith("pn_a");
    expect(onselect).not.toHaveBeenCalled();
  });

  // Without `onenter` — a machine that will not stream a pane — the map is a
  // chooser and nothing more. Falling through to a select on the second tap
  // keeps it from looking broken.
  test("a machine that cannot show a pane still lets the map select one", () => {
    const onselect = vi.fn();

    const { getByLabelText } = render(WorkspaceMap, {
      props: {
        panes: [aPane("pn_a")],
        layout: aLayout(["pn_a"]),
        selected: "pn_a",
        onselect,
      },
    });

    void fireEvent.click(getByLabelText("pn_a"));

    expect(onselect).toHaveBeenCalledWith("pn_a");
  });

  test("an agent's rectangle is named for its place on the map", () => {
    const { getByText } = render(WorkspaceMap, {
      props: {
        panes: [
          aPane("pn_a", { agent: "claude" as unknown as Pane["agent"] }),
          aPane("pn_b", { foreground_command: "pwsh" }),
        ],
        layout: aLayout(["pn_a", "pn_b"]),
      },
    });

    expect(getByText("C1")).toBeTruthy();
    expect(getByText("pwsh")).toBeTruthy();
  });
});
