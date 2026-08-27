import type { AgentStatus } from "$bindings/AgentStatus";

/**
 * What a glyph can show. AgentStatus comes from the Rust common crate — including
 * `stalled`, which the wire now carries, so the client no longer invents it. The
 * rest are client-side conditions the server has no opinion about.
 *
 * - offline: the machine is not answering, so its agents have no known state.
 * - set / unset: a form field, which uses the same marks so a screen has one
 *   vocabulary rather than two.
 */
export type GlyphState = AgentStatus | "offline" | "set" | "unset";

/** How much of the screen the pane drawer is taking. */
export type DrawerHeight = "peek" | "half" | "full";

/** Which glyphs mean the machine is dealing with it, and which mean you are. */
export const BLOCKING: readonly GlyphState[] = ["blocked", "unset"];

export function isBlocking(state: GlyphState): boolean {
  return BLOCKING.includes(state);
}
