export interface CodeSlotsProps {
  /** What has been typed so far. Shorter than length; never longer. */
  value: string;
  /** How many digits the machine is showing. */
  length?: number;
  /** Accessible name for the group. */
  label?: string;
}
