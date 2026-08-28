import { describe, expect, test } from "vitest";
import { Scroll } from "./scroll";

describe("Scroll", () => {
  // The fetch has to start before the reader arrives, or they hit the top and
  // wait for a round trip they could not see coming.
  test("asks for history before the reader reaches the top", () => {
    expect(Scroll.atTop(0)).toBe(true);
    expect(Scroll.atTop(Scroll.NEAR_TOP - 1)).toBe(true);
    expect(Scroll.atTop(Scroll.NEAR_TOP)).toBe(false);
    expect(Scroll.atTop(4000)).toBe(false);
  });

  // A box shorter than its own viewport never scrolls, so it is at the top and
  // at the bottom at once. Both answers have to be true or the reader is
  // stranded between them.
  test("a transcript shorter than the box is at the top and following at once", () => {
    expect(Scroll.atTop(0)).toBe(true);
    expect(Scroll.following(0, 400, 800)).toBe(true);
  });

  test("follows the tail only from within reach of it", () => {
    expect(Scroll.following(4000, 4800, 800)).toBe(true);
    expect(Scroll.following(4000 - Scroll.SLACK, 4800, 800)).toBe(false);
  });

  test("reading history is not following", () => {
    expect(Scroll.following(0, 9000, 800)).toBe(false);
  });
});
