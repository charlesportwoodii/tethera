import type { Key } from "$bindings/Key";
import type { Mods } from "$bindings/Mods";
import type { PaneId } from "$bindings/PaneId";
import type { SplitDirection } from "$bindings/SplitDirection";
import type { Tab } from "$bindings/Tab";
import type { TabId } from "$bindings/TabId";
import type { KeyCap } from "$console/components/KeyBar/KeyBar.types";
import type { PaneBox } from "$console/components/PaneMap/PaneMap.types";
import type { TerminalGrid } from "$console/lib/terminal";
import type { GlyphState } from "$console/types/state";

/**
 * The terminal side of a workspace, assembled.
 *
 * Tabs, the pane map, the screen and the key bar as one block. It fetches
 * nothing and holds no selection: which tab and which pane are props, because
 * the caller is the one that can act on a change — opening a tab is an RPC, and
 * a component that decided locally would show a tab the machine does not have.
 */
export interface TerminalPaneProps {
  tabs: Tab[];
  /** Null before the first tab is chosen, which is also true when there are none. */
  activeTabId?: TabId | null;
  /** Agent state per tab id. A tab running a plain shell has none. */
  tabStates?: Record<string, GlyphState>;

  /**
   * The panes of the active tab, with geometry supplied by the caller.
   *
   * One entry, or none, means there is nothing to map and the strip is absent.
   * The wire carries no pane positions — see PaneMap.types.
   */
  panes?: PaneBox[];
  activePaneId?: PaneId | null;

  /** The active pane's screen. */
  grid: TerminalGrid;
  /** Bump after applying frames; the grid mutates in place. */
  revision?: number;

  keys?: KeyCap[][];

  onselecttab?: (id: TabId) => void;
  /** Absent when the machine will not take another tab. */
  onaddtab?: (() => void) | null;
  onselectpane?: (id: PaneId) => void;
  /** Absent when the machine will not split. */
  onsplit?: ((direction: SplitDirection) => void) | null;
  onkey?: (key: Key, mods: Mods) => void;
  onfocuspane?: (() => void) | null;

  /** Overrides the copy shown when the workspace has no tabs. */
  emptyTitle?: string;
  emptyBody?: string;
  /** Names the machine in the empty state's action — "New tab on atlas". */
  machine?: string | null;
}
