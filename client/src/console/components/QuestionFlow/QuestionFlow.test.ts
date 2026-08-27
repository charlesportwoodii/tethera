import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import QuestionFlow from "./QuestionFlow.svelte";
import type { Ask } from "$bindings/Ask";
import type { Question } from "$bindings/Question";

const ROUTE: Ask = {
  header: "Route",
  prompt: "Which route should own tethera://pair?",
  options: [
    { label: "Rewrite first", description: "One place to fix." },
    { label: "Real route", description: "Fights the framework." },
  ],
  multi_select: false,
  allows_free_text: true,
};

const PLATFORMS: Ask = {
  header: "Platforms",
  prompt: "Which platforms must it work on?",
  options: [
    { label: "iOS", description: null },
    { label: "Android", description: null },
    { label: "Desktop", description: null },
  ],
  multi_select: true,
  allows_free_text: true,
};

const set = (asks: Ask[]): Question => ({ id: "q1", fingerprint: "fp-set", asks });

const BOTH = set([ROUTE, PLATFORMS]);

describe("QuestionFlow", () => {
  it("starts on the first ask", () => {
    const { getByRole } = render(QuestionFlow, { props: { question: BOTH } });
    expect(getByRole("dialog")).toHaveAttribute("data-step", "0");
    expect(getByRole("dialog", { name: ROUTE.prompt })).toBeInTheDocument();
  });

  it("counts the pips from the asks, not the set", () => {
    const { getByText } = render(QuestionFlow, { props: { question: BOTH } });
    expect(getByText("1 of 2")).toBeInTheDocument();
  });

  it("cannot advance until the ask is answered", async () => {
    const { getByRole } = render(QuestionFlow, { props: { question: BOTH } });
    expect(getByRole("button", { name: "Next question" })).toBeDisabled();
    await userEvent.click(getByRole("radio", { name: /Rewrite first/ }));
    expect(getByRole("button", { name: "Next question" })).toBeEnabled();
  });

  it("replaces the choice on a single-select ask", async () => {
    const { getAllByRole } = render(QuestionFlow, { props: { question: BOTH } });
    const [first, second] = getAllByRole("radio");
    await userEvent.click(first);
    await userEvent.click(second);
    expect(first).not.toBeChecked();
    expect(second).toBeChecked();
  });

  it("accumulates choices on a multi-select ask", async () => {
    const { getAllByRole, getByRole } = render(QuestionFlow, { props: { question: BOTH } });
    await userEvent.click(getAllByRole("radio")[0]);
    await userEvent.click(getByRole("button", { name: "Next question" }));
    const boxes = getAllByRole("checkbox");
    await userEvent.click(boxes[0]);
    await userEvent.click(boxes[1]);
    expect(boxes[0]).toBeChecked();
    expect(boxes[1]).toBeChecked();
  });

  it("hides the free-text row when the ask forbids it", () => {
    const { queryByRole } = render(QuestionFlow, {
      props: { question: set([{ ...ROUTE, allows_free_text: false }]) },
    });
    expect(queryByRole("radio", { name: /Something else/ })).toBeNull();
  });

  it("does not accept an empty free-text answer", async () => {
    const { getByRole } = render(QuestionFlow, { props: { question: BOTH } });
    await userEvent.click(getByRole("radio", { name: /Something else/ }));
    expect(getByRole("button", { name: "Next question" })).toBeDisabled();
  });

  it("picks an option by number, the way the menu on the machine is answered", async () => {
    const { getAllByRole } = render(QuestionFlow, { props: { question: BOTH } });
    await userEvent.keyboard("2");
    expect(getAllByRole("radio")[1]).toBeChecked();
  });

  it("reaches a review step that lists every ask", async () => {
    const { getByRole, getAllByRole, getByText } = render(QuestionFlow, {
      props: { question: BOTH },
    });
    await userEvent.click(getAllByRole("radio")[0]);
    await userEvent.click(getByRole("button", { name: "Next question" }));
    await userEvent.click(getAllByRole("checkbox")[0]);
    await userEvent.click(getByRole("button", { name: "Review" }));
    expect(getByRole("dialog")).toHaveAttribute("data-step", "review");
    expect(getByText("Rewrite first")).toBeInTheDocument();
    expect(getByText("iOS")).toBeInTheDocument();
  });

  it("sends one answer per ask and the set's single fingerprint", async () => {
    const onsubmit = vi.fn();
    const { getByRole, getAllByRole } = render(QuestionFlow, {
      props: { question: BOTH, onsubmit },
    });
    await userEvent.click(getAllByRole("radio")[1]);
    await userEvent.click(getByRole("button", { name: "Next question" }));
    await userEvent.click(getAllByRole("checkbox")[2]);
    await userEvent.click(getByRole("button", { name: "Review" }));
    await userEvent.click(getByRole("button", { name: /Send 2 answers/ }));

    // The fingerprint belongs to the set, once — the set is what gets answered.
    expect(onsubmit).toHaveBeenCalledWith([{ choice: 1 }, { multi: [2] }], "fp-set");
  });

  it("sends free text as a text answer", async () => {
    const onsubmit = vi.fn();
    const { getByRole, getByLabelText } = render(QuestionFlow, {
      props: { question: set([ROUTE]), onsubmit },
    });
    await userEvent.click(getByRole("radio", { name: /Something else/ }));
    await userEvent.type(getByLabelText("Your own answer"), "Neither");
    // Typing gets a send button even on the fast path: nothing can tell when a
    // sentence has finished being typed.
    await userEvent.click(getByRole("button", { name: /Send answer/ }));
    expect(onsubmit).toHaveBeenCalledWith([{ text: "Neither" }], "fp-set");
  });

  it("will not send a part-answered set", async () => {
    const { getByRole, getAllByRole } = render(QuestionFlow, { props: { question: BOTH } });
    await userEvent.click(getAllByRole("radio")[0]);
    await userEvent.click(getByRole("button", { name: "Next question" }));
    // Skip the second ask by editing back and forward through review.
    await userEvent.click(getByRole("button", { name: "back" }));
    await userEvent.click(getByRole("button", { name: "Next question" }));
    // Still on the second ask with no answer, so there is no way to reach review.
    expect(getByRole("button", { name: "Review" })).toBeDisabled();
  });

  it("goes back, and cannot go back from the first ask", async () => {
    const { getByRole, getAllByRole } = render(QuestionFlow, { props: { question: BOTH } });
    expect(getByRole("button", { name: "back" })).toBeDisabled();
    await userEvent.click(getAllByRole("radio")[0]);
    await userEvent.click(getByRole("button", { name: "Next question" }));
    await userEvent.click(getByRole("button", { name: "back" }));
    expect(getByRole("dialog")).toHaveAttribute("data-step", "0");
  });

  it("keeps the answer when a review edit sends you back", async () => {
    const { getByRole, getAllByRole } = render(QuestionFlow, {
      props: { question: set([ROUTE]), autoSubmit: false },
    });
    await userEvent.click(getAllByRole("radio")[0]);
    await userEvent.click(getByRole("button", { name: "Review" }));
    await userEvent.click(getByRole("button", { name: "edit" }));
    expect(getAllByRole("radio")[0]).toBeChecked();
  });

  it("cancels on Escape", async () => {
    const oncancel = vi.fn();
    render(QuestionFlow, { props: { question: BOTH, oncancel } });
    await userEvent.keyboard("{Escape}");
    expect(oncancel).toHaveBeenCalledOnce();
  });

  it("falls back to a numbered label when an ask has no header", async () => {
    const { getByRole, getAllByRole, getByText } = render(QuestionFlow, {
      props: { question: set([{ ...ROUTE, header: null }]), autoSubmit: false },
    });
    await userEvent.click(getAllByRole("radio")[0]);
    await userEvent.click(getByRole("button", { name: "Review" }));
    expect(getByText("Question 1")).toBeInTheDocument();
  });
  it("sends a lone single-select the moment an option is pressed", async () => {
    const onsubmit = vi.fn();
    const { getAllByRole } = render(QuestionFlow, {
      props: { question: set([ROUTE]), onsubmit },
    });
    await userEvent.click(getAllByRole("radio")[1]);
    // What the harness's own picker does. A permission prompt is the most
    // frequent question by a wide margin, and a review step for a single choice
    // is friction exactly where it is felt most.
    expect(onsubmit).toHaveBeenCalledWith([{ choice: 1 }], "fp-set");
  });

  it("offers no button of its own on the fast path", () => {
    const { queryByRole } = render(QuestionFlow, { props: { question: set([ROUTE]) } });
    // Pressing an option is the send, so a second control would be a second way
    // to send the same answer.
    expect(queryByRole("button", { name: "Review" })).toBeNull();
    expect(queryByRole("button", { name: /Send/ })).toBeNull();
  });

  it("reviews a multi-select even when it is the only ask", async () => {
    const onsubmit = vi.fn();
    const { getAllByRole, getByRole } = render(QuestionFlow, {
      props: { question: set([PLATFORMS]), onsubmit },
    });
    await userEvent.click(getAllByRole("checkbox")[0]);
    // A multi-select is not finished at the first tap; sending there would take
    // the answer away before it was given.
    expect(onsubmit).not.toHaveBeenCalled();
    await userEvent.click(getByRole("button", { name: "Review" }));
    await userEvent.click(getByRole("button", { name: /Send 1 answer/ }));
    expect(onsubmit).toHaveBeenCalledWith([{ multi: [0] }], "fp-set");
  });

  it("reviews a set of two even when each ask is a single-select", async () => {
    const onsubmit = vi.fn();
    const { getAllByRole, getByRole } = render(QuestionFlow, {
      props: { question: set([ROUTE, { ...ROUTE, multi_select: false }]), onsubmit },
    });
    await userEvent.click(getAllByRole("radio")[0]);
    expect(onsubmit).not.toHaveBeenCalled();
    expect(getByRole("button", { name: "Next question" })).toBeInTheDocument();
  });

  it("reviews when the caller turns the fast path off", async () => {
    const onsubmit = vi.fn();
    const { getAllByRole, getByRole } = render(QuestionFlow, {
      props: { question: set([ROUTE]), autoSubmit: false, onsubmit },
    });
    await userEvent.click(getAllByRole("radio")[0]);
    expect(onsubmit).not.toHaveBeenCalled();
    expect(getByRole("button", { name: "Review" })).toBeInTheDocument();
  });
});
