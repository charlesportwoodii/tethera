import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import ToolFold from "./ToolFold.svelte";

describe("ToolFold", () => {
  it("reports that it folds, and whether it is open", () => {
    const { getByRole } = render(ToolFold, { props: { name: "Bash" } });
    expect(getByRole("button")).toHaveAttribute("aria-expanded", "false");
  });

  it("puts the detail in the accessible name", () => {
    const { getByRole } = render(ToolFold, { props: { name: "Bash", detail: "2 hits" } });
    expect(getByRole("button", { name: /Bash 2 hits/ })).toBeInTheDocument();
  });

  it("spins while the tool is still running", () => {
    const { container } = render(ToolFold, { props: { name: "Bash", status: "running" } });
    expect(container.querySelector(".tc-braille")).toBeInTheDocument();
  });

  it("shows a chevron once it has finished", () => {
    const { container } = render(ToolFold, { props: { name: "Bash", status: "ok" } });
    expect(container.querySelector(".tc-braille")).toBeNull();
    expect(container.querySelector(".tc-icon")).toBeInTheDocument();
  });

  it("marks a failure in the DOM, so it can be found without reading it", () => {
    const { getByRole } = render(ToolFold, {
      props: { name: "cargo test", detail: "1 failed", status: "failed" },
    });
    expect(getByRole("button")).toHaveAttribute("data-status", "failed");
  });

  it("calls onclick when opened", async () => {
    const onclick = vi.fn();
    const { getByRole } = render(ToolFold, { props: { name: "Bash", onclick } });
    await userEvent.click(getByRole("button"));
    expect(onclick).toHaveBeenCalledOnce();
  });
});
