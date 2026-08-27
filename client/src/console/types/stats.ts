/**
 * What a live agent is costing and how far through its window it is.
 *
 * Not on the wire yet. Claude Code knows every field; the gateway has to carry
 * this as a live per-pane record rather than as transcript parts, or the numbers
 * only move when a message lands and the row is a spinner with decoration.
 * See types/README.md.
 */
export interface AgentStats {
  /** Seconds since the turn began. */
  elapsedSeconds: number;
  tokensIn: number;
  tokensOut: number;
  /** Tool calls this turn. */
  tools: number;
  /** Tokens of context used, and the model's window. */
  contextUsed?: number | null;
  contextWindow?: number | null;
  /** Shown only where there is room — desktop. */
  model?: string | null;
  /** USD for this turn. Omitted rather than guessed. */
  costUsd?: number | null;
}
