import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import KeyStream from "./KeyStream.svelte";

function mount(over: Record<string, unknown> = {}) {
  const onkey = vi.fn();
  const ontext = vi.fn();
  const rendered = render(KeyStream, { props: { onkey, ontext, ...over } });

  return { onkey, ontext, ...rendered };
}

describe("KeyStream", () => {
  // A terminal has no submit. Every keystroke is the message, which is the whole
  // difference between driving vim and typing a command line at it.
  it("emits each printable character as it is typed", async () => {
    const { ontext, getByRole } = mount();

    await userEvent.click(getByRole("textbox"));
    await userEvent.keyboard("ab");

    expect(ontext).toHaveBeenNthCalledWith(1, "a");
    expect(ontext).toHaveBeenNthCalledWith(2, "b");
  });

  it("sends Enter as a key rather than as text", async () => {
    const { onkey, ontext, getByRole } = mount();

    await userEvent.click(getByRole("textbox"));
    await userEvent.keyboard("{Enter}");

    expect(onkey).toHaveBeenCalledWith("enter", 0);
    expect(ontext).not.toHaveBeenCalled();
  });

  // Ctrl and Alt reach the pane as modifiers on a key, never as text. `Mods` is
  // xterm's bit order: shift 1, alt 2, ctrl 4.
  it("carries ctrl as a modifier on the character", async () => {
    const { onkey, getByRole } = mount();

    await userEvent.click(getByRole("textbox"));
    await userEvent.keyboard("{Control>}c{/Control}");

    expect(onkey).toHaveBeenCalledWith({ char: "c" }, 4);
  });

  it("names the keys a soft keyboard cannot produce", async () => {
    const { onkey, getByRole } = mount();

    await userEvent.click(getByRole("textbox"));
    await userEvent.keyboard("{Escape}{Tab}{ArrowUp}{Backspace}");

    expect(onkey).toHaveBeenNthCalledWith(1, "escape", 0);
    expect(onkey).toHaveBeenNthCalledWith(2, "tab", 0);
    expect(onkey).toHaveBeenNthCalledWith(3, "up", 0);
    expect(onkey).toHaveBeenNthCalledWith(4, "backspace", 0);
  });

  // The field never accumulates. A value left behind would be sent twice the
  // next time anything flushed it, and would also show a phantom line of text
  // over a terminal that has already received every character.
  it("holds no text between keystrokes", async () => {
    const { getByRole } = mount();
    const field = getByRole("textbox") as HTMLInputElement;

    await userEvent.click(field);
    await userEvent.keyboard("hello");

    expect(field.value).toBe("");
  });

  it("sends nothing while disabled", async () => {
    const { onkey, ontext, getByRole } = mount({ disabled: true });

    await userEvent.click(getByRole("textbox"));
    await userEvent.keyboard("a{Enter}");

    expect(onkey).not.toHaveBeenCalled();
    expect(ontext).not.toHaveBeenCalled();
  });
});
