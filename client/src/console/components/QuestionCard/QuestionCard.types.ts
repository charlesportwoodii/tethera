import type { Question } from "$bindings/Question";

/**
 * An announcement, not a control.
 *
 * Everything that answers a question lives in `QuestionFlow`. This says what is
 * being asked and offers one way in. It replaced a card that answered in place,
 * because every question bug of that evening lived in the inline path — a
 * free-text row that typed its own number into itself, a free-text answer with
 * nowhere to type it, and a fingerprint read off a live text buffer — and the
 * card was a fast path for exactly one shape while carrying the whole failure
 * surface for it.
 */
export interface QuestionCardProps {
  question: Question;
  /** How long it has been waiting — already formatted. */
  waiting?: string | null;
  /**
   * False once the set is answered or resolved by any other route.
   *
   * A question that is history must not offer a way in: the flow would open on a
   * set the machine has already moved past. Whether a set is still live is not
   * in the record for one resolved outside `answer`, so the caller derives it.
   */
  live?: boolean;
  onopen?: (() => void) | null;
}
