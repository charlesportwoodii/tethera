import type { Snippet } from "svelte";
import type { GlyphState } from "$console/types/state";
import type { LinkKind } from "$bindings/LinkKind";

export interface ServerTileProps {
  label: string;
  os: string;
  arch: string;
  link: LinkKind;
  rttMs?: number | null;
  /** "2d". Shown in place of a round trip once the machine has gone quiet. */
  lastSeen?: string | null;
  /**
   * Set when the machine answered and turned this device away. A refusal is a
   * different sentence from no route, and it replaces the route line entirely:
   * the network is working, so nothing should send somebody to debug it.
   */
  refusal?: string | null;
  states: GlyphState[];
  /**
   * The sentence under the strip, already written. The tile does not count,
   * because the counting depends on what the sweep managed to fetch and that is
   * a question about the wire, not about a tile.
   */
  summary: string;
  attention?: boolean;
  /**
   * How one mark on the strip is drawn. Forwarded to `StatusStrip`, which falls
   * back to the console's own `StatusGlyph`. Supply the app's mark where it has
   * one, so a tile and the rows under it do not draw one session two ways.
   */
  glyph?: Snippet<[GlyphState, number]>;
  onopen?: (() => void) | null;
  onstart?: (() => void) | null;
}
