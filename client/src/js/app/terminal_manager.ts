import { writable, type Readable, type Writable } from "svelte/store";
import { TerminalGrid } from "$console/lib/terminal";
import type { Key } from "$bindings/Key";
import type { Mods } from "$bindings/Mods";
import type { PaneFrame } from "$bindings/PaneFrame";
import type { TerminalControls } from "$bindings/TerminalControls";
import type { Invoke } from "./server_manager";

/** Subscribes to an event channel and returns the function that stops it. */
export type Listen = <T>(
  channel: string,
  handler: (event: { payload: T }) => void,
) => Promise<() => void>;

/**
 * What a machine that will not describe itself is assumed to allow.
 *
 * Nothing. A screen that guessed generously would draw a key bar over a pane it
 * cannot type into, and the first keystroke would be the thing that told
 * somebody.
 */
const NOTHING: TerminalControls = {
  attach: false,
  input: false,
  scrollback: false,
  open: false,
  split: false,
  close: false,
  layout: false,
  focus_tab: false,
};

/**
 * One pane's screen, and the keys going back to it.
 *
 * The grid is owned here rather than by the component, because damage frames
 * only make sense against what came before and a component recreated on a prop
 * change cannot hold that. `revision` is what tells Svelte to repaint: the grid
 * mutates in place, so there is nothing for it to compare.
 */
export class TerminalManager {
  /** Where frames arrive. Matches `PaneAttachments::CHANNEL`. */
  static readonly CHANNEL = "terminal";

  /** Where an unasked-for end arrives. Matches `PaneAttachments::ENDED`. */
  static readonly ENDED = "terminal_attach_ended";

  /**
   * How long to wait before opening another stream.
   *
   * Not immediately: whatever closed the path is usually a second or so from
   * being reachable again, and an instant retry spends the one attempt that
   * would have worked.
   */
  static readonly REATTACH_WAIT = 1200;

  public readonly grid = new TerminalGrid();

  private readonly revisionStore: Writable<number>;
  private readonly liveStore: Writable<boolean>;
  private readonly errorStore: Writable<string | null>;
  private readonly controlStore: Writable<TerminalControls>;

  public readonly revision: Readable<number>;
  /** Whether a stream is open. False means nothing typed will arrive. */
  public readonly live: Readable<boolean>;
  public readonly error: Readable<string | null>;
  public readonly controls: Readable<TerminalControls>;

  private pane: string | null = null;
  private unlisten: (() => void) | null = null;
  private unlistenEnded: (() => void) | null = null;
  private retry: ReturnType<typeof setTimeout> | null = null;
  private closed = false;

  constructor(
    private readonly invoke: Invoke,
    private readonly listen: Listen,
    private readonly server: string,
    private readonly cols: number,
    private readonly rows: number,
  ) {
    this.revisionStore = writable(0);
    this.liveStore = writable(false);
    this.errorStore = writable(null);
    this.controlStore = writable(NOTHING);

    this.revision = { subscribe: this.revisionStore.subscribe };
    this.live = { subscribe: this.liveStore.subscribe };
    this.error = { subscribe: this.errorStore.subscribe };
    this.controls = { subscribe: this.controlStore.subscribe };
  }

  /** What this machine will let a terminal screen do. */
  async loadControls(): Promise<void> {
    try {
      this.controlStore.set(
        (await this.invoke("terminal_controls", { server: this.server })) as TerminalControls,
      );
    } catch (error) {
      // Left at nothing rather than guessed. The screen then draws no controls
      // and says the machine did not answer, which is true.
      this.controlStore.set(NOTHING);
      this.refuse(String(error));
    }
  }

  /**
   * Attaches to a pane, detaching from whichever one was open.
   *
   * The grid is reset rather than kept: the first frame of any attach is a
   * snapshot, so keeping the previous pane's screen would show it under the new
   * pane's name for exactly as long as the round trip takes.
   */
  async open(pane: string): Promise<void> {
    if (this.pane !== null && this.pane !== pane) {
      await this.detach();
    }

    this.pane = pane;
    this.closed = false;
    this.grid.reset(0, 0);
    this.revisionStore.set(0);

    await this.subscribe();
    await this.attach();
  }

  async key(key: Key, mods: Mods): Promise<void> {
    await this.deliver("pane_key", { key, mods });
  }

  async text(text: string): Promise<void> {
    await this.deliver("pane_text", { text });
  }

  /**
   * Stops reading. The pane keeps running on the machine.
   *
   * Sets `closed` before anything else, so a pending re-attach that fires
   * between here and the timer being cleared finds a manager that has gone.
   */
  close(): void {
    this.closed = true;

    if (this.retry) {
      clearTimeout(this.retry);
      this.retry = null;
    }

    this.unlisten?.();
    this.unlistenEnded?.();
    this.unlisten = null;
    this.unlistenEnded = null;

    void this.detach();
  }

  private async subscribe(): Promise<void> {
    this.unlisten ??= await this.listen<PaneFrame>(TerminalManager.CHANNEL, (event) => {
      this.receive(event.payload);
    });

    this.unlistenEnded ??= await this.listen<string>(TerminalManager.ENDED, (event) => {
      if (event.payload === this.pane) {
        this.reattach();
      }
    });
  }

  private async attach(): Promise<void> {
    const pane = this.pane;

    if (pane === null) {
      return;
    }

    try {
      await this.invoke("attach_pane", {
        server: this.server,
        pane,
        cols: this.cols,
        rows: this.rows,
      });

      this.liveStore.set(true);
      this.errorStore.set(null);
    } catch (error) {
      this.liveStore.set(false);
      this.refuse(String(error));
    }
  }

  private async detach(): Promise<void> {
    const pane = this.pane;

    if (pane === null) {
      return;
    }

    this.liveStore.set(false);

    try {
      await this.invoke("detach_pane", { pane });
    } catch {
      // A detach that fails changes nothing worth telling somebody about: the
      // stream is going either way, and the machine treats a dropped one as a
      // peer that left.
    }
  }

  private receive(event: PaneFrame): void {
    // Addressed, because a frame for another pane would be applied to this grid
    // and look exactly like output.
    if ((event.pane as unknown as string) !== this.pane) {
      return;
    }

    if (this.grid.apply(event.frame)) {
      this.revisionStore.update((held) => held + 1);
    }
  }

  private reattach(): void {
    if (this.closed) {
      return;
    }

    this.liveStore.set(false);

    if (this.retry) {
      clearTimeout(this.retry);
    }

    this.retry = setTimeout(() => {
      this.retry = null;
      void this.attach();
    }, TerminalManager.REATTACH_WAIT);
  }

  private async deliver(command: string, payload: Record<string, unknown>): Promise<void> {
    if (this.pane === null) {
      this.refuse("no pane is open");

      return;
    }

    try {
      await this.invoke(command, { pane: this.pane, ...payload });
      this.errorStore.set(null);
    } catch (error) {
      this.refuse(String(error));
    }
  }

  /**
   * Records why something did not happen.
   *
   * Every refusal goes through here rather than returning quietly. A control
   * that sometimes does nothing, with no message, is indistinguishable from a
   * machine that has stopped answering - and on a terminal it is worse, because
   * a keystroke that vanished leaves no trace at all.
   */
  private refuse(why: string): void {
    console.error("terminal refused", why);
    this.errorStore.set(why);
  }
}
