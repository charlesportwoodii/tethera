export interface ToggleProps {
  checked?: boolean;
  /** The accessible name. Required: a switch with no name is unusable. */
  label: string;
  disabled?: boolean;
  onchange?: (checked: boolean) => void;
}
