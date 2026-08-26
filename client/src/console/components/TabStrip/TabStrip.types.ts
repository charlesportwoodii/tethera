import type { GlyphState } from "$console/types/state";

export interface PaneTab {
  id: string;
  /** As herdr names it: "1:claude". */
  label: string;
  /** Present only when an agent is attached to that pane. */
  state?: GlyphState;
}

export interface TabStripProps {
  tabs: PaneTab[];
  activeId: string;
  onselect?: (id: string) => void;
  /** Absent when the host will not take a new tab. */
  onadd?: (() => void) | null;
}
