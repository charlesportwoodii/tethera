import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import AttachChip from "./AttachChip.svelte";

describe("AttachChip", () => {
  it("shows progress while the file is still going up", () => {
    const { getByRole } = render(AttachChip, {
      props: { name: "screenshot-2.png", progress: 0.64 },
    });
    expect(getByRole("progressbar")).toHaveAttribute("aria-valuenow", "64");
  });

  it("offers removal only once the upload has landed", () => {
    const busy = render(AttachChip, {
      props: { name: "a.png", progress: 0.5, onremove: () => {} },
    });
    // Removing a file mid-upload is a different operation — cancelling — and the
    // caller has not been given a way to say what that means.
    expect(busy.queryByRole("button")).toBeNull();
    busy.unmount();

    const done = render(AttachChip, { props: { name: "a.png", onremove: () => {} } });
    expect(done.getByRole("button", { name: "Remove a.png" })).toBeInTheDocument();
  });

  it("removes by name so a screen reader knows which chip went", async () => {
    const onremove = vi.fn();
    const { getByRole } = render(AttachChip, {
      props: { name: "nat-punch.log", onremove },
    });
    await userEvent.click(getByRole("button", { name: "Remove nat-punch.log" }));
    expect(onremove).toHaveBeenCalledOnce();
  });

  it("treats progress zero as uploading, not as finished", () => {
    const { getByRole } = render(AttachChip, { props: { name: "a.png", progress: 0 } });
    expect(getByRole("progressbar")).toHaveAttribute("aria-valuenow", "0");
  });
});
