import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import FileViewer from "./FileViewer.svelte";
import type { FileMeta } from "$console/types/files";

const FILE: FileMeta = {
  name: "pairing-routes.md",
  size: 8396,
  mime: "markdown",
  at: "14:31",
};

describe("FileViewer", () => {
  it("is a modal dialog named after the file", () => {
    const { getByRole } = render(FileViewer, { props: { file: FILE } });
    const dialog = getByRole("dialog", { name: "pairing-routes.md" });
    expect(dialog).toHaveAttribute("aria-modal", "true");
  });

  it("states size, kind and time so the header is worth its space", () => {
    const { getByText } = render(FileViewer, { props: { file: FILE } });
    expect(getByText("8.2 KB · markdown · 14:31")).toBeInTheDocument();
  });

  it("anchors as a sheet by default and centres when asked", () => {
    const sheet = render(FileViewer, { props: { file: FILE } });
    expect(sheet.getByRole("dialog")).toHaveAttribute("data-anchor", "sheet");
    sheet.unmount();

    const modal = render(FileViewer, { props: { file: FILE, anchor: "modal" } });
    expect(modal.getByRole("dialog")).toHaveAttribute("data-anchor", "modal");
  });

  it("closes on the button", async () => {
    const onclose = vi.fn();
    const { getByRole } = render(FileViewer, { props: { file: FILE, onclose } });
    await userEvent.click(getByRole("button", { name: "Close" }));
    expect(onclose).toHaveBeenCalledOnce();
  });

  it("closes on Escape", async () => {
    const onclose = vi.fn();
    render(FileViewer, { props: { file: FILE, onclose } });
    await userEvent.keyboard("{Escape}");
    expect(onclose).toHaveBeenCalledOnce();
  });

  it("marks one tab selected and reports the choice", async () => {
    const onselecttab = vi.fn();
    const { getAllByRole } = render(FileViewer, {
      props: { file: FILE, tabs: ["Rendered", "Source"], activeTab: "Rendered", onselecttab },
    });
    const tabs = getAllByRole("tab");
    expect(tabs[0]).toHaveAttribute("aria-selected", "true");
    await userEvent.click(tabs[1]);
    expect(onselecttab).toHaveBeenCalledWith("Source");
  });

  it("omits the tab strip for a single-view file", () => {
    const { queryByRole } = render(FileViewer, { props: { file: FILE } });
    expect(queryByRole("tablist")).toBeNull();
  });

  it("says why there is no preview instead of showing an empty pane", () => {
    const { getByText } = render(FileViewer, {
      props: {
        file: { name: "core.dump", size: 5368709120n },
        noPreviewReason: "No preview for a binary this size. It stays on atlas.",
      },
    });
    expect(getByText(/No preview for a binary this size/)).toBeInTheDocument();
  });
});
