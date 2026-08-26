export interface ComposerProps {
  value?: string;
  placeholder?: string;
  disabled?: boolean;
  /** Absent when the host cannot take uploads — the control disappears rather than greying. */
  onattach?: (() => void) | null;
  oninput?: (value: string) => void;
  onsend?: (value: string) => void;
}
