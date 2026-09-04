import type { Snippet } from "svelte";
import type { IconName } from "$console/components/Icon/Icon.types";

export interface SwipeRowProps {
  /** What the revealed action is called, in the bed and to a screen reader. */
  action: string;
  icon?: IconName;
  enabled?: boolean;
  /**
   * How far across the row the drag must travel before it counts, as a fraction
   * of the row's width. Below this the row springs back: a list is scrolled far
   * more often than a row is swiped, and a low threshold turns every scroll
   * that starts slightly sideways into an action.
   */
  threshold?: number;
  onaction?: (() => void) | null;
  children?: Snippet;
}
