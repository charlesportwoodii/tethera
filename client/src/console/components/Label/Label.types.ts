/**
 * section — the word above a group. Aligns to the rail, not the screen edge.
 * field   — the name of a value in a form node.
 */
export type LabelKind = "section" | "field";

/**
 * quiet  — the default. A word above a group.
 * urgent — the band is asking for something, and the word says so before the
 *          rows under it are read.
 */
export type LabelTone = "quiet" | "urgent";

export interface LabelProps {
  kind?: LabelKind;
  /** Section words align to the gutter by default; a flush one starts at the edge. */
  flush?: boolean;
  /**
   * Carry a hairline across the rest of the line.
   *
   * What separates the top of a band from a caption floating above unrelated
   * rows. Off by default, so a label that only names a value stays a word.
   */
  rule?: boolean;
  /**
   * A figure at the end of the line — how many rows the band holds.
   *
   * `0` is a count and is drawn. Only `null` is absent: a band headed with a
   * zero says something that a missing figure does not.
   */
  count?: number | string | null;
  tone?: LabelTone;
}
