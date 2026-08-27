import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import QuestionCard from "./QuestionCard.svelte";
import type { Ask } from "$bindings/Ask";
import type { Question } from "$bindings/Question";

const ask = (over: Partial<Ask> = {}): Ask => ({
  header: "Route",
  prompt: "Which route should own tethera://pair?",
  options: [
    { label: "Rewrite first", description: "One place to fix." },
    { label: "Real route", description: null },
  ],
  multi_select: false,
  allows_free_text: true,
  ...over,
});

const set = (asks: Ask[]): Question => ({ id: "q1", fingerprint: "fp-set", asks });

describe("QuestionCard", () => {
  it("shows the prompt of a lone ask", () => {
    const { getByText } = render(QuestionCard, { props: { question: set([ask()]) } });
    expect(getByText(ask().prompt)).toBeInTheDocument();
  });

  it("lists the prompts of a set", () => {
    const { getByText } = render(QuestionCard, {
      props: { question: set([ask(), ask({ prompt: "Which platforms?" })]) },
    });
    expect(getByText("2 questions before it goes further.")).toBeInTheDocument();
    expect(getByText("Which platforms?")).toBeInTheDocument();
  });

  it("offers no way to answer in place", () => {
    const { queryByRole } = render(QuestionCard, {
      props: { question: set([ask()]), onopen: () => {} },
    });
    // Every answering bug lived in the inline path. There are no option rows here
    // at all, so there is nothing to answer with except the flow.
    expect(queryByRole("button", { name: /Rewrite first/ })).toBeNull();
    expect(queryByRole("radio")).toBeNull();
  });

  it("opens the flow when asked", async () => {
    const onopen = vi.fn();
    const { getByRole } = render(QuestionCard, { props: { question: set([ask()]), onopen } });
    await userEvent.click(getByRole("button", { name: "Answer" }));
    expect(onopen).toHaveBeenCalledOnce();
  });

  it("counts the questions in the action for a set", () => {
    const { getByRole } = render(QuestionCard, {
      props: { question: set([ask(), ask()]), onopen: () => {} },
    });
    expect(getByRole("button", { name: "Answer 2 questions" })).toBeInTheDocument();
  });

  it("says how long it has been waiting", () => {
    const { getByText } = render(QuestionCard, {
      props: { question: set([ask()]), waiting: "3m" },
    });
    expect(getByText("waiting on you · 3m")).toBeInTheDocument();
  });

  it("is inert once the set is no longer live", () => {
    const { queryByRole, getByText, container } = render(QuestionCard, {
      props: { question: set([ask()]), live: false, onopen: () => {} },
    });
    // Opening the flow on a set the machine has moved past would answer nothing.
    expect(queryByRole("button")).toBeNull();
    expect(getByText("no longer waiting")).toBeInTheDocument();
    expect(container.querySelector(".tc-qcard")).toHaveAttribute("data-live", "false");
  });

  it("drops the pink ring once it is history", () => {
    const { container } = render(QuestionCard, {
      props: { question: set([ask()]), live: false },
    });
    // A pink ring is how the system says something is owed.
    expect(container.querySelector(".tc-qcard")?.className).toContain("is-history");
  });
});
