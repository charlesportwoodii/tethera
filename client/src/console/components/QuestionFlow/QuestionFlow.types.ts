import type { Answer } from "$bindings/Answer";
import type { Question } from "$bindings/Question";

export interface QuestionFlowProps {
  /**
   * The whole set. Its `asks` are the steps, and its single fingerprint is what
   * goes back with the answers — the set is what gets answered, so there is one
   * fingerprint rather than one per ask.
   */
  question: Question;
  anchor?: "sheet" | "modal";
  /** How long the agent has been blocked — already formatted. */
  waiting?: string | null;
  /**
   * A lone single-select ask sends the moment an option is pressed, with no
   * review step.
   *
   * This is what the harness's own picker does, and it is what keeps a permission
   * prompt to one tap. Anything with more than one ask, a multi-select, or a
   * free-text answer in progress still ends on the review screen, because there
   * is then more than one decision to check before the set goes back.
   *
   * Set false to make every set review first.
   */
  autoSubmit?: boolean;
  /**
   * One entry per ask, in the set's order. Only fires when every ask has an
   * answer, so no entry is null at this point — the nullable element type is the
   * wire's, kept so the value passes straight through.
   */
  onsubmit?: (answers: Array<Answer | null>, fingerprint: string) => void;
  oncancel?: () => void;
}
