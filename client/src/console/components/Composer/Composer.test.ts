import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import Composer from "./Composer.svelte";

describe("Composer", () => {
  it("cannot send an empty message", () => {
    const { getByRole } = render(Composer, { props: { value: "" } });
    expect(getByRole("button", { name: "Send" })).toBeDisabled();
  });

  it("treats whitespace as empty", () => {
    const { getByRole } = render(Composer, { props: { value: "   " } });
    expect(getByRole("button", { name: "Send" })).toBeDisabled();
  });

  it("sends what is in the field", async () => {
    const onsend = vi.fn();
    const { getByRole } = render(Composer, { props: { value: "1", onsend } });
    await userEvent.click(getByRole("button", { name: "Send" }));
    expect(onsend).toHaveBeenCalledWith("1");
  });

  it("sends on Enter", async () => {
    const onsend = vi.fn();
    const { getByRole } = render(Composer, { props: { value: "hello", onsend } });
    getByRole("textbox").focus();
    await userEvent.keyboard("{Enter}");
    expect(onsend).toHaveBeenCalledWith("hello");
  });

  it("does not send on shift+Enter", async () => {
    const onsend = vi.fn();
    const { getByRole } = render(Composer, { props: { value: "hello", onsend } });
    getByRole("textbox").focus();
    await userEvent.keyboard("{Shift>}{Enter}{/Shift}");
    expect(onsend).not.toHaveBeenCalled();
  });

  it("hides attach entirely when the host cannot take uploads", () => {
    const { queryByRole } = render(Composer, { props: { value: "" } });
    // Absent, not disabled: a dead control promises something the machine cannot do.
    expect(queryByRole("button", { name: "Attach a file" })).toBeNull();
  });

  it("reports typing to the caller rather than keeping its own copy", async () => {
    const oninput = vi.fn();
    const { getByRole } = render(Composer, { props: { value: "", oninput } });
    await userEvent.type(getByRole("textbox"), "a");
    expect(oninput).toHaveBeenCalledWith("a");
  });
});
