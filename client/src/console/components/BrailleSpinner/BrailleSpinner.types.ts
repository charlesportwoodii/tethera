export interface BrailleSpinnerProps {
  size?: number;
  /**
   * Starting frame. Give each spinner on a screen a different offset so two
   * agents working at once do not appear to be in lockstep.
   */
  offset?: number;
  /** Milliseconds per frame. */
  interval?: number;
  /** Accessible name. Pass null for a spinner beside text that already says it. */
  label?: string | null;
}
