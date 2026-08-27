import type { PreviewKind } from "$console/types/files";

export interface FilePreviewProps {
  name: string;
  /** From the fetch head. Decides how the bytes are read. */
  mime?: string | null;
  /**
   * The first N bytes, already decoded as UTF-8 text. Null while it is still
   * arriving, which is a different state from "there is nothing to show".
   */
  text?: string | null;
  /** An object or data URL for an image preview. */
  imageUrl?: string | null;
  /**
   * True when the fetch stopped early. A preview reads the head of a file and
   * drops the stream; saying so is the difference between a short file and a
   * long one shown short.
   */
  truncated?: boolean;
  /** Overrides the decision made from `mime`. */
  kind?: PreviewKind;
}
