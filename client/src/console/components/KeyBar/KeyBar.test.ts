import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import KeyBar from "./KeyBar.svelte";
import { DEFAULT_KEYS } from "./KeyBar.types";

describe("KeyBar", () => {
  it("ships with the keys a phone terminal cannot do without", () => {
    const { getByRole } = render(KeyBar, { props: {} });
    for (const key of ["esc", "tab", "ctrl", "^C"]) {
      expect(getByRole("button", { name: key })).toBeInTheDocument();
    }
  });

  it("sends the caption that was pressed", async () => {
    const onkey = vi.fn();
    const { getByRole } = render(KeyBar, { props: { onkey } });
    await userEvent.click(getByRole("button", { name: "^C" }));
    expect(onkey).toHaveBeenCalledWith("^C");
  });

  it("takes a caller-supplied layout", () => {
    const { getAllByRole } = render(KeyBar, { props: { rows: [["a", "b"]] } });
    expect(getAllByRole("button")).toHaveLength(2);
  });

  it("keeps the default layout to two rows", () => {
    expect(DEFAULT_KEYS).toHaveLength(2);
  });
});
