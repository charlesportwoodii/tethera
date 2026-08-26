/**
 * section — the word above a group. Aligns to the rail, not the screen edge.
 * field   — the name of a value in a form node.
 */
export type LabelKind = "section" | "field";

export interface LabelProps {
  kind?: LabelKind;
  /** Section words align to the gutter by default; a flush one starts at the edge. */
  flush?: boolean;
}
