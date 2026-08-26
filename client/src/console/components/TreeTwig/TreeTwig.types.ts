import type { GlyphState } from "$console/types/state";

export interface TreeTwigProps {
  state?: GlyphState;
  /** Tighter twig, for a tab under a workspace rather than a session under a server. */
  compact?: boolean;
}
