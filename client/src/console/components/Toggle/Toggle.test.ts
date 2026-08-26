import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import Toggle from "./Toggle.svelte";

describe("Toggle", () => {
  it("is a switch with an accessible name", () => {
    const { getByRole } = render(Toggle, { props: { label: "Push notifications" } });
    expect(getByRole("switch", { name: "Push notifications" })).toBeInTheDocument();
  });

  it("reports the opposite of its current value", async () => {
    const onchange = vi.fn();
    const { getByRole } = render(Toggle, {
      props: { label: "Push notifications", checked: true, onchange },
    });
    await userEvent.click(getByRole("switch"));
    expect(onchange).toHaveBeenCalledWith(false);
  });

  it("does not flip itself — the parent owns the value", async () => {
    const { getByRole } = render(Toggle, { props: { label: "x", checked: false } });
    await userEvent.click(getByRole("switch"));
    // Uncontrolled state here would fight whatever the gateway says next.
    expect(getByRole("switch")).not.toBeChecked();
  });

  it("stays silent while disabled", async () => {
    const onchange = vi.fn();
    const { getByRole } = render(Toggle, {
      props: { label: "x", disabled: true, onchange },
    });
    await userEvent.click(getByRole("switch"));
    expect(onchange).not.toHaveBeenCalled();
  });
});
