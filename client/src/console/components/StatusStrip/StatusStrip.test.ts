import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import StatusStrip from "./StatusStrip.svelte";
import type { GlyphState } from "$console/types/state";

describe("StatusStrip", () => {
  it("draws one mark per state", () => {
    const states: GlyphState[] = ["working", "blocked", "idle"];
    const { container } = render(StatusStrip, { props: { states } });

    expect(container.querySelectorAll(".tc-glyph")).toHaveLength(3);
  });

  it("counts the ones it did not draw rather than dropping them", () => {
    const states: GlyphState[] = Array.from({ length: 11 }, () => "idle");
    const { container, getByText } = render(StatusStrip, { props: { states, cap: 8 } });

    expect(container.querySelectorAll(".tc-glyph")).toHaveLength(8);
    expect(getByText("+3")).toBeInTheDocument();
  });

  it("says nothing about overflow when everything fits", () => {
    const states: GlyphState[] = ["working", "idle"];
    const { container } = render(StatusStrip, { props: { states, cap: 8 } });

    expect(container.querySelector(".tc-strip__more")).toBeNull();
  });

  // The order is the caller's. A strip that sorted would disagree with the
  // sentence under it, which is written from the same array.
  it("keeps the order it was given", () => {
    const states: GlyphState[] = ["idle", "blocked", "working"];
    const { container } = render(StatusStrip, { props: { states } });

    const drawn = [...container.querySelectorAll(".tc-glyph")].map((node) =>
      node.getAttribute("data-state"),
    );

    expect(drawn).toEqual(["idle", "blocked", "working"]);
  });

  it("renders nothing at all for a machine with no sessions", () => {
    const { container } = render(StatusStrip, { props: { states: [] } });

    expect(container.querySelector(".tc-strip")).toBeNull();
  });
});

// The console's own mark is a default, not a rule. An app that already draws
// these states its own way must be able to say so, or a strip disagrees with
// the rows beneath it and one session appears in two shapes on one screen.
describe("StatusStrip glyph override", () => {
  it("draws the caller's mark instead of the console's when one is given", async () => {
    const Harness = (await import("./StatusStripHarness.test.svelte")).default;
    const { container } = render(Harness, {
      props: { states: ["idle", "working"] as GlyphState[] },
    });

    expect(container.querySelectorAll(".mine")).toHaveLength(2);
    expect(container.querySelector(".tc-glyph")).toBeNull();
  });
});
