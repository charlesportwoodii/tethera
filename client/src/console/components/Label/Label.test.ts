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

  // The rule is what makes a section word read as the top of a band rather than
  // as a caption floating above unrelated rows.
  it("draws a rule across the rest of the line when asked", () => {
    const { container } = render(Label, { props: { rule: true } });
    expect(container.querySelector(".tc-label__rule")).not.toBeNull();
  });

  it("draws no rule by default, so every existing caller is unchanged", () => {
    const { container } = render(Label, { props: {} });
    expect(container.querySelector(".tc-label__rule")).toBeNull();
  });

  it("carries a count at the end of the line", () => {
    const { getByText } = render(Label, { props: { rule: true, count: 5 } });
    expect(getByText("5")).toBeInTheDocument();
  });

  // Zero is a count, and a band headed "0" says something a missing figure does
  // not. Only an absent count is absent.
  it("shows a count of zero rather than treating it as absent", () => {
    const { getByText } = render(Label, { props: { count: 0 } });
    expect(getByText("0")).toBeInTheDocument();
  });

  it("omits the count entirely when none is given", () => {
    const { container } = render(Label, { props: { rule: true } });
    expect(container.querySelector(".tc-label__count")).toBeNull();
  });

  it("can take the attention tone, for a band that is asking for something", () => {
    const { container } = render(Label, { props: { tone: "urgent" } });
    expect(container.querySelector(".tc-label")).toHaveAttribute("data-tone", "urgent");
  });
});
