import { describe, expect, it } from "vitest";
import { isComplete, toAnswer, toAnswers, type Draft } from "./questions";
import type { Ask } from "$bindings/Ask";

const draft = (over: Partial<Draft> = {}): Draft => ({ selected: [], text: null, ...over });

const ask = (over: Partial<Ask> = {}): Ask => ({
  header: null,
  prompt: "?",
  options: [{ label: "a", description: null }, { label: "b", description: null }],
  multi_select: false,
  allows_free_text: true,
  ...over,
});

describe("toAnswer", () => {
  it("sends a single choice as choice", () => {
    expect(toAnswer(draft({ selected: [1] }), false)).toEqual({ choice: 1 });
  });

  it("sends several as multi", () => {
    expect(toAnswer(draft({ selected: [0, 2] }), true)).toEqual({ multi: [0, 2] });
  });

  it("sends free text as text", () => {
    expect(toAnswer(draft({ text: "Windows only" }), false)).toEqual({ text: "Windows only" });
  });

  it("trims free text", () => {
    expect(toAnswer(draft({ text: "  a  " }), false)).toEqual({ text: "a" });
  });

  it("treats blank free text as no answer", () => {
    expect(toAnswer(draft({ text: "   " }), false)).toBeNull();
  });

  it("is null when nothing was chosen", () => {
    expect(toAnswer(draft(), false)).toBeNull();
    expect(toAnswer(draft(), true)).toBeNull();
  });

  it("prefers free text on a single-select ask", () => {
    // Choosing "Other" is what cleared the option, so text is the live answer.
    expect(toAnswer(draft({ selected: [0], text: "mine" }), false)).toEqual({ text: "mine" });
  });

  it("prefers the options on a multi-select ask", () => {
    // The wire has no shape for options-plus-text, and the options are the part
    // the agent can act on mechanically.
    expect(toAnswer(draft({ selected: [1], text: "mine" }), true)).toEqual({ multi: [1] });
  });
});

describe("toAnswers", () => {
  it("returns one entry per ask, in the set's order", () => {
    const asks = [ask(), ask({ multi_select: true })];
    const answers = toAnswers(asks, { 0: draft({ selected: [1] }), 1: draft({ selected: [0, 1] }) });
    expect(answers).toEqual([{ choice: 1 }, { multi: [0, 1] }]);
  });

  it("leaves a hole rather than shortening the array", () => {
    const asks = [ask(), ask(), ask()];
    const answers = toAnswers(asks, { 0: draft({ selected: [0] }), 2: draft({ selected: [1] }) });
    // A shorter array would shift the third answer onto the second question.
    expect(answers).toEqual([{ choice: 0 }, null, { choice: 1 }]);
  });

  it("is all holes for a set nobody has touched", () => {
    expect(toAnswers([ask(), ask()], {})).toEqual([null, null]);
  });
});

describe("isComplete", () => {
  it("is false while any ask is unanswered", () => {
    expect(isComplete([ask(), ask()], { 0: draft({ selected: [0] }) })).toBe(false);
  });

  it("is true once every ask has an answer", () => {
    const drafts = { 0: draft({ selected: [0] }), 1: draft({ text: "x" }) };
    expect(isComplete([ask(), ask()], drafts)).toBe(true);
  });

  it("is true for an empty set, which has nothing outstanding", () => {
    expect(isComplete([], {})).toBe(true);
  });
});
