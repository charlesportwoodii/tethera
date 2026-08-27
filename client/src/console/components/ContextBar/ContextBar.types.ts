export interface ContextBarProps {
  /** Tokens of context in use. */
  used: number;
  /** The model's window. */
  window: number;
  /** Fraction above which the bar warns. */
  warnAt?: number;
  /** Hide the figures and show the bar alone, for a tight row. */
  bare?: boolean;
}
