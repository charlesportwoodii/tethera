import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import TreeNode from "./TreeNode.svelte";

describe("TreeNode", () => {
  it("draws no trunk by default, so a childless node does not end in mid-air", () => {
    const { container } = render(TreeNode, { props: { state: "idle" } });
    expect(container.querySelector(".tc-node__trunk")).toBeNull();
    expect(container.querySelector(".tc-node")).toHaveAttribute("data-branches", "false");
  });

  it("draws the trunk when it has twigs to carry", () => {
    const { container } = render(TreeNode, { props: { state: "idle", branches: true } });
    expect(container.querySelector(".tc-node__trunk")).toBeInTheDocument();
  });

  it("hides the trunk from assistive tech — it is a line, not content", () => {
    const { container } = render(TreeNode, { props: { branches: true } });
    expect(container.querySelector(".tc-node__trunk")).toHaveAttribute("aria-hidden", "true");
  });

  it("renders the state glyph on the rail", () => {
    const { getByRole } = render(TreeNode, { props: { state: "blocked" } });
    expect(getByRole("img", { name: "Waiting on you" })).toBeInTheDocument();
  });

  it("renders no glyph at all when given neither a state nor a snippet", () => {
    const { queryByRole } = render(TreeNode, { props: {} });
    expect(queryByRole("img")).toBeNull();
  });

  it("is a list item, so a tree announces its length", () => {
    const { getByRole } = render(TreeNode, { props: {} });
    expect(getByRole("listitem")).toBeInTheDocument();
  });
});
