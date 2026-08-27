import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import DiffView from "./DiffView.svelte";

const UNIFIED = [
  "--- a/src/lib/deeplink.ts",
  "+++ b/src/lib/deeplink.ts",
  "@@ -36,3 +36,5 @@",
  "   const u = new URL(raw);",
  "-  return u;",
  "+  if (u.host) return route(u.host);",
  "+  return u;",
].join("\n");

describe("DiffView", () => {
  it("stays collapsed until opened", () => {
    const { getByRole, container } = render(DiffView, {
      props: { path: "src/lib/deeplink.ts", unified: UNIFIED },
    });
    expect(getByRole("button")).toHaveAttribute("aria-expanded", "false");
    expect(container.querySelector(".tc-diff__body")).toBeNull();
  });

  it("shows the counts in the header", () => {
    const { getByText } = render(DiffView, {
      props: { path: "x.ts", unified: UNIFIED, added: 3, removed: 1 },
    });
    expect(getByText("+3")).toBeInTheDocument();
    expect(getByText("−1")).toBeInTheDocument();
  });

  it("omits a count the machine did not send rather than showing zero", () => {
    const { queryByText } = render(DiffView, { props: { path: "x.ts", unified: UNIFIED } });
    expect(queryByText("+0")).toBeNull();
  });

  it("classifies additions and removals", () => {
    const { container } = render(DiffView, {
      props: { path: "x.ts", unified: UNIFIED, open: true },
    });
    expect(container.querySelectorAll(".is-add")).toHaveLength(2);
    expect(container.querySelectorAll(".is-del")).toHaveLength(1);
  });

  it("treats the file headers as hunks, not as changes", () => {
    const { container } = render(DiffView, {
      props: { path: "x.ts", unified: UNIFIED, open: true },
    });
    // "+++ b/..." starts with a plus and is not an added line.
    expect(container.querySelectorAll(".is-hunk")).toHaveLength(3);
  });

  it("preserves the columns a diff is laid out in", () => {
    const { container } = render(DiffView, {
      props: { path: "x.ts", unified: UNIFIED, open: true },
    });
    const ctx = [...container.querySelectorAll(".is-ctx")][0];
    expect(ctx?.textContent).toBe("   const u = new URL(raw);");
  });

  it("reports the toggle", async () => {
    const ontoggle = vi.fn();
    const { getByRole } = render(DiffView, {
      props: { path: "x.ts", unified: UNIFIED, ontoggle },
    });
    await userEvent.click(getByRole("button"));
    expect(ontoggle).toHaveBeenCalledOnce();
  });
});
