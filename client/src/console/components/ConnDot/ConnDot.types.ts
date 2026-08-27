import type { LinkKind } from "$bindings/LinkKind";

export interface ConnDotProps {
  link: LinkKind;
  /** Null until a path has settled. Absent is not zero. */
  rttMs?: number | null;
  /** Shown instead of the round trip when offline — "2d", "Saturday, 21:04". */
  lastSeen?: string | null;
  /** Appended after the route and the figure. Used for the transcript tier. */
  note?: string | null;
}
