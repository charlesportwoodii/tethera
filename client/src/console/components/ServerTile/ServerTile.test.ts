import { describe, expect, it, vi } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";
import ServerTile from "./ServerTile.svelte";
import type { GlyphState } from "$console/types/state";
import type { ServerTileProps } from "./ServerTile.types";

function props(over: Partial<ServerTileProps> = {}): ServerTileProps {
  const states: GlyphState[] = ["blocked", "working", "idle"];

  return {
    label: "noble2",
    os: "linux",
    arch: "aarch64",
    link: "direct",
    rttMs: 38,
    states,
    summary: "1 needs you · 1 working · 1 idle",
    ...over,
  };
}

describe("ServerTile", () => {
  // The whole card is the way in, not the two lines of text at the top of it.
  // A tile is ~250px tall and the name block is ~50px of that, so a target
  // drawn round the text alone leaves most of the card dead — and a card that
  // does nothing when tapped reads as broken rather than as decorative.
  it("makes the whole card the way in, not just its title", () => {
    const { getByRole } = render(ServerTile, { props: props() });
    const open = getByRole("button", { name: "Open noble2" });

    // Empty, because it lies under the content rather than wrapping it. A
    // button that wrapped the tile could not contain the + button inside it.
    expect(open.textContent?.trim()).toBe("");
    expect(open.contains(document.querySelector(".tc-tile__name"))).toBe(false);
  });

  it("names the machine and what it is", () => {
    const { getByText } = render(ServerTile, { props: props() });

    expect(getByText("noble2")).toBeInTheDocument();
    expect(getByText("linux · aarch64")).toBeInTheDocument();
  });

  it("draws a mark per session and the sentence the caller wrote", () => {
    const { container, getByText } = render(ServerTile, { props: props() });

    expect(container.querySelectorAll(".tc-glyph")).toHaveLength(3);
    expect(getByText("1 needs you · 1 working · 1 idle")).toBeInTheDocument();
  });

  // The ring is the only thing that tells you which machine to look at before
  // you have read a word, so it is asserted in the DOM rather than in the text.
  it("marks the tile when something is waiting on a person", () => {
    const { container } = render(ServerTile, { props: props({ attention: true }) });

    expect(container.querySelector(".tc-tile")).toHaveAttribute("data-attention", "true");
  });

  it("does not mark a tile where nothing is waiting", () => {
    const states: GlyphState[] = ["working", "idle"];
    const { container } = render(ServerTile, { props: props({ states, attention: false }) });

    expect(container.querySelector(".tc-tile")).toHaveAttribute("data-attention", "false");
  });

  it("shows when a quiet machine was last seen instead of a round trip", () => {
    const { getByText } = render(ServerTile, {
      props: props({ states: [], link: "offline", rttMs: 38, lastSeen: "2d" }),
    });

    // The stale figure must not survive: it would read as a live measurement.
    expect(getByText("no route · 2d")).toBeInTheDocument();
  });

  it("says a refusal in its own words rather than as a dead route", () => {
    const { getByText, container } = render(ServerTile, {
      props: props({ states: [], refusal: "would not accept this device" }),
    });

    expect(getByText("would not accept this device")).toBeInTheDocument();
    // Both at once reads as a machine that is unreachable and rude, which is
    // one fact too many and the wrong one.
    expect(container.querySelector(".tc-conn")).toBeNull();
  });

  it("invites a first session on a machine running nothing", () => {
    const { getByText } = render(ServerTile, { props: props({ states: [], summary: "" }) });

    expect(getByText("Nothing running")).toBeInTheDocument();
  });

  it("opens the machine and starts a session through separate callbacks", async () => {
    const onopen = vi.fn();
    const onstart = vi.fn();
    const { getByRole, getByLabelText } = render(ServerTile, {
      props: props({ onopen, onstart }),
    });

    await fireEvent.click(getByRole("button", { name: "Open noble2" }));
    expect(onopen).toHaveBeenCalledOnce();
    expect(onstart).not.toHaveBeenCalled();

    await fireEvent.click(getByLabelText("New session on noble2"));
    expect(onstart).toHaveBeenCalledOnce();
    expect(onopen).toHaveBeenCalledOnce();
  });
});
