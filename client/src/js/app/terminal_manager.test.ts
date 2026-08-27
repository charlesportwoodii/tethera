import { describe, expect, test, vi, beforeEach, afterEach } from "vitest";
import { get } from "svelte/store";
import { TerminalManager } from "./terminal_manager";
import type { Invoke } from "./server_manager";
import type { PaneFrame } from "$bindings/PaneFrame";
import type { TerminalFrame } from "$bindings/TerminalFrame";

/** A snapshot carrying one row of text, at the size given. */
function snapshot(text: string, cols = 20, rows = 4): TerminalFrame {
  return {
    snapshot: {
      cols,
      rows,
      styles: [{ fg: "default", bg: "default", attrs: 0 }],
      rows_data: [{ y: 0, from_x: 0, spans: [{ style: 0, text }] }],
      cursor: null,
      alt_screen: false,
      scrollback_len: 0,
    },
  } as unknown as TerminalFrame;
}

/**
 * A fake Tauri event bus.
 *
 * Handlers are kept per channel so a test can push a frame or an ending the
 * same way the real bridge does, rather than reaching into the manager.
 */
class Bus {
  private handlers = new Map<string, Array<(event: { payload: unknown }) => void>>();

  listen = async (channel: string, handler: (event: { payload: never }) => void) => {
    const held = this.handlers.get(channel) ?? [];
    held.push(handler as (event: { payload: unknown }) => void);
    this.handlers.set(channel, held);

    return () => {
      this.handlers.set(
        channel,
        (this.handlers.get(channel) ?? []).filter((candidate) => candidate !== handler),
      );
    };
  };

  emit(channel: string, payload: unknown): void {
    for (const handler of this.handlers.get(channel) ?? []) {
      handler({ payload });
    }
  }

  frame(pane: string, frame: TerminalFrame): void {
    this.emit(TerminalManager.CHANNEL, { pane, frame } as unknown as PaneFrame);
  }

  ended(pane: string): void {
    this.emit(TerminalManager.ENDED, pane);
  }
}

describe("TerminalManager", () => {
  let bus: Bus;
  // The mock and the contract in one type, so a command called with the wrong
  // shape fails type-checking here rather than at run time on a device.
  let invoke: ReturnType<typeof vi.fn> & Invoke;

  beforeEach(() => {
    vi.useFakeTimers();
    bus = new Bus();
    invoke = vi.fn().mockResolvedValue(undefined) as ReturnType<typeof vi.fn> & Invoke;
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  async function attached(pane = "pn_a"): Promise<TerminalManager> {
    const manager = new TerminalManager(invoke, bus.listen, "sv_1", 20, 4);
    await manager.open(pane);

    return manager;
  }

  test("a frame for the attached pane reaches the grid", async () => {
    const manager = await attached();

    bus.frame("pn_a", snapshot("hello"));

    expect(get(manager.revision)).toBe(1);
    expect(manager.grid.line(0)[0]?.text).toContain("hello");
  });

  test("a frame for another pane is ignored", async () => {
    const manager = await attached();

    bus.frame("pn_b", snapshot("wrong pane"));

    // Nothing applied, so nothing to repaint. A grid that took this would show
    // one pane's output under another pane's name, and output looks like output.
    expect(get(manager.revision)).toBe(0);
  });

  test("an unasked-for end re-attaches rather than sitting on a dead stream", async () => {
    const manager = await attached();
    invoke.mockClear();

    bus.ended("pn_a");
    expect(get(manager.live)).toBe(false);

    await vi.advanceTimersByTimeAsync(TerminalManager.REATTACH_WAIT);

    expect(invoke).toHaveBeenCalledWith("attach_pane", expect.objectContaining({ pane: "pn_a" }));
    expect(get(manager.live)).toBe(true);
  });

  test("an end for a different pane is not ours to act on", async () => {
    const manager = await attached();
    invoke.mockClear();

    bus.ended("pn_b");
    await vi.advanceTimersByTimeAsync(TerminalManager.REATTACH_WAIT);

    expect(invoke).not.toHaveBeenCalled();
    expect(get(manager.live)).toBe(true);
  });

  test("a refused key reaches the error store rather than returning silently", async () => {
    const manager = await attached();
    invoke.mockRejectedValueOnce("nothing is attached to pn_a");

    await manager.key("enter", 0);

    expect(get(manager.error)).toContain("nothing is attached to pn_a");
  });

  test("a refused attach leaves the screen not live, and says why", async () => {
    const manager = new TerminalManager(invoke, bus.listen, "sv_1", 20, 4);
    invoke.mockRejectedValueOnce("that pane is gone");

    await manager.open("pn_a");

    expect(get(manager.live)).toBe(false);
    expect(get(manager.error)).toContain("that pane is gone");
  });

  test("closing stops the re-attach timer", async () => {
    const manager = await attached();
    invoke.mockClear();

    bus.ended("pn_a");
    manager.close();
    await vi.advanceTimersByTimeAsync(TerminalManager.REATTACH_WAIT * 2);

    // `detach_pane` is the only call a close should make. A re-attach after it
    // would open a stream nobody is reading and leave it open.
    const attachCalls = invoke.mock.calls.filter(([name]) => name === "attach_pane");
    expect(attachCalls).toHaveLength(0);
  });

  test("switching view re-attaches the same pane with the other reading", async () => {
    const manager = await attached();
    invoke.mockClear();

    await manager.setView("screen");

    expect(invoke).toHaveBeenCalledWith(
      "attach_pane",
      expect.objectContaining({ pane: "pn_a", view: "screen" }),
    );
  });
});
