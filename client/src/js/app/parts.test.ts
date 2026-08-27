import { describe, expect, test } from "vitest";
import { Parts } from "./parts";

/**
 * A conversation row shows one line, so a preview cannot render markdown — and
 * showing the source instead is the one place a person reads the syntax rather
 * than the words. Every case here is one that reached a screen.
 */
describe("Parts.plain", () => {
  test("strips a heading marker, which the inline parser never sees", () => {
    expect(Parts.plain("## What the message means")).toBe("What the message means");
  });

  test("strips a code span nested inside bold", () => {
    // The inline parser captures the body of a strong span as raw text rather
    // than recursing, so a single pass leaves the inner backticks behind. This
    // exact string reached a device.
    expect(Parts.plain("**304 tests passing, `svelte-check` clean**")).toBe(
      "304 tests passing, svelte-check clean",
    );
  });

  test("collapses a list to one line", () => {
    expect(Parts.plain("- first\n- second")).toBe("first second");
  });

  test("keeps the text of a link and drops its target", () => {
    expect(Parts.plain("see [the docs](https://example.com)")).toBe("see the docs");
  });

  test("leaves plain prose untouched", () => {
    expect(Parts.plain("Hi — what would you like to work on?")).toBe(
      "Hi — what would you like to work on?",
    );
  });

  test("an absent preview is an empty string rather than a crash", () => {
    expect(Parts.plain(null)).toBe("");
  });

  test("a fenced block keeps its contents and loses its fence", () => {
    expect(Parts.plain("```sh\nyarn build\n```")).toBe("yarn build");
  });
});
