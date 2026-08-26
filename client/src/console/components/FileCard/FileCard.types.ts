export interface FileCardProps {
  name: string;
  /** Bytes. bigint because that is what the Rust side sends. */
  size: number | bigint;
  /** Already formatted. */
  at?: string | null;
  ondownload?: () => void;
}
