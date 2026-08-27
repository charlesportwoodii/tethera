import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import ViewToggle from "./ViewToggle.svelte";

describe("ViewToggle", () => {
  it("offers exactly two sides", () => {
    const { getAllByRole } = render(ViewToggle, { props: { view: "chat" } });
    expect(getAllByRole("tab")).toHaveLength(2);
  });

  it("marks the side being shown", () => {
    const { getByRole } = render(ViewToggle, { props: { view: "terminal" } });
    expect(getByRole("tab", { name: "Terminal" })).toHaveAttribute("aria-selected", "true");
    expect(getByRole("tab", { name: /Chat/ })).toHaveAttribute("aria-selected", "false");
  });

  it("reports the side that was chosen", async () => {
    const onchange = vi.fn();
    const { getByRole } = render(ViewToggle, { props: { view: "chat", onchange } });
    await userEvent.click(getByRole("tab", { name: "Terminal" }));
    expect(onchange).toHaveBeenCalledWith("terminal");
  });

  it("does not render at all when the workspace has no transcript", () => {
    const { queryAllByRole } = render(ViewToggle, {
      props: { view: "terminal", chatAvailable: false },
    });
    // A chat side that turns out to be empty is the control-that-refuses-on-press
    // failure; the screen is simply a terminal instead.
    expect(queryAllByRole("tab")).toHaveLength(0);
  });

  it("badges the chat side when something is waiting there", () => {
    const { getByRole } = render(ViewToggle, {
      props: { view: "terminal", chatBadge: "waiting" },
    });
    const chat = getByRole("tab", { name: /Chat/ });
    expect(chat.querySelector('[role="img"]')).not.toBeNull();
  });

  it("carries no badge when nothing needs attention", () => {
    const { getByRole } = render(ViewToggle, { props: { view: "chat" } });
    expect(getByRole("tab", { name: /Chat/ }).querySelector('[role="img"]')).toBeNull();
  });
});
