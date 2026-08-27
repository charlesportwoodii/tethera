import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import KeyBar from "./KeyBar.svelte";
import { DEFAULT_KEYS, MOD } from "./KeyBar.types";

describe("KeyBar", () => {
  it("ships with the keys a phone terminal cannot do without", () => {
    const { getByRole } = render(KeyBar, { props: {} });
    for (const label of ["esc", "tab", "^C", "⏎"]) {
      expect(getByRole("button", { name: label })).toBeInTheDocument();
    }
  });

  it("sends a named key as intent, with no modifiers", async () => {
    const onkey = vi.fn();
    const { getByRole } = render(KeyBar, { props: { onkey } });
    await userEvent.click(getByRole("button", { name: "esc" }));
    expect(onkey).toHaveBeenCalledWith("escape", MOD.none);
  });

  it("sends a control combination as a char plus a modifier bit", async () => {
    const onkey = vi.fn();
    const { getByRole } = render(KeyBar, { props: { onkey } });
    await userEvent.click(getByRole("button", { name: "^C" }));
    // Never the byte 0x03: the wire has no raw path, and encoding is the
    // server's job precisely so a phone never has to know a terminal encoding.
    expect(onkey).toHaveBeenCalledWith({ char: "c" }, MOD.ctrl);
  });

  it("sends the arrows as named keys rather than escape sequences", async () => {
    const onkey = vi.fn();
    const { getByRole } = render(KeyBar, { props: { onkey } });
    await userEvent.click(getByRole("button", { name: "↑" }));
    expect(onkey).toHaveBeenCalledWith("up", MOD.none);
  });

  it("takes a caller-supplied layout", () => {
    const { getAllByRole } = render(KeyBar, {
      props: { rows: [[{ label: "a", key: { char: "a" } }]] },
    });
    expect(getAllByRole("button")).toHaveLength(1);
  });

  it("keeps the default layout to two rows", () => {
    expect(DEFAULT_KEYS).toHaveLength(2);
  });

  it("gives every default cap a key to send", () => {
    for (const row of DEFAULT_KEYS) {
      for (const cap of row) {
        expect(cap.key).toBeDefined();
      }
    }
  });
});
