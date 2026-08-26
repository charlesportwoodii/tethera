import type { IconName } from "$console/components/Icon/Icon.types";

/**
 * primary — the one filled thing on a screen. There should never be two.
 * quiet   — a destructive or secondary escape hatch. Reads as text.
 */
export type ButtonVariant = "primary" | "quiet";

export interface ButtonProps {
  variant?: ButtonVariant;
  icon?: IconName;
  disabled?: boolean;
  /** Passed straight through. Screens use "submit" inside a form. */
  type?: "button" | "submit";
  onclick?: (event: MouseEvent) => void;
}
