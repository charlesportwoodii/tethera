import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import TerminalView from "./TerminalView.svelte";
import { ATTR, TerminalGrid } from "$console/lib/terminal";
import type { Style } from "$bindings/Style";
import type { TerminalFrame } from "$bindings/TerminalFrame";

/** The snapshot variant alone. TerminalFrame also includes the bare string "bell". */
type Snapshot = Extract<TerminalFrame, { snapshot: unknown }>;

const plain: Style = { fg: "default", bg: "default", attrs: 0 };

function snapshot(text: string, styles: Style[] = [plain], cols = 12, rows = 2): Snapshot {
  return {
    snapshot: {
      cols,
      rows,
      styles,
      rows_data: [{ y: 0, from_x: 0, spans: [{ style: 0, text }] }],
      cursor: { x: text.length, y: 0, visible: true, shape: "block" },
      alt_screen: false,
      scrollback_len: 0,
    },
  };
}

function loaded(frame: TerminalFrame = snapshot("hello")): TerminalGrid {
  const grid = new TerminalGrid();
  grid.apply(frame);
  return grid;
}

describe("TerminalView", () => {
  it("renders the grid's rows", () => {
    const { container } = render(TerminalView, { props: { grid: loaded() } });
    const rows = container.querySelectorAll(".tc-term__row");
    expect(rows).toHaveLength(2);
    expect(rows[0].textContent).toContain("hello");
  });

  it("reports the geometry as data, so a caller can show 80x24", () => {
    const { getByRole } = render(TerminalView, { props: { grid: loaded() } });
    const pane = getByRole("log");
    expect(pane).toHaveAttribute("data-cols", "12");
    expect(pane).toHaveAttribute("data-rows", "2");
  });

  it("is focusable, so a hardware keyboard can reach the pane", () => {
    const { getByRole } = render(TerminalView, { props: { grid: loaded() } });
    expect(getByRole("log")).toHaveAttribute("tabindex", "0");
  });

  it("tells the host when the pane takes focus, so it can raise a keyboard", async () => {
    const onfocus = vi.fn();
    const { getByRole } = render(TerminalView, { props: { grid: loaded(), onfocus } });
    // Focus rather than click: a tap, a Tab key and a hardware keyboard arriving
    // all mean the same thing to the host.
    await fireEvent.focusIn(getByRole("log"));
    expect(onfocus).toHaveBeenCalled();
  });

  it("draws the cursor on its own row, at its own column", () => {
    const { container } = render(TerminalView, { props: { grid: loaded() } });
    const rows = container.querySelectorAll(".tc-term__row");
    const cursor = rows[0].querySelector(".tc-term__cursor") as HTMLElement | null;
    expect(cursor).not.toBeNull();
    // ch units are exact in a monospace face and need no measurement.
    expect(cursor?.style.left).toBe("5ch");
    expect(rows[1].querySelector(".tc-term__cursor")).toBeNull();
  });

  it("does not draw a hidden cursor", () => {
    const frame = snapshot("hi");
    frame.snapshot.cursor = { x: 0, y: 0, visible: false, shape: "block" };
    const { container } = render(TerminalView, { props: { grid: loaded(frame) } });
    expect(container.querySelector(".tc-term__cursor")).toBeNull();
  });

  it("carries the cursor shape", () => {
    const frame = snapshot("hi");
    frame.snapshot.cursor = { x: 1, y: 0, visible: true, shape: "bar" };
    const { container } = render(TerminalView, { props: { grid: loaded(frame) } });
    expect(container.querySelector(".tc-term__cursor")?.className).toContain("is-bar");
  });

  it("styles a run from the frame's own style table", () => {
    const red: Style = { fg: { indexed: 1 }, bg: "default", attrs: ATTR.bold };
    const { container } = render(TerminalView, { props: { grid: loaded(snapshot("hi", [red])) } });
    const run = container.querySelector(".tc-term__run") as HTMLElement;
    expect(run.className).toContain("is-bold");
    // Custom properties survive a strict parser; a raw color: var(...) does not.
    expect(run.style.getPropertyValue("--tc-run-fg")).toContain("--tc-term-red");
  });

  it("merges a plain row into one run rather than one node per cell", () => {
    const { container } = render(TerminalView, { props: { grid: loaded(snapshot("hello")) } });
    const runs = container.querySelectorAll(".tc-term__row:first-child .tc-term__run");
    expect(runs).toHaveLength(1);
  });

  it("repaints when the revision moves, because the grid mutates in place", async () => {
    const grid = loaded(snapshot("first", [plain], 12, 1));
    const { container, rerender } = render(TerminalView, { props: { grid, revision: 1 } });
    expect(container.textContent).toContain("first");

    grid.apply({
      damage: {
        styles: [plain],
        rows_data: [{ y: 0, from_x: 0, spans: [{ style: 0, text: "second" }] }],
        cursor: null,
      },
    });
    // Without the revision there is nothing for Svelte to compare: the grid
    // object is the same reference it was.
    await rerender({ grid, revision: 2 });
    expect(container.textContent).toContain("second");
  });

  it("says why a pane closed, and says it as a status", () => {
    const grid = loaded();
    grid.apply({ closed: { reason: "exited" } });
    const { getByRole } = render(TerminalView, { props: { grid, revision: 1 } });
    expect(getByRole("status").textContent).toContain("The program exited");
  });

  it("names an unfamiliar close reason rather than rendering nothing", () => {
    const grid = loaded();
    // @ts-expect-error a reason from a newer server
    grid.apply({ closed: { reason: "evicted" } });
    const { getByRole } = render(TerminalView, { props: { grid, revision: 1 } });
    expect(getByRole("status").textContent).toContain("The pane closed");
  });

  it("reports the alternate screen, which has no scrollback to page", () => {
    const frame = snapshot("full screen app");
    frame.snapshot.alt_screen = true;
    frame.snapshot.scrollback_len = null;
    const { getByRole } = render(TerminalView, { props: { grid: loaded(frame) } });
    expect(getByRole("log")).toHaveAttribute("data-alt", "true");
  });
});
