import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import ThinkingRow from "./ThinkingRow.svelte";
import type { AgentStats } from "$console/types/stats";

const STATS: AgentStats = {
  elapsedSeconds: 41,
  tokensIn: 18412,
  tokensOut: 2100,
  tools: 12,
  contextUsed: 62000,
  contextWindow: 200000,
  model: "opus 4.6",
  costUsd: 0.41,
};

describe("ThinkingRow", () => {
  it("shows the figures that prove work is happening", () => {
    const { getByText } = render(ThinkingRow, { props: { stats: STATS } });
    expect(getByText("41s")).toBeInTheDocument();
    expect(getByText("18.4k")).toBeInTheDocument();
    expect(getByText("2.1k")).toBeInTheDocument();
    expect(getByText("12")).toBeInTheDocument();
  });

  it("holds back model and cost until there is room for them", () => {
    const { queryByText } = render(ThinkingRow, { props: { stats: STATS } });
    expect(queryByText("opus 4.6")).toBeNull();
    expect(queryByText("$0.41")).toBeNull();
  });

  it("shows them when dense", () => {
    const { getByText } = render(ThinkingRow, { props: { stats: STATS, dense: true } });
    expect(getByText("opus 4.6")).toBeInTheDocument();
    expect(getByText("$0.41")).toBeInTheDocument();
  });

  it("shows the context bar when the window is known", () => {
    const { getByRole } = render(ThinkingRow, { props: { stats: STATS } });
    expect(getByRole("progressbar")).toHaveAttribute("aria-valuenow", "31");
  });

  it("omits the context bar rather than inventing a window", () => {
    const { queryByRole } = render(ThinkingRow, {
      props: { stats: { ...STATS, contextUsed: null, contextWindow: null } },
    });
    expect(queryByRole("progressbar")).toBeNull();
  });

  it("shows the in-flight activity", () => {
    const { getByText } = render(ThinkingRow, {
      props: { stats: STATS, activity: "Reading src/lib/deeplink.ts" },
    });
    expect(getByText("Reading src/lib/deeplink.ts")).toBeInTheDocument();
  });

  it("offers a stop only when the caller can honour it", async () => {
    const quiet = render(ThinkingRow, { props: { stats: STATS } });
    expect(quiet.queryByRole("button")).toBeNull();
    quiet.unmount();

    const onstop = vi.fn();
    const { getByRole } = render(ThinkingRow, { props: { stats: STATS, onstop } });
    await userEvent.click(getByRole("button"));
    expect(onstop).toHaveBeenCalledOnce();
  });

  it("keeps the spinner out of the accessibility tree — the verb says it", () => {
    const { container } = render(ThinkingRow, { props: { stats: STATS } });
    expect(container.querySelector(".tc-braille")).toHaveAttribute("aria-hidden", "true");
  });
});
