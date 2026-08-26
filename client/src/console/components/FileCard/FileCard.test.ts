import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import FileCard from "./FileCard.svelte";

describe("FileCard", () => {
  it("formats bytes in binary units", () => {
    const { getByText } = render(FileCard, {
      props: { name: "pairing-routes.md", size: 8396 },
    });
    expect(getByText(/8\.2 KB/)).toBeInTheDocument();
  });

  it("takes the bigint the Rust side actually sends", () => {
    const { getByText } = render(FileCard, {
      props: { name: "core.dump", size: 5368709120n },
    });
    expect(getByText(/5\.0 GB/)).toBeInTheDocument();
  });

  it("says so rather than printing NaN for a size it cannot read", () => {
    const { getByText } = render(FileCard, {
      props: { name: "x.bin", size: Number.NaN },
    });
    expect(getByText(/unknown size/)).toBeInTheDocument();
  });

  it("falls back to FILE when there is no extension", () => {
    const { container } = render(FileCard, { props: { name: "Makefile", size: 10 } });
    expect(container.querySelector(".tc-file__ext")?.textContent?.trim()).toBe("FILE");
  });

  it("hands the file over when pressed", async () => {
    const ondownload = vi.fn();
    const { getByRole } = render(FileCard, {
      props: { name: "a.md", size: 1, ondownload },
    });
    await userEvent.click(getByRole("button"));
    expect(ondownload).toHaveBeenCalledOnce();
  });
});
