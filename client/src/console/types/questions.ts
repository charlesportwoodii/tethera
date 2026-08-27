import type { Answer } from "$bindings/Answer";
import type { Ask } from "$bindings/Ask";
import type { Question } from "$bindings/Question";
import type { QuestionOption } from "$bindings/QuestionOption";

/**
 * Re-exported so a component imports one name rather than reaching into the
 * generated tree. The shapes are the wire's.
 *
 * A `Question` is a **set**: it owns the id and the fingerprint, and holds one or
 * more `Ask`s. Answering is atomic — the harness stays blocked until it has every
 * answer, and its picker is one piece of screen state — so answers are collected
 * and delivered together, one entry per ask in the set's own order.
 */
export type { Answer, Ask, Question, QuestionOption };

/** What the flow holds while one ask is part-answered. */
export interface Draft {
  /** Indices into that ask's own options. */
  selected: number[];
  /** Free text, when the ask allows it. Null when not chosen. */
  text: string | null;
}

export const EMPTY_DRAFT: Draft = { selected: [], text: null };

/**
 * Turn a draft into the answer the server accepts, or null when the ask has not
 * been answered yet.
 *
 * Free text wins on a single-select ask, because choosing "Other" is what
 * cleared the option in the first place.
 */
export function toAnswer(draft: Draft, multiSelect: boolean): Answer | null {
  const text = draft.text !== null && draft.text.trim() !== "" ? draft.text.trim() : null;
  if (multiSelect) {
    if (draft.selected.length > 0) return { multi: draft.selected };
    return text === null ? null : { text };
  }
  if (text !== null) return { text };
  if (draft.selected.length > 0) return { choice: draft.selected[0] };
  return null;
}

/**
 * One entry per ask, in the set's order, with a hole where an ask has no answer.
 *
 * The holes are deliberate: a shorter array would shift every later answer onto
 * the wrong question, which is the same reason the wire types it that way.
 */
export function toAnswers(asks: Ask[], drafts: Record<number, Draft>): Array<Answer | null> {
  return asks.map((ask, i) => toAnswer(drafts[i] ?? EMPTY_DRAFT, ask.multi_select));
}

/** Whether every ask in the set has an answer. The agent stays blocked until so. */
export function isComplete(asks: Ask[], drafts: Record<number, Draft>): boolean {
  return toAnswers(asks, drafts).every((a) => a !== null);
}
