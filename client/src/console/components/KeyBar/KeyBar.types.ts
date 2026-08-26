export interface KeyBarProps {
  /** Rows of key captions. What is sent is the caption itself. */
  rows?: string[][];
  onkey?: (key: string) => void;
}

/** The keys a terminal on a phone cannot do without. */
export const DEFAULT_KEYS: string[][] = [
  ["esc", "tab", "ctrl", "alt", "\u2191", "\u2193", "\u2190", "\u2192"],
  ["^C", "^D", "^L", "/", "-", "|"],
];
