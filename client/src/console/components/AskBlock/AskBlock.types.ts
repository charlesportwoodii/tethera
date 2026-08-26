/**
 * An option as the design shows it: a short label and a line explaining the
 * trade-off. The wire currently sends a bare string, so both are accepted and a
 * string is treated as a label with no detail. See types/README.md.
 */
export interface AskOption {
  label: string;
  detail?: string | null;
}

export interface AskBlockProps {
  prompt: string;
  options: Array<AskOption | string>;
  /** How long it has been waiting — already formatted. */
  waiting?: string | null;
  /**
   * Guard from the gateway: an answer is refused if the pane has moved on to a
   * different question. Passed straight back with the answer.
   */
  fingerprint?: string | null;
  onanswer?: (index: number, fingerprint: string | null) => void;
}
