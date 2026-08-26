import type { TurnRole } from "$console/types/state";

export interface TurnProps {
  role: TurnRole;
  /** Already formatted. Formatting is a locale decision the system does not make. */
  time: string;
  /** Epoch millis, for the machine-readable datetime. Optional. */
  at?: number | null;
  /** Lights the node — the turn that is waiting on you. */
  marked?: boolean;
}
