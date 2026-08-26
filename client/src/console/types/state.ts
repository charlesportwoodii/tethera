import type { AgentStatus } from "$bindings/AgentStatus";

/**
 * What a glyph can show. AgentStatus comes from the Rust common crate; the
 * extra members are client-side conditions the server has no opinion about.
 *
 * - offline: the server is not answering, so its agents have no known state.
 * - set / unset: a form field, which uses the same marks so a screen has one
 *   vocabulary rather than two.
 */
export type GlyphState = AgentStatus | "offline" | "set" | "unset";

/**
 * How the phone reached a server.
 *
 * Not in the bindings yet. The gateway knows this — it is the difference between
 * a hole-punched QUIC path and one carried by the relay — but the wire contract
 * has no field for it. Until it does, the client decides from the Iroh endpoint
 * and this type should move to the common crate.
 */
export type LinkState = "direct" | "relayed" | "offline";

/** Who produced a turn. Also absent from TranscriptEntry — see types/README. */
export type TurnRole = "you" | "agent";

/** How much of the screen the pane drawer is taking. */
export type DrawerHeight = "peek" | "half" | "full";

/** Which glyphs mean the machine is dealing with it, and which mean you are. */
export const BLOCKING: readonly GlyphState[] = ["blocked", "unset"];

export function isBlocking(state: GlyphState): boolean {
  return BLOCKING.includes(state);
}
