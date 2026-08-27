import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import TabStrip from "./TabStrip.svelte";
import type { Tab } from "$bindings/Tab";

const tab = (over: Partial<Tab> = {}): Tab => ({
  id: "t1",
  workspace_id: "w1",
  index: 1,
  title: "claude",
  conversation: "c1",
  foreground_command: null,
  ...over,
});

const TABS: Tab[] = [
  tab({ id: "a", index: 1, title: "claude" }),
  tab({ id: "b", index: 2, title: "build", conversation: null }),
  tab({ id: "c", index: 3, title: "git", conversation: null }),
];

describe("TabStrip", () => {
  it("labels a tab with the backend's own ordinal", () => {
    const { getByRole } = render(TabStrip, { props: { tabs: TABS, activeId: "b" } });
    // A number from list position would renumber every tab when one closes.
    expect(getByRole("tab", { name: /2:build/ })).toBeInTheDocument();
  });

  it("orders by that ordinal, not by arrival", () => {
    const shuffled = [TABS[2], TABS[0], TABS[1]];
    const { getAllByRole } = render(TabStrip, { props: { tabs: shuffled, activeId: "a" } });
    // A watch event landing out of order must not reorder the row.
    expect(getAllByRole("tab").map((t) => t.textContent?.trim().slice(0, 7))).toEqual([
      "1:claud",
      "2:build",
      "3:git",
    ]);
  });

  it("marks exactly one tab selected", () => {
    const { getAllByRole } = render(TabStrip, { props: { tabs: TABS, activeId: "b" } });
    const selected = getAllByRole("tab").filter(
      (t) => t.getAttribute("aria-selected") === "true",
    );
    expect(selected).toHaveLength(1);
    expect(selected[0]).toHaveTextContent("2:build");
  });

  it("selects nothing when nothing is open yet", () => {
    const { getAllByRole } = render(TabStrip, { props: { tabs: TABS } });
    expect(
      getAllByRole("tab").filter((t) => t.getAttribute("aria-selected") === "true"),
    ).toHaveLength(0);
  });

  it("reports the selected id", async () => {
    const onselect = vi.fn();
    const { getAllByRole } = render(TabStrip, {
      props: { tabs: TABS, activeId: "b", onselect },
    });
    await userEvent.click(getAllByRole("tab")[2]);
    expect(onselect).toHaveBeenCalledWith("c");
  });

  it("shows agent state only for a tab that has an agent", () => {
    const { getAllByRole } = render(TabStrip, {
      props: { tabs: TABS, activeId: "b", states: { a: "working" } },
    });
    const glyphs = getAllByRole("tab").map((t) => t.querySelector('[role="img"]'));
    expect(glyphs[0]).not.toBeNull();
    // A plain shell gets no glyph rather than an idle one.
    expect(glyphs[1]).toBeNull();
  });

  it("offers to make the first tab when none are open", () => {
    const { getByRole, queryAllByRole } = render(TabStrip, {
      props: { tabs: [], onadd: () => {} },
    });
    // The only useful thing on an empty strip is the way to make the first tab.
    expect(queryAllByRole("tab")).toHaveLength(0);
    expect(getByRole("button", { name: /New tab/ })).toBeInTheDocument();
  });

  it("says so plainly when there are no tabs and it cannot make one", () => {
    const { getByText, queryByRole } = render(TabStrip, { props: { tabs: [] } });
    expect(getByText("No tabs open")).toBeInTheDocument();
    expect(queryByRole("button")).toBeNull();
  });

  it("omits the add control when the machine will not take another tab", () => {
    const { queryByRole } = render(TabStrip, { props: { tabs: TABS, activeId: "a" } });
    expect(queryByRole("button", { name: "New tab" })).toBeNull();
  });

  it("makes a tab when asked", async () => {
    const onadd = vi.fn();
    const { getByRole } = render(TabStrip, { props: { tabs: [], onadd } });
    await userEvent.click(getByRole("button", { name: /New tab/ }));
    expect(onadd).toHaveBeenCalledOnce();
  });
});
