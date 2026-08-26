/**
 * The colours a pane line can take. Deliberately a small named set rather than
 * 256 ANSI codes: this component renders what the gateway has already normalised.
 * A real ANSI parser belongs upstream of it, not inside it.
 */
export type TermTone = "plain" | "dim" | "ok" | "accent" | "warn" | "attn";

export interface TermLine {
  text: string;
  tone?: TermTone;
}

export interface TerminalViewProps {
  lines: TermLine[];
  /** Draw the block cursor after the last line. */
  cursor?: boolean;
  /** Reported in the drawer head — "80x24". */
  label?: string;
}
