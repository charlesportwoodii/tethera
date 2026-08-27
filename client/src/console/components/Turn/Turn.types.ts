import type { Role } from "$bindings/Role";

export interface TurnProps {
  role: Role;
  /** Already formatted. Formatting is a locale decision the system does not make. */
  time: string;
  /** Epoch millis from the wire's Timestamp. Optional. */
  at?: number | null;
  /** Lights the node — the turn that is waiting on you. */
  marked?: boolean;
}
