import type { LinkState } from "$console/types/state";

export interface ConnDotProps {
  link: LinkState;
  /** Round trip in milliseconds. Omitted when there is no path to measure. */
  rttMs?: number | null;
  /** Shown instead of the round trip when offline — "2d", "Saturday, 21:04". */
  lastSeen?: string | null;
  /** Appended after the route and the figure. Used for the transcript tier. */
  note?: string | null;
}
