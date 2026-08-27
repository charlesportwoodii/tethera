import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import PaneMap from "./PaneMap.svelte";
import type { PaneBox } from "./PaneMap.types";

const SPLIT: PaneBox[] = [
  { id: "p1", label: "claude", x: 0, y: 0, w: 0.5, h: 1 },
  { id: "p2", label: "cargo watch", x: 0.5, y: 0, w: 0.5, h: 1 },
];

describe("PaneMap", () => {
  it("draws nothing for a tab that was never split", () => {
    const { container } = render(PaneMap, {
      props: { panes: [SPLIT[0]], activeId: "p1" },
    });
    // A diagram of one rectangle is furniture.
    expect(container.querySelector(".tc-panemap")).toBeNull();
  });

  it("draws one cell per pane once there is a layout to show", () => {
    const { getAllByRole } = render(PaneMap, { props: { panes: SPLIT, activeId: "p1" } });
    expect(getAllByRole("button")).toHaveLength(2);
  });

  it("places cells from the fractions it was given", () => {
    const { getByRole } = render(PaneMap, { props: { panes: SPLIT, activeId: "p1" } });
    const second = getByRole("button", { name: "cargo watch" }) as HTMLElement;
    // Geometry is supplied, never derived: the wire carries no pane positions.
    expect(second.style.left).toBe("50%");
    expect(second.style.width).toBe("50%");
  });

  it("marks the pane being viewed, since the phone shows one at a time", () => {
    const { getByRole } = render(PaneMap, { props: { panes: SPLIT, activeId: "p2" } });
    expect(getByRole("button", { name: "cargo watch" })).toHaveAttribute("aria-pressed", "true");
    expect(getByRole("button", { name: "claude" })).toHaveAttribute("aria-pressed", "false");
  });

  it("names the pane being viewed", () => {
    const { getByText } = render(PaneMap, { props: { panes: SPLIT, activeId: "p2" } });
    expect(getByText("cargo watch")).toBeInTheDocument();
  });

  it("falls back to a count when nothing is selected", () => {
    const { getByText } = render(PaneMap, { props: { panes: SPLIT } });
    expect(getByText("2 panes")).toBeInTheDocument();
  });

  it("reports which pane was picked", async () => {
    const onselect = vi.fn();
    const { getByRole } = render(PaneMap, { props: { panes: SPLIT, activeId: "p1", onselect } });
    await userEvent.click(getByRole("button", { name: "cargo watch" }));
    expect(onselect).toHaveBeenCalledWith("p2");
  });

  it("omits splitting when the machine will not split", () => {
    const { queryByRole } = render(PaneMap, { props: { panes: SPLIT, activeId: "p1" } });
    expect(queryByRole("button", { name: "Split beside" })).toBeNull();
  });

  it("splits in the direction that was pressed", async () => {
    const onsplit = vi.fn();
    const { getByRole } = render(PaneMap, {
      props: { panes: SPLIT, activeId: "p1", onsplit },
    });
    await userEvent.click(getByRole("button", { name: "Split below" }));
    expect(onsplit).toHaveBeenCalledWith("horizontal");
  });

  it("clamps a fraction outside the grid rather than drawing off the edge", () => {
    const { getByRole } = render(PaneMap, {
      props: {
        panes: [SPLIT[0], { id: "p3", label: "odd", x: -1, y: 0, w: 4, h: 1 }],
        activeId: "p1",
      },
    });
    const odd = getByRole("button", { name: "odd" }) as HTMLElement;
    expect(odd.style.left).toBe("0%");
    expect(odd.style.width).toBe("100%");
  });
});
