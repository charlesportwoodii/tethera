import { parseMarkdown, plainText } from "$console";
import type { AnswerRecord } from "$bindings/AnswerRecord";
import type { Part } from "$bindings/Part";
import type { QuestionOption } from "$bindings/QuestionOption";
import type { Role } from "$bindings/Role";
import type { Turn } from "$bindings/Turn";

/** What a tool fold's one-line detail is coloured as. */
export type Tone = "muted" | "ok" | "attn";

/** An option as the Console's AskBlock takes it. */
export interface AskChoice {
  label: string;
  detail: string | null;
}

/**
 * Turning wire parts into the strings a screen draws.
 *
 * Formatting lives here rather than in the markup so each decision is one
 * testable function, and so the same tool fold reads identically wherever it
 * appears.
 */
export class Parts {

  /**
   * The clock time in the gutter.
   *
   * `Timestamp` is epoch milliseconds. Reading it as seconds puts every turn in
   * 1970, which renders as a plausible-looking time and a wrong date.
   */
  static clock(at: number): string {
    return new Date(at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }

  static day(at: number): string {
    return new Date(at).toLocaleDateString([], {
      weekday: "short",
      month: "short",
      day: "numeric",
    });
  }

  /** Whether two turns fall on different days, which is where a date header goes. */
  static newDay(previous: Turn | undefined, turn: Turn): boolean {
    if (!previous) {
      return true;
    }

    return (
      new Date(Number(previous.at)).toDateString() !== new Date(Number(turn.at)).toDateString()
    );
  }

  /** How long a question has been waiting, as the design writes it. */
  static waited(since: number, now: number): string {
    const seconds = Math.max(0, Math.round((now - since) / 1000));

    if (seconds < 60) {
      return `${seconds}s`;
    }

    const minutes = Math.round(seconds / 60);

    if (minutes < 60) {
      return `${minutes}m`;
    }

    const hours = Math.round(minutes / 60);

    return hours < 24 ? `${hours}h` : `${Math.round(hours / 24)}d`;
  }

  static choices(options: QuestionOption[]): AskChoice[] {
    return options.map((option) => ({ label: option.label, detail: option.description }));
  }

  /**
   * The one-line result beside a tool's name.
   *
   * A running tool says so. A failed one says so too, because a fold that looks
   * identical whether it worked or not makes somebody open every one of them.
   */
  static toolDetail(part: Extract<Part, { tool_use: unknown }>["tool_use"]): string | null {
    if (part.status === "running") {
      return "running";
    }

    if (part.status === "failed") {
      return "failed";
    }

    const result = part.result?.trim();

    if (!result) {
      return null;
    }

    const first = result.split("\n")[0];

    return first.length > 48 ? `${first.slice(0, 47)}…` : first;
  }


  /** `+3 −1`, or nothing when the machine did not count. */
  static diffDetail(added: number | null, removed: number | null): string | null {
    if (added === null && removed === null) {
      return null;
    }

    return `+${added ?? 0} −${removed ?? 0}`;
  }

  /**
   * A size in bytes as a person reads it.
   *
   * Absent rather than zero when the machine did not measure: "0 B" claims a
   * measurement that was never taken.
   */
  static size(bytes: number | bigint | null): string | null {
    if (bytes === null) {
      return null;
    }

    const value = Number(bytes);
    const units = ["B", "KB", "MB", "GB"];
    let held = value;
    let unit = 0;

    while (held >= 1024 && unit < units.length - 1) {
      held /= 1024;
      unit += 1;
    }

    return `${held < 10 && unit > 0 ? held.toFixed(1) : Math.round(held)} ${units[unit]}`;
  }

  /**
   * The last question in a turn's parts, unanswered.
   *
   * The transcript and the `Blocked` watch event carry the same question, so a
   * screen that drew both would show it twice. The transcript copy is the one
   * with a place on the timeline.
   */
  /**
   * Markdown flattened to the words it renders as.
   *
   * A conversation row shows one line, so it cannot render markdown - a heading
   * or a list has nowhere to go on a single line. Showing the source is the
   * other wrong answer: a preview reading `**304 tests passing**` is the one
   * place a person sees the syntax rather than the text.
   *
   * The Console's own parser and flattener do the reading. It already decides
   * what a marker means, and a second answer to that question here would drift
   * from the one the transcript renders with.
   */
  static plain(text: string | null): string {
    if (!text) {
      return "";
    }

    return plainText(parseMarkdown(text)).replace(/\s+/g, " ").trim();
  }

  static pendingQuestion(turn: Turn): boolean {
    return turn.parts.some((part) => "question" in part && !Parts.wholeAnswered(part.question));
  }

  /**
   * Whether one ask inside a set has been answered.
   *
   * A historical set can genuinely be part answered, and the record carries a
   * hole where an answer is missing. Reading the array by index rather than
   * counting it is what keeps a hole from shifting every later answer onto the
   * wrong question.
   */
  static answeredAt(record: AnswerRecord | null, index: number): boolean {
    return (record?.answers[index] ?? null) !== null;
  }

  /** Whether every ask in a set has an answer, which is when it stops blocking. */
  static wholeAnswered(part: Extract<Part, { question: unknown }>["question"]): boolean {
    const record = part.answered;

    if (record === null) {
      return false;
    }

    return part.question.asks.every((_, index) => Parts.answeredAt(record, index));
  }
}
