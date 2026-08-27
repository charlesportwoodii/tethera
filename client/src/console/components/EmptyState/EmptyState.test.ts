import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import EmptyState from "./EmptyState.svelte";

describe("EmptyState", () => {
  it("puts the title in a heading", () => {
    const { getByRole } = render(EmptyState, { props: { title: "No servers yet" } });
    expect(getByRole("heading", { name: "No servers yet" })).toBeInTheDocument();
  });

  it("says what would fill the screen", () => {
    const { getByText } = render(EmptyState, {
      props: { title: "forge is idle", body: "No agent is running on it right now." },
    });
    expect(getByText("No agent is running on it right now.")).toBeInTheDocument();
  });

  it("omits the body rather than leaving an empty paragraph", () => {
    const { container } = render(EmptyState, { props: { title: "Nothing open here" } });
    expect(container.querySelector(".tc-empty__body")).toBeNull();
  });

  it("keeps the mark out of the accessibility tree", () => {
    const { container } = render(EmptyState, { props: { icon: "terminal", title: "x" } });
    // Decoration: the heading already says what this is.
    expect(container.querySelector(".tc-empty__mark")).toHaveAttribute("aria-hidden", "true");
  });

  it("renders without a mark for a state that needs no picture", () => {
    const { container } = render(EmptyState, { props: { title: "x" } });
    expect(container.querySelector(".tc-empty__mark")).toBeNull();
  });

  it("omits the action row entirely when there is nothing to do", () => {
    const { container } = render(EmptyState, { props: { title: "x", body: "y" } });
    expect(container.querySelector(".tc-empty__actions")).toBeNull();
  });
});
