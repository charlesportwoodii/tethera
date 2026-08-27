import { describe, expect, test, vi } from "vitest";
import { get } from "svelte/store";
import { MachineManager } from "./machine_manager";
import type { Conversation } from "$bindings/Conversation";
import type { MachineTree } from "$bindings/MachineTree";
import type { Pane } from "$bindings/Pane";
import type { Tab } from "$bindings/Tab";
import type { TabLayout } from "$bindings/TabLayout";

function aTab(id: string, title = "shell"): Tab {
  return {
    id: id as unknown as Tab["id"],
    workspace_id: "ws_1" as unknown as Tab["workspace_id"],
    index: 1,
    title,
    conversation: null,
    foreground_command: null,
  };
}

function aPane(id: string): Pane {
  return {
    id: id as unknown as Pane["id"],
    tab_id: "tb_1" as unknown as Pane["tab_id"],
    workspace_id: "ws_1" as unknown as Pane["workspace_id"],
    label: id,
    title: null,
    cwd: null,
    size: { cols: 80, rows: 24 },
    focused: false,
    foreground_command: null,
    conversation: null,
    agent: null,
  };
}

function aLayout(tab: string, panes: string[]): TabLayout {
  return {
    tab: tab as unknown as TabLayout["tab"],
    slots: panes.map((pane, at) => ({
      pane: pane as unknown as TabLayout["slots"][number]["pane"],
      rect: { x: at * 40, y: 0, width: 40, height: 20 },
    })),
    zoomed: null,
  };
}

function aTree(over: Partial<MachineTree> = {}): MachineTree {
  return {
    workspaces: [],
    tabs: [],
    panes: [],
    conversations: [] as Conversation[],
    layouts: [],
    ...over,
  };
}

/**
 * Drives the manager by calling the handler it registered.
 *
 * That is the seam: `listen` hands the manager a function, and the harness
 * keeps a reference to it. Faking a transport instead would test the transport.
 */
function harness(tree: MachineTree) {
  const handlers = new Map<string, (event: { payload: unknown }) => void>();

  const invoke = vi.fn(async (command: string) => {
    if (command === "watch_machine") {
      return tree;
    }

    return undefined;
  });

  const listen = vi.fn(
    async (channel: string, handler: (event: { payload: unknown }) => void) => {
      handlers.set(channel, handler);

      return () => handlers.delete(channel);
    },
  );

  const manager = new MachineManager(invoke as never, listen as never, "sv_1");

  return {
    manager,
    invoke,
    emit: (channel: string, payload: unknown) => handlers.get(channel)?.({ payload }),
  };
}

describe("MachineManager", () => {
  test("the opening snapshot fills every rank at once", async () => {
    const { manager } = harness(
      aTree({ tabs: [aTab("tb_1")], panes: [aPane("pn_a")], layouts: [aLayout("tb_1", ["pn_a"])] }),
    );

    await manager.open();

    expect(get(manager.tabs)).toHaveLength(1);
    expect(get(manager.panes)).toHaveLength(1);
    expect(manager.layoutOf("tb_1")?.slots).toHaveLength(1);
  });

  // The event set has no "added". A change naming an id the client has not seen
  // is an insert, and treating it as a miss is how a tab created at the desk
  // failed to appear in the strip.
  test("a change naming an unknown tab inserts it", async () => {
    const { manager, emit } = harness(aTree({ tabs: [aTab("tb_1")] }));
    await manager.open();

    emit(MachineManager.CHANNEL, {
      server: "sv_1",
      event: { tab_changed: aTab("tb_2") },
    });

    expect(get(manager.tabs)).toHaveLength(2);
  });

  test("a change naming a known tab replaces it in place", async () => {
    const { manager, emit } = harness(aTree({ tabs: [aTab("tb_1", "old")] }));
    await manager.open();

    emit(MachineManager.CHANNEL, {
      server: "sv_1",
      event: { tab_changed: aTab("tb_1", "new") },
    });

    const tabs = get(manager.tabs);

    expect(tabs).toHaveLength(1);
    expect(tabs[0].title).toBe("new");
  });

  test("a removed pane leaves the tree", async () => {
    const { manager, emit } = harness(aTree({ panes: [aPane("pn_a"), aPane("pn_b")] }));
    await manager.open();

    emit(MachineManager.CHANNEL, { server: "sv_1", event: { pane_removed: "pn_a" } });

    expect(get(manager.panes).map((pane) => pane.id)).toEqual(["pn_b"]);
  });

  // One webview holds several machines. An event applied to the wrong tree looks
  // exactly like the right tree changing.
  test("an event from another machine is ignored", async () => {
    const { manager, emit } = harness(aTree({ tabs: [aTab("tb_1")] }));
    await manager.open();

    emit(MachineManager.CHANNEL, {
      server: "sv_other",
      event: { tab_changed: aTab("tb_9") },
    });

    expect(get(manager.tabs)).toHaveLength(1);
  });

  // A layout is carried whole, so applying one replaces that tab's own geometry
  // and leaves every other tab's alone.
  test("a layout change replaces only that tab's geometry", async () => {
    const { manager, emit } = harness(
      aTree({ layouts: [aLayout("tb_1", ["pn_a"]), aLayout("tb_2", ["pn_b"])] }),
    );
    await manager.open();

    emit(MachineManager.CHANNEL, {
      server: "sv_1",
      event: { layout_changed: aLayout("tb_1", ["pn_a", "pn_c"]) },
    });

    expect(manager.layoutOf("tb_1")?.slots).toHaveLength(2);
    expect(manager.layoutOf("tb_2")?.slots).toHaveLength(1);
  });

  // A watch that ends silently is indistinguishable from a machine where nothing
  // is happening, which is the failure this whole manager exists to end.
  test("an unasked-for end stops claiming to be live", async () => {
    const { manager, emit } = harness(aTree({}));
    await manager.open();

    expect(get(manager.live)).toBe(true);

    emit(MachineManager.ENDED, "sv_1");

    expect(get(manager.live)).toBe(false);
  });

  // Panes and tabs are asked for by their parent, because every screen here is
  // showing one tab at a time rather than the whole machine.
  test("panes and layouts are addressable by tab", async () => {
    const other = aPane("pn_b");
    other.tab_id = "tb_2" as unknown as Pane["tab_id"];

    const { manager } = harness(aTree({ panes: [aPane("pn_a"), other] }));
    await manager.open();

    expect(manager.panesOf("tb_1").map((pane) => pane.id)).toEqual(["pn_a"]);
    expect(manager.panesOf("tb_2").map((pane) => pane.id)).toEqual(["pn_b"]);
  });
});
