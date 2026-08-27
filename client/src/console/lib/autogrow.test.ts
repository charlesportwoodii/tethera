import { describe, expect, it } from "vitest";
import { growHeight } from "./autogrow";

const metrics = (over: Partial<Parameters<typeof growHeight>[0]> = {}) => ({
  contentHeight: 20,
  lineHeight: 18,
  chrome: 22,
  maxRows: 5,
  ...over,
});

describe("growHeight", () => {
  it("shows one row when there is one row of content", () => {
    const { height, scrolls } = growHeight(metrics({ contentHeight: 40 }));
    expect(height).toBe(40);
    expect(scrolls).toBe(false);
  });

  it("grows with the content", () => {
    expect(growHeight(metrics({ contentHeight: 76 })).height).toBe(76);
  });

  it("stops at the cap", () => {
    // 18 × 5 + 22 = 112.
    const { height, scrolls } = growHeight(metrics({ contentHeight: 400 }));
    expect(height).toBe(112);
    expect(scrolls).toBe(true);
  });

  it("scrolls only once it is actually capped", () => {
    expect(growHeight(metrics({ contentHeight: 112 })).scrolls).toBe(false);
    expect(growHeight(metrics({ contentHeight: 113 })).scrolls).toBe(true);
  });

  it("never collapses below one row", () => {
    // A field that has not been laid out reports nonsense; one row is the floor.
    const { height } = growHeight(metrics({ contentHeight: 0 }));
    expect(height).toBe(40);
  });

  it("honours a different cap", () => {
    expect(growHeight(metrics({ contentHeight: 400, maxRows: 2 })).height).toBe(58);
  });

  it("falls back to a sane line height when the computed one is unreadable", () => {
    // getComputedStyle reports "normal" for an unstyled line-height, which parses
    // as NaN; the measurer passes 0 and this keeps the arithmetic finite.
    const { height } = growHeight(metrics({ lineHeight: 0, contentHeight: 999, maxRows: 4 }));
    expect(Number.isFinite(height)).toBe(true);
    expect(height).toBe(18 * 4 + 22);
  });

  it("treats a zero cap as one row rather than as no rows", () => {
    expect(growHeight(metrics({ contentHeight: 999, maxRows: 0 })).height).toBe(40);
  });
});
