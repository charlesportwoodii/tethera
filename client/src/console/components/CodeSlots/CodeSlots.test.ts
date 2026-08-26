import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import CodeSlots from "./CodeSlots.svelte";

describe("CodeSlots", () => {
  it("draws one slot per digit the machine is showing", () => {
    const { container } = render(CodeSlots, { props: { value: "", length: 6 } });
    expect(container.querySelectorAll(".tc-code__slot")).toHaveLength(6);
  });

  it("puts the caret on the next empty slot", () => {
    const { container } = render(CodeSlots, { props: { value: "7329" } });
    const slots = [...container.querySelectorAll(".tc-code__slot")];
    expect(slots[4].className).toContain("is-cursor");
    expect(slots[3].textContent?.trim()).toBe("9");
  });

  it("shows no caret once the code is complete", () => {
    const { container } = render(CodeSlots, { props: { value: "732941" } });
    expect(container.querySelector(".is-cursor")).toBeNull();
  });

  it("ignores anything typed past the length", () => {
    const { container } = render(CodeSlots, { props: { value: "7329411111" } });
    expect(container.querySelector(".tc-code")).toHaveAttribute("data-filled", "6");
  });

  it("is a named group so the whole entry announces once", () => {
    const { getByRole } = render(CodeSlots, { props: { value: "1" } });
    expect(getByRole("group", { name: "Pairing code" })).toBeInTheDocument();
  });
});
