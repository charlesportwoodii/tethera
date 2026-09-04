import type { Snippet } from "svelte";
import type { GlyphState } from "$console/types/state";

export interface StatusStripProps {
  /**
   * One entry per session, in the order the caller wants them read. Sorting is
   * the caller's job: the sentence beside the strip is written from the same
   * array, and a strip that reordered would disagree with it.
   */
  states: GlyphState[];
  /** How many marks are drawn before the rest become a figure. */
  cap?: number;
  /** Edge length of each mark's box. */
  size?: number;
  /**
   * How one mark is drawn, given its state and size.
   *
   * Defaults to the console's own `StatusGlyph`. It is a prop because an app may
   * already draw these states its own way, and a strip that disagreed with the
   * rows beneath it would put the same session in two shapes on one screen.
   */
  glyph?: Snippet<[GlyphState, number]>;
}
