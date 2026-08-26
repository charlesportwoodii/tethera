import type { GlyphState } from "$console/types/state";

export interface StatusGlyphProps {
  state: GlyphState;
  /** Edge length of the box the mark sits in. The mark scales with it. */
  size?: number;
  /**
   * Surface the glyph is drawn on. Only matters on the rail, where the mark
   * punches the trunk out behind itself.
   */
  bg?: string;
}
