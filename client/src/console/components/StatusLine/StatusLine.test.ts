import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import StatusLine from "./StatusLine.svelte";

describe("StatusLine", () => {
  it("shows the label", () => {
    const { getByText } = render(StatusLine, { props: { label: "Compacted" } });
    expect(getByText("Compacted")).toBeInTheDocument();
  });

  it("shows the detail beside it", () => {
    const { getByText } = render(StatusLine, {
      props: { label: "Compacted", detail: "62k tokens reclaimed" },
    });
    expect(getByText("62k tokens reclaimed")).toBeInTheDocument();
  });

  it("omits the detail rather than leaving an empty span", () => {
    const { container } = render(StatusLine, { props: { label: "Compacted" } });
    expect(container.querySelector(".tc-status__detail")).toBeNull();
  });
});
