import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import Label from "./Label.svelte";

describe("Label", () => {
  it("defaults to a section word", () => {
    const { container } = render(Label, { props: {} });
    expect(container.querySelector(".tc-label")).toHaveAttribute("data-kind", "section");
  });

  it("marks a field label differently so the rail spacing does not apply", () => {
    const { container } = render(Label, { props: { kind: "field" } });
    expect(container.querySelector(".tc-label")).toHaveAttribute("data-kind", "field");
  });

  it("can start flush at the screen edge instead of the gutter", () => {
    const { container } = render(Label, { props: { flush: true } });
    expect(container.querySelector(".tc-label")?.className).toContain("is-flush");
  });
});
