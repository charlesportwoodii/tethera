import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import Chip from "./Chip.svelte";

describe("Chip", () => {
  it("reports selection through aria-checked, not only colour", () => {
    const { getByRole } = render(Chip, { props: { label: "Claude Code", selected: true } });
    expect(getByRole("radio")).toBeChecked();
  });

  it("is unchecked by default", () => {
    const { getByRole } = render(Chip, { props: { label: "Codex" } });
    expect(getByRole("radio")).not.toBeChecked();
  });

  it("puts the detail inside the accessible name so the version is announced", () => {
    const { getByRole } = render(Chip, { props: { label: "Claude Code", detail: "2.1.4" } });
    expect(getByRole("radio", { name: /Claude Code 2\.1\.4/ })).toBeInTheDocument();
  });

  it("calls onclick when chosen", async () => {
    const onclick = vi.fn();
    const { getByRole } = render(Chip, { props: { label: "Codex", onclick } });
    await userEvent.click(getByRole("radio"));
    expect(onclick).toHaveBeenCalledOnce();
  });
});
