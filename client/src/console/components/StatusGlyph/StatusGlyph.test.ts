import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import StatusGlyph from "./StatusGlyph.svelte";
import type { GlyphState } from "$console/types/state";

const ALL: GlyphState[] = ["working", "idle", "done", "blocked", "offline", "set", "unset"];

describe("StatusGlyph", () => {
  it("names every state for assistive tech", () => {
    for (const state of ALL) {
      const { getByRole, unmount } = render(StatusGlyph, { props: { state } });
      expect(getByRole("img").getAttribute("aria-label")).toBeTruthy();
      unmount();
    }
  });

  it("carries the state as data so a parent can style by it", () => {
    const { getByRole } = render(StatusGlyph, { props: { state: "blocked" } });
    expect(getByRole("img")).toHaveAttribute("data-state", "blocked");
  });

  it("distinguishes blocked from every other state by class, not colour alone", () => {
    const { getByRole } = render(StatusGlyph, { props: { state: "blocked" } });
    expect(getByRole("img").className).toContain("is-blocked");
  });

  it("scales the box and the mark together", () => {
    const { getByRole } = render(StatusGlyph, { props: { state: "done", size: 24 } });
    const el = getByRole("img") as HTMLElement;
    expect(el.style.width).toBe("24px");
    // The wedge is em-sized, so font-size has to track the box or it stays 14px.
    expect(el.style.fontSize).toBe("24px");
  });

  it("takes a background so it can punch the rail out behind itself", () => {
    const { getByRole } = render(StatusGlyph, {
      props: { state: "idle", bg: "rgb(1, 2, 3)" },
    });
    expect((getByRole("img") as HTMLElement).style.background).toBe("rgb(1, 2, 3)");
  });
});
