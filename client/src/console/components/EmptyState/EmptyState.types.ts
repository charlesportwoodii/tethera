import type { IconName } from "$console/components/Icon/Icon.types";

export interface EmptyStateProps {
  /** The mark in the disc. Omit for a wordless state that needs no picture. */
  icon?: IconName | null;
  title: string;
  /**
   * One or two sentences saying what would fill this screen and how.
   *
   * An empty screen is an invitation, not a report: "Nothing open here" alone
   * tells somebody what they can already see.
   */
  body?: string | null;
}
