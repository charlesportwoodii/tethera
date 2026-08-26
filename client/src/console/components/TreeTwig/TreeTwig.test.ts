import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import TreeTwig from "./TreeTwig.svelte";

describe("TreeTwig", () => {
  it("always draws an elbow — a twig with no join is just an indent", () => {
    const { container } = render(TreeTwig, { props: {} });
    expect(container.querySelector(".tc-twig__elbow")).toBeInTheDocument();
  });

  it("keeps the elbow out of the accessibility tree", () => {
    const { container } = render(TreeTwig, { props: {} });
    expect(container.querySelector(".tc-twig__elbow")).toHaveAttribute("aria-hidden", "true");
  });

  it("shows the state on the rail", () => {
    const { getByRole } = render(TreeTwig, { props: { state: "working" } });
    expect(getByRole("img", { name: "Working" })).toBeInTheDocument();
  });

  it("has a compact form for a rank below a session", () => {
    const { container } = render(TreeTwig, { props: { compact: true } });
    expect(container.querySelector(".tc-twig")?.className).toContain("is-compact");
  });
});
