import { describe, expect, test, vi } from "vitest";
import { render } from "@testing-library/svelte";
import DownloadTray from "./DownloadTray.svelte";
import type { Download } from "$managers/download_manager";

function aRow(over: Partial<Download> = {}): Download {
  return {
    id: "dl_0",
    asset: "as_apk",
    name: "tethera.apk",
    received: 0,
    total: 0,
    state: "opening",
    savedTo: null,
    failure: null,
    ...over,
  };
}

describe("DownloadTray", () => {
  test("draws nothing when nothing is being carried", () => {
    const { container } = render(DownloadTray, { props: { rows: [] } });

    expect(container.querySelector(".tray")).toBeNull();
  });

  // The whole point of the row: a file still arriving has to look different
  // from one that has arrived. A truncated APK opens perfectly well right up
  // until it will not install.
  test("a download that has not been sized yet claims no position", () => {
    const { container } = render(DownloadTray, { props: { rows: [aRow()] } });

    const bar = container.querySelector('[role="progressbar"]');

    expect(bar).not.toBeNull();
    expect(bar?.getAttribute("aria-valuenow")).toBeNull();
    expect(bar?.className).toContain("waiting");
  });

  test("a running download reports how far along it is", () => {
    const { container, getByText } = render(DownloadTray, {
      props: { rows: [aRow({ state: "running", received: 100, total: 400 })] },
    });

    expect(container.querySelector('[role="progressbar"]')?.getAttribute("aria-valuenow")).toBe(
      "0.25",
    );
    expect(getByText(/100 B of 400 B/)).toBeInTheDocument();
  });

  // Reported as coming back rather than as broken. Somebody told "failed"
  // starts the transfer over, and starting over is what discards the bytes
  // already on this phone.
  test("an interrupted download says it is asking again, not that it failed", () => {
    const { getByText } = render(DownloadTray, {
      props: {
        rows: [
          aRow({
            state: "paused",
            received: 300,
            total: 400,
            failure: "connection lost: timed out",
          }),
        ],
      },
    });

    expect(getByText(/asking again/)).toBeInTheDocument();
  });

  test("a finished download says where it went", () => {
    const { getByText } = render(DownloadTray, {
      props: {
        rows: [aRow({ state: "done", received: 400, total: 400, savedTo: "/Download/a.apk" })],
      },
    });

    expect(getByText(/saved to \/Download\/a\.apk/)).toBeInTheDocument();
  });

  test("a download in flight can be stopped", async () => {
    const oncancel = vi.fn();
    const { getByLabelText } = render(DownloadTray, {
      props: { rows: [aRow({ state: "running", received: 1, total: 4 })], oncancel },
    });

    (getByLabelText("Stop downloading tethera.apk") as HTMLButtonElement).click();

    expect(oncancel).toHaveBeenCalledWith("dl_0");
  });

  // A settled row is a report, not a control. Offering "stop" on one asks a
  // person to cancel something that already happened.
  test("a settled download offers dismissal rather than a stop", () => {
    const { getByLabelText, queryByLabelText } = render(DownloadTray, {
      props: { rows: [aRow({ state: "done", received: 4, total: 4 })] },
    });

    expect(getByLabelText("Dismiss tethera.apk")).toBeInTheDocument();
    expect(queryByLabelText("Stop downloading tethera.apk")).toBeNull();
  });
});
