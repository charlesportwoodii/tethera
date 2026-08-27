import type { FileMeta } from "$console/types/files";

export interface FileViewerProps {
  file: FileMeta;
  /** sheet anchors to the bottom edge (phone); modal centres (desktop). */
  anchor?: "sheet" | "modal";
  /** Tab captions. Omit for a single-view file. */
  tabs?: string[];
  activeTab?: string;
  /** Shown in place of a preview when there is nothing sensible to render. */
  noPreviewReason?: string | null;
  onselecttab?: (tab: string) => void;
  onclose?: () => void;
}
