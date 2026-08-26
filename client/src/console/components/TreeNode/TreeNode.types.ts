import type { GlyphState } from "$console/types/state";

export interface TreeNodeProps {
  /** The mark on the rail. Omit to supply your own through the glyph snippet. */
  state?: GlyphState;
  /**
   * Draw the trunk. True whenever the node has twigs under it — the trunk is
   * what they hang from, so a childless node that draws one ends in mid-air.
   */
  branches?: boolean;
  /** Remembered rather than live: dimmed, and nothing inside it is actionable. */
  dim?: boolean;
  /** Extra space above, for every node after the first in a tree. */
  spaced?: boolean;
}
