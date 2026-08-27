import type { TerminalGrid } from "$console/lib/terminal";

export interface TerminalViewProps {
  /**
   * The pane's screen.
   *
   * The grid is owned by the caller, because damage frames only make sense
   * against what came before and a component recreated on every prop change
   * cannot hold that. Pass the same instance and bump `revision` when it changes.
   */
  grid: TerminalGrid;
  /**
   * Increment after applying frames. The grid mutates in place, so there is
   * nothing for Svelte to compare — this is the signal to repaint.
   */
  revision?: number;
  /** Reported in the drawer head — "80×24". */
  label?: string;
  /** Called when the pane is tapped, so a host can raise a keyboard. */
  onfocus?: (() => void) | null;
}
