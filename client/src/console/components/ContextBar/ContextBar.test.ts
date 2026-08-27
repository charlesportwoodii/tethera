import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import ContextBar from "./ContextBar.svelte";

describe("ContextBar", () => {
  it("reports the percentage to assistive tech, not just visually", () => {
    const { getByRole } = render(ContextBar, { props: { used: 62000, window: 200000 } });
    expect(getByRole("progressbar")).toHaveAttribute("aria-valuenow", "31");
  });

  it("shows both figures so the number means something", () => {
    const { getByText } = render(ContextBar, { props: { used: 62000, window: 200000 } });
    expect(getByText("62k / 200k")).toBeInTheDocument();
  });

  it("warns before the wall, not at it", () => {
    const { container } = render(ContextBar, { props: { used: 160000, window: 200000 } });
    expect(container.querySelector(".tc-ctx")).toHaveAttribute("data-warn", "true");
  });

  it("does not warn in the ordinary case", () => {
    const { container } = render(ContextBar, { props: { used: 62000, window: 200000 } });
    expect(container.querySelector(".tc-ctx")).toHaveAttribute("data-warn", "false");
  });

  it("clamps rather than overflowing when a window is exceeded", () => {
    const { getByRole } = render(ContextBar, { props: { used: 300000, window: 200000 } });
    expect(getByRole("progressbar")).toHaveAttribute("aria-valuenow", "100");
  });

  it("survives a window of zero without dividing by it", () => {
    const { getByRole } = render(ContextBar, { props: { used: 10, window: 0 } });
    expect(getByRole("progressbar")).toHaveAttribute("aria-valuenow", "0");
  });

  it("can drop the figures for a tight row", () => {
    const { container } = render(ContextBar, { props: { used: 1, window: 2, bare: true } });
    expect(container.querySelector(".tc-ctx__labels")).toBeNull();
  });
});
