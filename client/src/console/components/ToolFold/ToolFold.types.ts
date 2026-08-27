import type { ToolStatus } from "$bindings/ToolStatus";

export interface ToolFoldProps {
  /** What ran: "Bash", a file path, a tool name from the transcript. */
  name: string;
  /** The one-line result: "2 hits", "+3 -1". */
  detail?: string | null;
  /**
   * Running, finished, or failed. A running tool is what makes the transcript
   * feel live, so it gets the spinner rather than a static mark.
   */
  status?: ToolStatus;
  expanded?: boolean;
  onclick?: () => void;
}
