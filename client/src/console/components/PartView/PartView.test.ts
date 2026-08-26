import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import PartView from "./PartView.svelte";
import type { Part } from "$bindings/Part";

describe("PartView", () => {
  it("renders a text part as prose", () => {
    const part: Part = { text: { text: "The document is explicit." } };
    const { getByText } = render(PartView, { props: { part } });
    expect(getByText("The document is explicit.")).toBeInTheDocument();
  });

  it("folds a tool use", () => {
    const part: Part = { tool_use: { name: "Bash", input: "ls", fallback_text: "ls" } };
    const { getByRole } = render(PartView, { props: { part } });
    expect(getByRole("button", { name: /Bash/ })).toBeInTheDocument();
  });

  it("turns a question into an answerable block", async () => {
    const onanswer = vi.fn();
    const part: Part = {
      question: { prompt: "Which route?", options: ["a", "b"], fallback_text: "" },
    };
    const { getAllByRole } = render(PartView, { props: { part, onanswer } });
    await userEvent.click(getAllByRole("button")[1]);
    expect(onanswer).toHaveBeenCalledWith(1, null);
  });

  it("offers a file the agent handed over", () => {
    const part: Part = {
      file: { name: "pairing-routes.md", size: 8396n, fallback_text: "" },
    };
    const { getByText } = render(PartView, { props: { part } });
    expect(getByText("pairing-routes.md")).toBeInTheDocument();
  });

  it("renders an unknown part as the rows the gateway sent", () => {
    const part: Part = {
      unknown: { kind: "diff", fallback_text: "--- a/x\n+++ b/x" },
    };
    const { container } = render(PartView, { props: { part } });
    const pre = container.querySelector(".tc-part__fallback");
    // A newer gateway must never produce a blank turn on an older client.
    expect(pre).toHaveAttribute("data-kind", "diff");
    expect(pre?.textContent).toContain("+++ b/x");
  });
});
