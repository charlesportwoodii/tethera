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

  it("tones the detail without relying on tone alone for meaning", () => {
    const { container } = render(ToolFold, {
      props: { name: "deeplink.ts", detail: "+3 -1", tone: "ok" },
    });
    // The text carries the sign; the colour is the second signal, not the first.
    expect(container.querySelector(".tc-fold__detail")?.textContent).toContain("+3 -1");
  });

  it("calls onclick when opened", async () => {
    const onclick = vi.fn();
    const { getByRole } = render(ToolFold, { props: { name: "Bash", onclick } });
    await userEvent.click(getByRole("button"));
    expect(onclick).toHaveBeenCalledOnce();
  });
});
