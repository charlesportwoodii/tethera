export interface AttachChipProps {
  name: string;
  /**
   * Upload progress, 0 to 1. Null once it has landed.
   *
   * A megabyte over a relayed path is not instant, and a chip that claims the
   * file is attached before it is loses the file.
   */
  progress?: number | null;
  onremove?: (() => void) | null;
}
