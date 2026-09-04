import { describe, expect, it, vi } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";
import SwipeRow from "./SwipeRow.svelte";

/**
 * A pointer drag, in the three events the component listens for.
 *
 * The width is stamped on the node because a detached element measures zero,
 * and the threshold is a fraction of the row's width.
 */
async function drag(track: Element, from: number, to: number): Promise<void> {
  Object.defineProperty(track, "clientWidth", { value: 300, configurable: true });

  await fireEvent.pointerDown(track, { clientX: from, pointerId: 1 });
  await fireEvent.pointerMove(track, { clientX: to, pointerId: 1 });
  await fireEvent.pointerUp(track, { clientX: to, pointerId: 1 });
}

function trackOf(container: Element): Element {
  const track = container.querySelector(".tc-swipe__track");

  if (track === null) {
    throw new Error("the row drew no track");
  }

  return track;
}

describe("SwipeRow", () => {
  it("fires when the drag passes the threshold", async () => {
    const onaction = vi.fn();
    const { container } = render(SwipeRow, {
      props: { action: "Release", onaction, threshold: 0.3 },
    });

    await drag(trackOf(container), 280, 100);

    expect(onaction).toHaveBeenCalledOnce();
  });

  // A row that fired on any movement would fire while somebody scrolled a list.
  it("does not fire on a drag that falls short", async () => {
    const onaction = vi.fn();
    const { container } = render(SwipeRow, {
      props: { action: "Release", onaction, threshold: 0.3 },
    });

    await drag(trackOf(container), 280, 250);

    expect(onaction).not.toHaveBeenCalled();
  });

  it("settles closed after a drag that falls short", async () => {
    const { container } = render(SwipeRow, {
      props: { action: "Release", threshold: 0.3 },
    });

    await drag(trackOf(container), 280, 250);

    expect(container.querySelector(".tc-swipe")).toHaveAttribute("data-open", "false");
  });

  // The gesture is not the capability. A pointer-only action is unreachable by
  // keyboard and invisible to a screen reader, which on a control that looks
  // destructive is the difference between an affordance and a trap.
  it("offers the same action as a real button", async () => {
    const onaction = vi.fn();
    const { getByRole } = render(SwipeRow, { props: { action: "Release", onaction } });

    await fireEvent.click(getByRole("button", { name: "Release" }));

    expect(onaction).toHaveBeenCalledOnce();
  });

  it("ignores the gesture entirely when disabled", async () => {
    const onaction = vi.fn();
    const { container } = render(SwipeRow, {
      props: { action: "Release", onaction, enabled: false, threshold: 0.3 },
    });

    await drag(trackOf(container), 280, 20);

    expect(onaction).not.toHaveBeenCalled();
  });

  it("draws no action bed when disabled", () => {
    const { container } = render(SwipeRow, {
      props: { action: "Release", enabled: false },
    });

    expect(container.querySelector(".tc-swipe__bed")).toBeNull();
  });
});
