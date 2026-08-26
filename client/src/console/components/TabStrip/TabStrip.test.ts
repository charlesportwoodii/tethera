import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import TabStrip from "./TabStrip.svelte";

const TABS = [
  { id: "a", label: "1:claude", state: "blocked" as const },
  { id: "b", label: "2:build" },
  { id: "c", label: "3:git" },
];

describe("TabStrip", () => {
  it("marks exactly one tab selected", () => {
    const { getAllByRole } = render(TabStrip, { props: { tabs: TABS, activeId: "b" } });
    const selected = getAllByRole("tab").filter(
      (t) => t.getAttribute("aria-selected") === "true",
    );
    expect(selected).toHaveLength(1);
    expect(selected[0]).toHaveTextContent("2:build");
  });

  it("shows agent state only on tabs that have an agent", () => {
    const { getAllByRole } = render(TabStrip, { props: { tabs: TABS, activeId: "b" } });
    const glyphs = getAllByRole("tab").map((t) => t.querySelector('[role="img"]'));
    expect(glyphs[0]).not.toBeNull();
    expect(glyphs[1]).toBeNull();
  });

  it("reports the selected id", async () => {
    const onselect = vi.fn();
    const { getAllByRole } = render(TabStrip, {
      props: { tabs: TABS, activeId: "b", onselect },
    });
    await userEvent.click(getAllByRole("tab")[2]);
    expect(onselect).toHaveBeenCalledWith("c");
  });

  it("omits the add control when the host will not take a new tab", () => {
    const { queryByRole } = render(TabStrip, { props: { tabs: TABS, activeId: "b" } });
    expect(queryByRole("button", { name: "New tab" })).toBeNull();
  });
});
