import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import ConnDot from "./ConnDot.svelte";

describe("ConnDot", () => {
  it("says direct and the round trip when the path is direct", () => {
    const { getByText } = render(ConnDot, { props: { link: "direct", rttMs: 38 } });
    expect(getByText("direct · 38 ms")).toBeInTheDocument();
  });

  it("says via relay, because the dot only reports reachability", () => {
    const { getByText } = render(ConnDot, { props: { link: "relayed", rttMs: 112 } });
    expect(getByText("via relay · 112 ms")).toBeInTheDocument();
  });

  it("reads as connected while the path is still unclassified", () => {
    const { getByText, container } = render(ConnDot, { props: { link: "unknown" } });
    // Reachable but unclassified is not offline, and the dot must not go hollow.
    expect(getByText("connected")).toBeInTheDocument();
    expect(container.querySelector(".tc-conn")?.className).not.toContain("is-offline");
  });

  it("shows when it was last seen instead of a round trip when offline", () => {
    const { getByText } = render(ConnDot, {
      props: { link: "offline", rttMs: 38, lastSeen: "2d" },
    });
    // The stale round trip must not survive: it would read as a live measurement.
    expect(getByText("no route · 2d")).toBeInTheDocument();
  });

  it("omits the figure entirely when a path has not settled", () => {
    const { getByText } = render(ConnDot, { props: { link: "direct" } });
    expect(getByText("direct")).toBeInTheDocument();
  });

  it("appends a note after the figure", () => {
    const { getByText } = render(ConnDot, {
      props: { link: "direct", rttMs: 38, note: "native" },
    });
    expect(getByText("direct · 38 ms · native")).toBeInTheDocument();
  });

  it("marks the offline case in the DOM, not only in the text", () => {
    const { container } = render(ConnDot, { props: { link: "offline" } });
    expect(container.querySelector(".tc-conn")).toHaveAttribute("data-link", "offline");
  });
});
