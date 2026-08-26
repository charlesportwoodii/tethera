export interface ToolFoldProps {
  /** What ran: "Bash", a file path, a tool name from the transcript. */
  name: string;
  /** The one-line result: "2 hits", "+3 -1". */
  detail?: string | null;
  /** Colours the detail. ok for a diff that landed, attn for one that failed. */
  tone?: "muted" | "ok" | "attn";
  expanded?: boolean;
  onclick?: () => void;
}
