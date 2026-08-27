export interface FileCardProps {
  name: string;
  /** Bytes. Null when the machine has not measured it. */
  size: number | bigint | null;
  /** Already formatted. */
  at?: string | null;
  ondownload?: () => void;
}
