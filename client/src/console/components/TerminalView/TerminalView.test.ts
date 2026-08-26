import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import TerminalView from "./TerminalView.svelte";

describe("TerminalView", () => {
  it("renders one element per line, in order", () => {
    const { container } = render(TerminalView, {
      props: { lines: [{ text: "a" }, { text: "b" }] },
    });
    const rendered = [...container.querySelectorAll(".tc-term__line")].map((n) => n.textContent);
    expect(rendered).toEqual(["a", "b"]);
  });

  it("defaults a line with no tone to plain", () => {
    const { container } = render(TerminalView, { props: { lines: [{ text: "x" }] } });
    expect(container.querySelector(".tc-term__line")?.className).toContain("is-plain");
  });

  it("applies the named tone", () => {
    const { container } = render(TerminalView, {
      props: { lines: [{ text: "FAILED", tone: "attn" }] },
    });
    expect(container.querySelector(".tc-term__line")?.className).toContain("is-attn");
  });

  it("preserves whitespace, because a pane is a grid", () => {
    const { container } = render(TerminalView, {
      props: { lines: [{ text: "  left:  \"/pair\"" }] },
    });
    expect(container.querySelector(".tc-term__line")?.textContent).toBe('  left:  "/pair"');
  });

  it("draws the cursor only when asked", () => {
    const off = render(TerminalView, { props: { lines: [] } });
    expect(off.container.querySelector(".tc-term__cursor")).toBeNull();
    off.unmount();

    const on = render(TerminalView, { props: { lines: [], cursor: true } });
    expect(on.container.querySelector(".tc-term__cursor")).toBeInTheDocument();
  });

  it("is focusable, so a hardware keyboard can reach the pane", () => {
    const { getByRole } = render(TerminalView, { props: { lines: [] } });
    expect(getByRole("log")).toHaveAttribute("tabindex", "0");
  });
});
