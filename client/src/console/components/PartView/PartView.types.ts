import type { Part } from "$bindings/Part";

export interface PartViewProps {
  part: Part;
  /** Already formatted; only used by the file part. */
  at?: string | null;
  waiting?: string | null;
  fingerprint?: string | null;
  onanswer?: (index: number, fingerprint: string | null) => void;
  ondownload?: (name: string) => void;
  ontool?: (name: string) => void;
}
