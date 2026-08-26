export interface NavBarProps {
  title: string;
  /** The mono line under the title: route, ids, counts. */
  subtitle?: string | null;
  /** Absent on the root screen, which is how you know it is the root. */
  onback?: (() => void) | null;
  backLabel?: string;
}
