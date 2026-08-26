import type { DrawerHeight } from "$console/types/state";

export interface DrawerProps {
  height?: DrawerHeight;
  /** The tab the drawer is showing — "2:build". */
  label: string;
  /** One line of what that pane is doing, shown while it is only peeking. */
  summary?: string | null;
  onheight?: (next: DrawerHeight) => void;
}
