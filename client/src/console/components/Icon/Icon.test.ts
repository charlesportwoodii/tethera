import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import Icon from "./Icon.svelte";

describe("Icon", () => {
  it("is hidden from assistive tech when it has no label", () => {
    const { container } = render(Icon, { props: { name: "back" } });
    const svg = container.querySelector("svg");
    expect(svg).toHaveAttribute("aria-hidden", "true");
  });

  it("becomes an image with an accessible name when labelled", () => {
    const { getByRole } = render(Icon, { props: { name: "send", label: "Send" } });
    expect(getByRole("img", { name: "Send" })).toBeInTheDocument();
  });

  it("scales through width and height, not a transform", () => {
    const { container } = render(Icon, { props: { name: "plus", size: 24 } });
    const svg = container.querySelector("svg");
    expect(svg).toHaveAttribute("width", "24");
    expect(svg).toHaveAttribute("viewBox", "0 0 20 20");
  });

  it("renders nothing rather than throwing for an unknown name", () => {
    // @ts-expect-error deliberately outside IconName
    const { container } = render(Icon, { props: { name: "nope" } });
    expect(container.querySelectorAll("path")).toHaveLength(0);
  });
});
