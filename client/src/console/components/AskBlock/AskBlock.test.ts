import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import AskBlock from "./AskBlock.svelte";

const PROMPT = "Which route should own tethera://pair?";

describe("AskBlock", () => {
  it("accepts the bare strings the wire currently sends", () => {
    const { getByRole } = render(AskBlock, {
      props: { prompt: PROMPT, options: ["Rewrite first", "Real route"] },
    });
    expect(getByRole("button", { name: /Rewrite first/ })).toBeInTheDocument();
  });

  it("accepts labelled options with a detail line", () => {
    const { getByRole } = render(AskBlock, {
      props: {
        prompt: PROMPT,
        options: [{ label: "Rewrite first", detail: "One place to fix." }],
      },
    });
    expect(getByRole("button", { name: /One place to fix/ })).toBeInTheDocument();
  });

  it("answers by index, because that is what selects a numbered menu", async () => {
    const onanswer = vi.fn();
    const { getAllByRole } = render(AskBlock, {
      props: { prompt: PROMPT, options: ["a", "b", "c"], onanswer },
    });
    await userEvent.click(getAllByRole("button")[2]);
    expect(onanswer).toHaveBeenCalledWith(2, null);
  });

  it("returns the fingerprint so the gateway can refuse a stale answer", async () => {
    const onanswer = vi.fn();
    const { getAllByRole } = render(AskBlock, {
      props: { prompt: PROMPT, options: ["a"], fingerprint: "abc123", onanswer },
    });
    await userEvent.click(getAllByRole("button")[0]);
    expect(onanswer).toHaveBeenCalledWith(0, "abc123");
  });

  it("names the group with the question itself", () => {
    const { getByRole } = render(AskBlock, { props: { prompt: PROMPT, options: ["a"] } });
    expect(getByRole("group", { name: PROMPT })).toBeInTheDocument();
  });

  it("says how long it has been waiting when told", () => {
    const { getByText } = render(AskBlock, {
      props: { prompt: PROMPT, options: ["a"], waiting: "3m" },
    });
    expect(getByText("waiting on you · 3m")).toBeInTheDocument();
  });
});
