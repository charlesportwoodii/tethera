import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import Drawer from "./Drawer.svelte";

describe("Drawer", () => {
  it("peeks by default — the pane is never gone", () => {
    const { getByRole } = render(Drawer, { props: { label: "2:build" } });
    expect(getByRole("region", { name: "Pane" })).toHaveAttribute("data-height", "peek");
  });

  it("reports collapsed state through aria-expanded", () => {
    const { getByRole } = render(Drawer, { props: { label: "2:build" } });
    expect(getByRole("button")).toHaveAttribute("aria-expanded", "false");
  });

  it("cycles peek to half to full and back", async () => {
    const onheight = vi.fn();
    const peek = render(Drawer, { props: { label: "x", height: "peek", onheight } });
    await userEvent.click(peek.getByRole("button"));
    expect(onheight).toHaveBeenLastCalledWith("half");
    peek.unmount();

    const full = render(Drawer, { props: { label: "x", height: "full", onheight } });
    await userEvent.click(full.getByRole("button"));
    expect(onheight).toHaveBeenLastCalledWith("peek");
  });

  it("shows the summary while peeking, so the strip still reports", () => {
    const { getByText } = render(Drawer, {
      props: { label: "2:build", summary: "11 passed, 1 failed" },
    });
    expect(getByText("11 passed, 1 failed")).toBeInTheDocument();
  });
});
