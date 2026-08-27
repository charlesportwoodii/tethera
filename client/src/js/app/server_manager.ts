import { writable, type Readable, type Writable } from "svelte/store";
import type { ServerRow } from "$bindings/ServerRow";

export type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

/**
 * The machines this device knows, and how they answered.
 *
 * `load` paints what is remembered, with every link Unknown because nothing has
 * been measured yet. `sweep` dials them all and replaces the list. A sweep that
 * fails leaves the painted rows alone: blanking the list would lose the one
 * useful fact on it.
 */
export class ServerManager {
  /**
   * How often a screen that is following re-dials.
   *
   * A sweep is one round trip per machine, and what it carries back is a
   * snapshot: which sessions are attached and what each one is doing right now.
   * Swept once at mount, that snapshot is a photograph — a session that starts
   * work after the screen opened never changes its mark, and one that finishes
   * keeps a working mark until somebody navigates away and back.
   *
   * Five seconds is short enough that the state on screen matches the machine
   * within a glance, and long enough that a pocketed phone is not dialling
   * constantly. Overlapping sweeps are skipped rather than queued.
   */
  static readonly FOLLOW_MS = 5000;

  private readonly rowStore: Writable<ServerRow[]>;
  private readonly sweepingStore: Writable<boolean>;

  private inFlight = false;

  public readonly rows: Readable<ServerRow[]>;
  public readonly sweeping: Readable<boolean>;

  constructor(private readonly invoke: Invoke) {
    this.rowStore = writable([]);
    this.sweepingStore = writable(false);
    this.rows = { subscribe: this.rowStore.subscribe };
    this.sweeping = { subscribe: this.sweepingStore.subscribe };
  }

  async load(): Promise<void> {
    const rows = (await this.invoke("list_servers")) as ServerRow[];
    this.rowStore.set(rows);
  }

  async sweep(): Promise<void> {
    if (this.inFlight) {
      return;
    }

    this.inFlight = true;
    this.sweepingStore.set(true);

    try {
      const rows = (await this.invoke("sweep_servers")) as ServerRow[];
      this.rowStore.set(rows);
    } catch (error) {
      console.error("sweep failed", error);
    } finally {
      this.inFlight = false;
      this.sweepingStore.set(false);
    }
  }

  /**
   * Keeps sweeping until the returned function is called.
   *
   * Paused while the screen is hidden and swept once on return, so a phone put
   * down stops dialling and a phone picked up shows the machine as it is now
   * rather than as it was when it went into a pocket.
   */
  follow(alongside?: () => void): () => void {
    const tick = () => {
      if (typeof document !== "undefined" && document.visibilityState === "hidden") {
        return;
      }

      void this.sweep();
      alongside?.();
    };

    const timer = setInterval(tick, ServerManager.FOLLOW_MS);

    if (typeof document === "undefined") {
      return () => clearInterval(timer);
    }

    const onVisible = () => {
      if (document.visibilityState === "visible") {
        void this.sweep();
        alongside?.();
      }
    };

    document.addEventListener("visibilitychange", onVisible);

    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }

  async forget(id: string): Promise<void> {
    await this.invoke("forget_server", { id });
    await this.load();
  }
}
