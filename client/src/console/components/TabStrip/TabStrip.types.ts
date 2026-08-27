import type { Tab } from "$bindings/Tab";
import type { TabId } from "$bindings/TabId";
import type { GlyphState } from "$console/types/state";

export interface TabStripProps {
  /**
   * The workspace's tabs, from the wire.
   *
   * `Tab.index` is the backend's own ordinal, which is what a person means by
   * "2:build". A number assigned by list position would renumber every tab when
   * one closes, so the index is displayed, never derived.
   */
  tabs: Tab[];
  /** Null when nothing is open yet. */
  activeId?: TabId | null;
  /**
   * Agent state per tab, keyed by tab id. Absent for a tab with no agent — a
   * plain shell gets no glyph rather than an idle one.
   */
  states?: Record<string, GlyphState>;
  onselect?: (id: TabId) => void;
  /** Absent when the machine will not take another tab. */
  onadd?: (() => void) | null;
}
