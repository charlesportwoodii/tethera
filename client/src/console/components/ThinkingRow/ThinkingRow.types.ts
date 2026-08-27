import type { AgentStats } from "$console/types/stats";

export interface ThinkingRowProps {
  stats: AgentStats;
  /** The in-flight tool call — "Reading src/lib/deeplink.ts". */
  activity?: string | null;
  /** What the agent is doing. "Thinking" unless the harness says otherwise. */
  verb?: string;
  /** One row instead of three, with model and cost. For desktop. */
  dense?: boolean;
  /** Spinner offset, so two rows on one screen are out of phase. */
  offset?: number;
  onstop?: (() => void) | null;
}
