import { describe, expect, test, vi } from "vitest";
import { get } from "svelte/store";
import { ServerManager } from "./server_manager";
import type { ServerRow } from "$bindings/ServerRow";

function aRow(id: string, label: string): ServerRow {
  return {
    entry: {
      server: { id, label, app_version: "0.1.0", os: "windows", arch: "x86_64" },
      endpoint_id: "555bfc38",
      relay: null,
      direct_addrs: [],
      device: { id: "dv_phone", name: "phone", paired_at: 1 },
      capabilities: [],
      last_seen_at: null,
    },
    link: { kind: "unknown", rtt_ms: null },
    refusal: null,
  } as unknown as ServerRow;
}

describe("ServerManager", () => {
  test("load publishes the remembered rows", async () => {
    const invoke = vi.fn().mockResolvedValue([aRow("sv_atlas", "atlas")]);
    const manager = new ServerManager(invoke);

    await manager.load();

    expect(get(manager.rows)).toHaveLength(1);
    expect(invoke).toHaveBeenCalledWith("list_servers");
  });

  // The sweep replaces what load painted, so a machine that stopped answering
  // loses its live dot rather than keeping a stale one beside a fresh row.
  test("sweep replaces the rows rather than appending", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce([aRow("sv_atlas", "atlas")])
      .mockResolvedValueOnce([aRow("sv_atlas", "atlas")]);
    const manager = new ServerManager(invoke);

    await manager.load();
    await manager.sweep();

    expect(get(manager.rows)).toHaveLength(1);
  });

  test("sweeping is false again once the sweep settles", async () => {
    const invoke = vi.fn().mockResolvedValue([]);
    const manager = new ServerManager(invoke);

    await manager.sweep();

    expect(get(manager.sweeping)).toBe(false);
  });

  // A failed sweep must leave the remembered rows on screen. Blanking the list
  // because one call threw would lose the only useful thing on it, and would
  // read as "you have no servers".
  test("a failed sweep keeps the rows that were already painted", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce([aRow("sv_atlas", "atlas")])
      .mockRejectedValueOnce(new Error("no route"));
    const manager = new ServerManager(invoke);

    await manager.load();
    await manager.sweep();

    expect(get(manager.rows)).toHaveLength(1);
    expect(get(manager.sweeping)).toBe(false);
  });

  test("forget removes the machine and reloads", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce(true)
      .mockResolvedValueOnce([]);
    const manager = new ServerManager(invoke);

    await manager.forget("sv_atlas");

    expect(invoke).toHaveBeenCalledWith("forget_server", { id: "sv_atlas" });
    expect(get(manager.rows)).toHaveLength(0);
  });
});
