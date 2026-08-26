import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import Turn from "./Turn.svelte";

describe("Turn", () => {
  it("marks whose turn it is in the DOM, since the labels are gone", () => {
    const { container } = render(Turn, { props: { role: "you", time: "14:18" } });
    expect(container.querySelector(".tc-turn")).toHaveAttribute("data-role", "you");
  });

  it("shows the shell caret only on your own turns", () => {
    const you = render(Turn, { props: { role: "you", time: "14:18" } });
    expect(you.container.querySelector(".tc-turn__caret")).toBeInTheDocument();
    you.unmount();

    const agent = render(Turn, { props: { role: "agent", time: "14:19" } });
    expect(agent.container.querySelector(".tc-turn__caret")).toBeNull();
  });

  it("hides the caret from assistive tech — it is decoration, not a word", () => {
    const { container } = render(Turn, { props: { role: "you", time: "14:18" } });
    expect(container.querySelector(".tc-turn__caret")).toHaveAttribute("aria-hidden", "true");
  });

  it("emits a machine-readable datetime when given epoch millis", () => {
    const { container } = render(Turn, {
      props: { role: "agent", time: "14:19", at: 1735689600000 },
    });
    expect(container.querySelector("time")).toHaveAttribute(
      "datetime",
      new Date(1735689600000).toISOString(),
    );
  });

  it("omits datetime rather than inventing one", () => {
    const { container } = render(Turn, { props: { role: "agent", time: "14:19" } });
    expect(container.querySelector("time")).not.toHaveAttribute("datetime");
  });

  it("lights the node when the turn is waiting on you", () => {
    const { container } = render(Turn, {
      props: { role: "agent", time: "14:29", marked: true },
    });
    expect(container.querySelector(".tc-turn")?.className).toContain("is-marked");
  });
});
