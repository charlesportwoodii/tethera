import type { PaneId } from "$bindings/PaneId";
import type { SplitDirection } from "$bindings/SplitDirection";

/**
 * One pane's place in a tab's layout.
 *
 * Geometry is normalised 0..1 and supplied by the caller, **not derived here.**
 *
 * The wire does not carry pane positions: `Pane` has `tab_id`, `size` and
 * `focused`, and no coordinates or split tree. Sizes alone can suggest an
 * arrangement — equal `rows` hints side-by-side — but that guess breaks on
 * nested splits, and a layout diagram that is subtly wrong is worse than none.
 * So the component draws what it is told and the gap stays visible.
 */
export interface PaneBox {
  id: PaneId;
  /** What the person calls it. Usually the foreground command. */
  label: string;
  /** Fractions of the tab's width and height. */
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface PaneMapProps {
  panes: PaneBox[];
  /** The pane being viewed. The phone shows one at a time. */
  activeId?: PaneId | null;
  onselect?: (id: PaneId) => void;
  /**
   * Absent when the machine will not split. Splitting from a phone costs the
   * pane you are about to read half its columns, so it lives here rather than in
   * the main chrome — available, not prominent.
   */
  onsplit?: ((direction: SplitDirection) => void) | null;
}
