export interface ChipProps {
  label: string;
  /** Rides on the chip in mono — a version, a count. */
  detail?: string | null;
  selected?: boolean;
  disabled?: boolean;
  onclick?: (event: MouseEvent) => void;
}
