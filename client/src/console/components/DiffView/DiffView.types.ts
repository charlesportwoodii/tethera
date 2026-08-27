export interface DiffViewProps {
  path: string;
  /** Unified diff, verbatim from the agent's own record. */
  unified: string;
  added?: number | null;
  removed?: number | null;
  /** Collapsed to a header until opened. */
  open?: boolean;
  ontoggle?: () => void;
}
