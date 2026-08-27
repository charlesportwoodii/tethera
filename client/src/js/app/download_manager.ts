import { derived, writable, type Readable, type Writable } from "svelte/store";
import type { DownloadProgress } from "$bindings/DownloadProgress";
import type { DownloadState } from "$bindings/DownloadState";
import type { Invoke } from "./server_manager";

/** Subscribes to the download channel and returns the function that stops it. */
export type ListenDownloads = (
  channel: string,
  handler: (event: { payload: DownloadProgress }) => void,
) => Promise<() => void>;

/** One download as a screen draws it. */
export interface Download {
  id: string;
  asset: string;
  name: string;
  /** Bytes on this phone, counting whatever a previous attempt left. */
  received: number;
  /** The whole file. Zero while it is still unknown. */
  total: number;
  state: DownloadState;
  savedTo: string | null;
  failure: string | null;
}

/**
 * Whether nothing further will happen to a download in this state.
 *
 * Written as a full map rather than a list of the terminal ones so that a state
 * added to the Rust enum fails to compile here instead of quietly counting as
 * still running.
 */
const SETTLED: Record<DownloadState, boolean> = {
  opening: false,
  running: false,
  // Interrupted, and being asked for again. Reporting this as finished is what
  // would teach somebody to start the transfer over, which is the one action
  // that throws away the bytes already on disk.
  paused: false,
  done: true,
  failed: true,
  cancelled: true,
};

/**
 * Every download this app is carrying, as rows.
 *
 * The transfer does not live here and does not live on a screen: it runs in a
 * task on the Rust side and reports on one channel. This is what makes a
 * download survive the screen that started it, and it is why rows arrive for
 * downloads this object never asked for.
 */
export class DownloadManager {
  private readonly rowStore: Writable<Download[]>;
  private readonly errorStore: Writable<string | null>;

  public readonly rows: Readable<Download[]>;
  /** The ones still moving, for a screen that only draws live transfers. */
  public readonly working: Readable<Download[]>;
  public readonly error: Readable<string | null>;

  /** The channel the Rust side reports every download on. */
  public static readonly CHANNEL = "download-progress";

  /**
   * How long a finished row stays on screen.
   *
   * The row is what says the file is whole and safe to open, so clearing it the
   * instant the last byte lands takes that away at the one moment it is worth
   * reading. Failures do not linger - they stay until dismissed.
   */
  public static readonly LINGER = 6000;

  private unlisten: (() => void) | null = null;
  private readonly clearing = new Map<string, ReturnType<typeof setTimeout>>();

  constructor(
    private readonly invoke: Invoke,
    private readonly listen: ListenDownloads,
  ) {
    this.rowStore = writable([]);
    this.errorStore = writable(null);

    this.rows = { subscribe: this.rowStore.subscribe };
    this.error = { subscribe: this.errorStore.subscribe };
    this.working = derived(this.rowStore, ($rows) => $rows.filter((row) => !SETTLED[row.state]));
  }

  /** Starts listening. Safe to call twice; the second replaces the first. */
  async attach(): Promise<void> {
    this.cleanup();

    this.unlisten = await this.listen(DownloadManager.CHANNEL, (event) => {
      this.absorb(event.payload);
    });
  }

  /**
   * Asks for a file and answers the id of the download.
   *
   * `null` means the save dialog was dismissed, or the machine could not be
   * asked. Neither waits for the bytes: the row this puts on screen is how the
   * transfer is followed from here.
   */
  async start(server: string, asset: string, name: string): Promise<string | null> {
    this.errorStore.set(null);

    try {
      const id = (await this.invoke("download_asset", { server, asset, name })) as string | null;

      if (!id) {
        return null;
      }

      // Drawn here rather than waited for. The Rust side emits its own opening
      // row, but that row crosses a channel while this one is already a return
      // value in hand - and the gap between them is a person looking at a
      // screen that has not acknowledged their tap.
      this.absorb({
        id,
        asset,
        name,
        received: 0,
        total: 0,
        state: "opening",
        saved_to: null,
        failure: null,
      });

      return id;
    } catch (error) {
      this.errorStore.set(String(error));

      return null;
    }
  }

  /**
   * Stops a download, keeping what has already arrived.
   *
   * Asking for the same file again resumes from there, so this is a pause a
   * person can act on rather than a decision to lose the transfer.
   */
  async cancel(id: string): Promise<void> {
    try {
      await this.invoke("cancel_download", { id });
    } catch (error) {
      this.errorStore.set(String(error));
    }
  }

  /** Takes a row off the screen. Does not touch the transfer. */
  dismiss(id: string): void {
    const clearing = this.clearing.get(id);

    if (clearing) {
      clearTimeout(clearing);
      this.clearing.delete(id);
    }

    this.rowStore.update((rows) => rows.filter((row) => row.id !== id));
  }

  cleanup(): void {
    for (const clearing of this.clearing.values()) {
      clearTimeout(clearing);
    }

    this.clearing.clear();

    if (this.unlisten) {
      this.unlisten();
      this.unlisten = null;
    }
  }

  /**
   * How far along, or `null` when that is not yet knowable.
   *
   * `null` is not zero. A machine hashes the whole asset before it says how big
   * it is, and on a large file that is most of a second - a bar sitting at zero
   * for that long says "nothing is happening", which is the opposite of true.
   * A screen draws an indeterminate row instead.
   */
  static fraction(row: Download): number | null {
    if (row.total <= 0) {
      return null;
    }

    return Math.min(1, row.received / row.total);
  }

  static settled(state: DownloadState): boolean {
    return SETTLED[state];
  }

  /**
   * Folds one report into the rows.
   *
   * Upserts by id rather than requiring the row to exist. A transfer outlives
   * the screen that started it, so a screen opened halfway through has to be
   * able to show one it never asked for.
   */
  private absorb(progress: DownloadProgress): void {
    const asset = String(progress.asset);

    this.rowStore.update((rows) => {
      const held = rows.find((row) => row.id === progress.id);

      // A total of zero means "not known here", never "the file is empty".
      // Only the machine's head carries the size, so every report that is not
      // about bytes - opening, paused, cancelled - carries a zero, and taking
      // it literally empties the bar at the moment somebody is reading it to
      // find out whether their download survived.
      const total = progress.total > 0 ? Number(progress.total) : (held?.total ?? 0);

      const row: Download = {
        id: progress.id,
        asset,
        name: progress.name,
        received: Number(progress.received),
        total,
        state: progress.state,
        savedTo: progress.saved_to ?? null,
        failure: progress.failure ?? null,
      };

      return held ? rows.map((existing) => (existing.id === row.id ? row : existing)) : [...rows, row];
    });

    this.age(progress.id, progress.state);
  }

  /** Schedules a finished row to clear itself, and only a finished one. */
  private age(id: string, state: DownloadState): void {
    const clearing = this.clearing.get(id);

    if (clearing) {
      clearTimeout(clearing);
      this.clearing.delete(id);
    }

    // A failure is the one row nobody should have to catch in passing: it is
    // the only report that asks a person to do something about it.
    if (state !== "done" && state !== "cancelled") {
      return;
    }

    this.clearing.set(
      id,
      setTimeout(() => {
        this.clearing.delete(id);
        this.rowStore.update((rows) => rows.filter((row) => row.id !== id));
      }, DownloadManager.LINGER),
    );
  }
}
