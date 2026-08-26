import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import Button from "./Button.svelte";

describe("Button", () => {
  it("defaults to the primary variant", () => {
    const { getByRole } = render(Button, { props: {} });
    expect(getByRole("button")).toHaveAttribute("data-variant", "primary");
  });

  it("calls onclick when pressed", async () => {
    const onclick = vi.fn();
    const { getByRole } = render(Button, { props: { onclick } });
    await userEvent.click(getByRole("button"));
    expect(onclick).toHaveBeenCalledOnce();
  });

  it("does not call onclick while disabled", async () => {
    const onclick = vi.fn();
    const { getByRole } = render(Button, { props: { onclick, disabled: true } });
    await userEvent.click(getByRole("button"));
    expect(onclick).not.toHaveBeenCalled();
  });

  it("defaults to type button so it never submits a form by accident", () => {
    const { getByRole } = render(Button, { props: {} });
    expect(getByRole("button")).toHaveAttribute("type", "button");
  });

  it("keeps the icon out of the accessible name", () => {
    const { getByRole } = render(Button, { props: { icon: "plus" } });
    // The icon is unlabelled, so the name comes from the text alone.
    expect(getByRole("button").querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });
});
