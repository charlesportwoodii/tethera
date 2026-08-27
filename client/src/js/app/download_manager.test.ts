import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { get } from "svelte/store";
import { DownloadManager } from "./download_manager";
import type { DownloadProgress } from "$bindings/DownloadProgress";
import type { DownloadState } from "$bindings/DownloadState";

/** A machine that reports whatever the test tells it to, when it is told to. */
function aWire() {
  let emit: ((progress: DownloadProgress) => void) | null = null;
  const stop = vi.fn();

  const listen = vi.fn(
    async (_channel: string, handler: (event: { payload: DownloadProgress }) => void) => {
      emit = (progress) => handler({ payload: progress });

      return stop;
    },
  );

  return {
    listen,
    stop,
    say(progress: Partial<DownloadProgress> & { id: string; state: DownloadState }) {
      emit?.({
        asset: "as_apk",
        name: "tethera.apk",
        received: 0,
        total: 0,
        saved_to: null,
        failure: null,
        ...progress,
      } as DownloadProgress);
    },
  };
}

describe("DownloadManager", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // The defect that produced a truncated APK somebody tried to install: nothing
  // on screen distinguished a file still arriving from one that had arrived.
  // The row has to exist before the machine has answered anything, because the
  // machine hashes the whole asset before it answers at all.
  test("a row appears as soon as the download is asked for", async () => {
    const wire = aWire();
    const invoke = vi.fn().mockResolvedValue("dl_0");
    const manager = new DownloadManager(invoke, wire.listen);

    await manager.attach();
    await manager.start("sv_atlas", "as_apk", "tethera.apk");

    const [row] = get(manager.rows);

    expect(row.id).toBe("dl_0");
    expect(row.state).toBe("opening");
    expect(DownloadManager.fraction(row)).toBeNull();
  });

  test("a dismissed save dialog leaves no row behind", async () => {
    const wire = aWire();
    const invoke = vi.fn().mockResolvedValue(null);
    const manager = new DownloadManager(invoke, wire.listen);

    await manager.attach();
    const id = await manager.start("sv_atlas", "as_apk", "tethera.apk");

    expect(id).toBeNull();
    expect(get(manager.rows)).toHaveLength(0);
  });

  test("the bar follows the bytes once the machine says how many there are", async () => {
    const wire = aWire();
    const manager = new DownloadManager(vi.fn().mockResolvedValue("dl_0"), wire.listen);

    await manager.attach();
    await manager.start("sv_atlas", "as_apk", "tethera.apk");

    wire.say({ id: "dl_0", state: "running", received: 100, total: 400 });

    const [row] = get(manager.rows);

    expect(row.received).toBe(100);
    expect(DownloadManager.fraction(row)).toBeCloseTo(0.25);
  });

  // Every event that is not about bytes carries a total of zero, because the
  // only thing that knows the total is the machine's head. Taking that zero
  // literally empties the bar the moment a transfer pauses - which is the exact
  // moment a person is looking at it to decide whether their file survived.
  test("an event that does not know the total leaves the last one alone", async () => {
    const wire = aWire();
    const manager = new DownloadManager(vi.fn().mockResolvedValue("dl_0"), wire.listen);

    await manager.attach();
    await manager.start("sv_atlas", "as_apk", "tethera.apk");

    wire.say({ id: "dl_0", state: "running", received: 300, total: 400 });
    wire.say({
      id: "dl_0",
      state: "paused",
      received: 300,
      total: 0,
      failure: "connection lost: timed out",
    });

    const [row] = get(manager.rows);

    expect(row.state).toBe("paused");
    expect(row.total).toBe(400);
    expect(DownloadManager.fraction(row)).toBeCloseTo(0.75);
  });

  test("a paused download is still working, and a failed one is not", async () => {
    const wire = aWire();
    const manager = new DownloadManager(vi.fn().mockResolvedValue("dl_0"), wire.listen);

    await manager.attach();
    await manager.start("sv_atlas", "as_apk", "tethera.apk");

    wire.say({ id: "dl_0", state: "paused", received: 300 });

    expect(get(manager.working)).toHaveLength(1);

    wire.say({ id: "dl_0", state: "failed", received: 300, failure: "gave up" });

    expect(get(manager.working)).toHaveLength(0);
    expect(get(manager.rows)[0].failure).toBe("gave up");
  });

  test("cancelling tells the machine which download to stop", async () => {
    const wire = aWire();
    const invoke = vi.fn().mockResolvedValue("dl_0");
    const manager = new DownloadManager(invoke, wire.listen);

    await manager.attach();
    await manager.start("sv_atlas", "as_apk", "tethera.apk");
    await manager.cancel("dl_0");

    expect(invoke).toHaveBeenCalledWith("cancel_download", { id: "dl_0" });
  });

  // The row is what says the file is safe to open. Clearing it the instant the
  // last byte lands takes that away at the one moment it is worth reading.
  test("a finished download says so, then leaves on its own", async () => {
    const wire = aWire();
    const manager = new DownloadManager(vi.fn().mockResolvedValue("dl_0"), wire.listen);

    await manager.attach();
    await manager.start("sv_atlas", "as_apk", "tethera.apk");

    wire.say({
      id: "dl_0",
      state: "done",
      received: 400,
      total: 400,
      saved_to: "/storage/Download/tethera.apk",
    });

    expect(get(manager.rows)[0].savedTo).toBe("/storage/Download/tethera.apk");

    vi.advanceTimersByTime(DownloadManager.LINGER);

    expect(get(manager.rows)).toHaveLength(0);
  });

  // A failure is the one row nobody should have to catch in passing.
  test("a failed download stays until it is dismissed", async () => {
    const wire = aWire();
    const manager = new DownloadManager(vi.fn().mockResolvedValue("dl_0"), wire.listen);

    await manager.attach();
    await manager.start("sv_atlas", "as_apk", "tethera.apk");

    wire.say({ id: "dl_0", state: "failed", received: 12, failure: "no route" });
    vi.advanceTimersByTime(DownloadManager.LINGER * 4);

    expect(get(manager.rows)).toHaveLength(1);

    manager.dismiss("dl_0");

    expect(get(manager.rows)).toHaveLength(0);
  });

  // A transfer outlives the screen that started it, so a screen that arrives
  // late has to be able to see one it never asked for.
  test("a download this screen never started still shows", async () => {
    const wire = aWire();
    const manager = new DownloadManager(vi.fn(), wire.listen);

    await manager.attach();

    wire.say({ id: "dl_7", state: "running", received: 5, total: 10, name: "notes.md" });

    expect(get(manager.rows)).toHaveLength(1);
    expect(get(manager.rows)[0].name).toBe("notes.md");
  });

  test("cleanup stops listening", async () => {
    const wire = aWire();
    const manager = new DownloadManager(vi.fn(), wire.listen);

    await manager.attach();
    manager.cleanup();

    expect(wire.stop).toHaveBeenCalled();
  });

  test("a download that could not be asked for is reported rather than swallowed", async () => {
    const wire = aWire();
    const invoke = vi.fn().mockRejectedValue(new Error("that machine is not paired"));
    const manager = new DownloadManager(invoke, wire.listen);

    await manager.attach();
    const id = await manager.start("sv_atlas", "as_apk", "tethera.apk");

    expect(id).toBeNull();
    expect(get(manager.error)).toContain("not paired");
  });
});
