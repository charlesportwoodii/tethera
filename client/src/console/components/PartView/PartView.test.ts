import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import PartView from "./PartView.svelte";
import type { Part } from "$bindings/Part";
import type { Question } from "$bindings/Question";

const QUESTION: Question = {
  id: "q1",
  fingerprint: "fp-set",
  asks: [
    {
      header: "Route",
      prompt: "Which route should own tethera://pair?",
      options: [
        { label: "Rewrite first", description: null },
        { label: "Real route", description: null },
      ],
      multi_select: false,
      allows_free_text: false,
    },
  ],
};

describe("PartView", () => {
  it("renders a text part as prose", () => {
    const part: Part = { text: { text: "The document is explicit." } };
    const { getByText } = render(PartView, { props: { part } });
    expect(getByText("The document is explicit.")).toBeInTheDocument();
  });

  it("folds a tool call and carries its status through", () => {
    const part: Part = {
      tool_use: {
        name: "Bash",
        input: "cargo test",
        result: "1 failed",
        status: "failed",
        fallback_text: "",
      },
    };
    const { getByRole } = render(PartView, { props: { part } });
    expect(getByRole("button")).toHaveAttribute("data-status", "failed");
  });

  it("reveals the tool body when expanded, so the fold has something to show", () => {
    const part: Part = {
      tool_use: {
        name: "Bash",
        input: "cargo test -p tethera-common",
        result: "11 passed; 1 failed",
        status: "failed",
        fallback_text: "",
      },
    };
    const closed = render(PartView, { props: { part } });
    expect(closed.container.querySelector(".tc-part__body")).toBeNull();
    closed.unmount();

    const open = render(PartView, { props: { part, expanded: true } });
    // Without a body the chevron moves and visibly nothing happens.
    expect(open.container.querySelector(".tc-part__body")?.textContent).toBe(
      "11 passed; 1 failed",
    );
  });

  it("falls back to the input for a call that has not returned yet", () => {
    const part: Part = {
      tool_use: {
        name: "Bash",
        input: "cargo test -p tethera-common",
        result: null,
        status: "running",
        fallback_text: "",
      },
    };
    const { container } = render(PartView, { props: { part, expanded: true } });
    expect(container.querySelector(".tc-part__body")?.textContent).toBe(
      "cargo test -p tethera-common",
    );
  });

  it("renders a diff with its counts", () => {
    const part: Part = {
      diff: {
        path: "src/lib/deeplink.ts",
        unified: "@@ -1 +1 @@\n-a\n+b",
        added: 1,
        removed: 1,
        fallback_text: "",
      },
    };
    const { getByText } = render(PartView, { props: { part } });
    expect(getByText("src/lib/deeplink.ts")).toBeInTheDocument();
    expect(getByText("+1")).toBeInTheDocument();
  });

  it("renders a plan", () => {
    const part: Part = {
      todo: {
        items: [
          { text: "Read the contract", status: "done" },
          { text: "Decide", status: "in_progress" },
        ],
        fallback_text: "",
      },
    };
    const { getByText } = render(PartView, { props: { part } });
    expect(getByText("1 of 2")).toBeInTheDocument();
  });

  it("renders a table as a table", () => {
    const part: Part = {
      table: {
        columns: ["test", "result"],
        rows: [["host_not_path", "FAILED"]],
        fallback_text: "",
      },
    };
    const { getAllByRole } = render(PartView, { props: { part } });
    expect(getAllByRole("columnheader")).toHaveLength(2);
  });

  it("renders a status line", () => {
    const part: Part = {
      status: { label: "Compacted", detail: "62k reclaimed", fallback_text: "" },
    };
    const { getByText } = render(PartView, { props: { part } });
    expect(getByText("62k reclaimed")).toBeInTheDocument();
  });

  it("offers a file the agent handed over, by asset id", async () => {
    const onopenfile = vi.fn();
    const part: Part = {
      file: {
        asset: "as_9f21c",
        name: "pairing-routes.md",
        mime: "text/markdown",
        size: 8396,
        fallback_text: "",
      },
    };
    const { getByRole } = render(PartView, { props: { part, onopenfile } });
    await userEvent.click(getByRole("button"));
    expect(onopenfile).toHaveBeenCalledWith("as_9f21c", "pairing-routes.md");
  });

  it("draws an image inline once its bytes are supplied", () => {
    const part: Part = {
      file: { asset: "as_1", name: "shot.png", mime: "image/png", size: 4096, fallback_text: "" },
    };
    const { container } = render(PartView, { props: { part, imageUrl: "data:image/png;base64,AA" } });
    const img = container.querySelector("img");
    expect(img).toHaveAttribute("alt", "shot.png");
    expect(img).toHaveAttribute("loading", "lazy");
  });

  it("keeps the thumbnail tappable, opening the same viewer a card opens", async () => {
    const onopenfile = vi.fn();
    const part: Part = {
      file: { asset: "as_1", name: "shot.png", mime: "image/png", size: 4096, fallback_text: "" },
    };
    const { getByRole } = render(PartView, {
      props: { part, imageUrl: "data:image/png;base64,AA", onopenfile },
    });
    await userEvent.click(getByRole("button"));
    // Being able to see it is not a reason to lose the full view, the size or Save.
    expect(onopenfile).toHaveBeenCalledWith("as_1", "shot.png");
  });

  it("falls back to the card while the bytes are still arriving", () => {
    const part: Part = {
      file: { asset: "as_1", name: "shot.png", mime: "image/png", size: 4096, fallback_text: "" },
    };
    const { container, getByText } = render(PartView, { props: { part } });
    // An image has to arrive whole to decode, so there is no partial picture to show.
    expect(container.querySelector("img")).toBeNull();
    expect(getByText("shot.png")).toBeInTheDocument();
  });

  it("never draws an SVG as a thumbnail, whatever URL it is handed", () => {
    const part: Part = {
      file: {
        asset: "as_1",
        name: "logo.svg",
        mime: "image/svg+xml",
        size: 900,
        fallback_text: "",
      },
    };
    const { container } = render(PartView, { props: { part, imageUrl: "data:image/svg+xml,AA" } });
    // An SVG is a document that can carry script, and csp: null is still open.
    expect(container.querySelector("img")).toBeNull();
  });

  it("draws a non-image as a card even with a URL supplied", () => {
    const part: Part = {
      file: { asset: "as_1", name: "notes.md", mime: "text/markdown", size: 10, fallback_text: "" },
    };
    const { container } = render(PartView, { props: { part, imageUrl: "data:text/plain,AA" } });
    expect(container.querySelector("img")).toBeNull();
  });

  it("says unknown size rather than nothing when the machine did not measure it", () => {
    const part: Part = {
      file: { asset: "as_1", name: "core.dump", mime: null, size: null, fallback_text: "" },
    };
    const { getByText } = render(PartView, { props: { part } });
    expect(getByText(/unknown size/)).toBeInTheDocument();
  });

  it("announces an unanswered question without offering a way to answer it", async () => {
    const onexpandquestion = vi.fn();
    const part: Part = { question: { question: QUESTION, answered: null, fallback_text: "" } };
    const { getByRole, getByText } = render(PartView, {
      props: { part, onexpandquestion },
    });
    expect(getByText(QUESTION.asks[0].prompt)).toBeInTheDocument();
    // The only control is the way into the flow. Answering from the transcript
    // is what produced every question bug worth having: two code paths composing
    // the same reply, and only one of them getting the fingerprint right.
    await userEvent.click(getByRole("button", { name: "Answer" }));
    expect(onexpandquestion).toHaveBeenCalledOnce();
  });

  it("leaves the announcement inert when the caller cannot answer", () => {
    const part: Part = { question: { question: QUESTION, answered: null, fallback_text: "" } };
    const { queryByRole } = render(PartView, { props: { part } });
    expect(queryByRole("button")).toBeNull();
  });

  it("collapses a question that has already been answered", () => {
    const part: Part = {
      question: {
        question: QUESTION,
        answered: { answers: [{ choice: 0 }], at: 1735689600000 },
        fallback_text: "",
      },
    };
    const { getByText, queryByRole } = render(PartView, { props: { part } });
    expect(getByText("answered")).toBeInTheDocument();
    expect(queryByRole("group")).toBeNull();
  });

  it("renders an unknown part as the rows the server sent", () => {
    const part: Part = { unknown: { kind: "chart", fallback_text: "--- a/x\n+++ b/x" } };
    const { container } = render(PartView, { props: { part } });
    const pre = container.querySelector(".tc-part__fallback");
    // A newer server must never produce a blank turn on an older client.
    expect(pre).toHaveAttribute("data-kind", "chart");
    expect(pre?.textContent).toContain("+++ b/x");
  });
});
