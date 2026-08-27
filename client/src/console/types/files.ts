/** How a file should be shown once it is open. */
export type PreviewKind = "text" | "markdown" | "code" | "diff" | "image" | "none";

/**
 * A file as the transcript describes it. Mirrors `Part::file` plus the two things
 * the client adds: a formatted time, and the preview it decided on.
 */
export interface FileMeta {
  name: string;
  /** Null when the machine has not measured it. */
  size: number | bigint | null;
  mime?: string | null;
  preview?: PreviewKind;
  /** Already formatted. */
  at?: string | null;
}
