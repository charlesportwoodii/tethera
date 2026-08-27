import { writable, type Readable, type Writable } from "svelte/store";
import type { Conversation } from "$bindings/Conversation";
import type { MachineEvent } from "$bindings/MachineEvent";
import type { MachineTree } from "$bindings/MachineTree";
import type { Pane } from "$bindings/Pane";
import type { Tab } from "$bindings/Tab";
import type { TabLayout } from "$bindings/TabLayout";
import type { WatchEvent } from "$bindings/WatchEvent";
import type { Workspace } from "$bindings/Workspace";
import type { Invoke } from "./server_manager";
import type { Listen } from "./terminal_manager";

/**
 * One machine's tree, kept live.
 *
 * The machine does not push this: it diffs successive reads of its own backend
 * and publishes the differences. What a subscription buys is that those reads
 * happen while somebody is watching, rather than only when a screen asks — which
 * is why a tab closed at the desk used to stay on the phone until something
 * unrelated triggered a fetch.
 */
export class MachineManager {
  /** Where events arrive. Matches `MachineWatch::CHANNEL`. */
  static readonly CHANNEL = "machine";

  /** Where an unasked-for end arrives. Matches `MachineWatch::ENDED`. */
  static readonly ENDED = "machine_watch_ended";

  /**
   * How long to wait before opening another subscription.
   *
   * The same 1200 ms `TerminalManager` waits to re-attach, for the same reason:
   * whatever closed the path is usually about a second from being reachable
   * again, and an instant retry spends the one attempt that would have worked.
   */
  static readonly REOPEN_WAIT = 1200;

  private readonly workspaceStore: Writable<Workspace[]>;
  private readonly tabStore: Writable<Tab[]>;
  private readonly paneStore: Writable<Pane[]>;
  private readonly conversationStore: Writable<Conversation[]>;
  private readonly layoutStore: Writable<TabLayout[]>;
  private readonly liveStore: Writable<boolean>;
  private readonly errorStore: Writable<string | null>;

  public readonly workspaces: Readable<Workspace[]>;
  public readonly tabs: Readable<Tab[]>;
  public readonly panes: Readable<Pane[]>;
  public readonly conversations: Readable<Conversation[]>;
  public readonly layouts: Readable<TabLayout[]>;
  /** Whether a subscription is open. False means the tree may already be stale. */
  public readonly live: Readable<boolean>;
  public readonly error: Readable<string | null>;

  private held: MachineTree | null = null;
  private unlisten: (() => void) | null = null;
  private unlistenEnded: (() => void) | null = null;
  private retry: ReturnType<typeof setTimeout> | null = null;
  private closed = false;

  constructor(
    private readonly invoke: Invoke,
    private readonly listen: Listen,
    private readonly server: string,
  ) {
    this.workspaceStore = writable([]);
    this.tabStore = writable([]);
    this.paneStore = writable([]);
    this.conversationStore = writable([]);
    this.layoutStore = writable([]);
    this.liveStore = writable(false);
    this.errorStore = writable(null);

    this.workspaces = { subscribe: this.workspaceStore.subscribe };
    this.tabs = { subscribe: this.tabStore.subscribe };
    this.panes = { subscribe: this.paneStore.subscribe };
    this.conversations = { subscribe: this.conversationStore.subscribe };
    this.layouts = { subscribe: this.layoutStore.subscribe };
    this.live = { subscribe: this.liveStore.subscribe };
    this.error = { subscribe: this.errorStore.subscribe };
  }

  /** Subscribes, and fills every rank from the snapshot the machine opens with. */
  async open(): Promise<void> {
    this.closed = false;

    await this.subscribe();

    try {
      const tree = (await this.invoke("watch_machine", {
        server: this.server,
      })) as MachineTree;

      this.adopt(tree);
      this.liveStore.set(true);
      this.errorStore.set(null);
    } catch (error) {
      this.liveStore.set(false);
      this.refuse(String(error));
    }
  }

  /**
   * Stops following. The machine keeps running.
   *
   * Sets `closed` before anything else, so a reopen that fires between here and
   * the timer being cleared finds a manager that has gone.
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
    this.liveStore.set(false);

    void this.invoke("unwatch_machine").catch(() => {
      // A stop that fails changes nothing worth telling somebody about: the
      // stream is going either way, and the machine treats a dropped one as a
      // peer that left.
    });
  }

  tabsOf(workspace: string): Tab[] {
    return (this.held?.tabs ?? []).filter(
      (tab) => (tab.workspace_id as unknown as string) === workspace,
    );
  }

  panesOf(tab: string): Pane[] {
    return (this.held?.panes ?? []).filter(
      (pane) => (pane.tab_id as unknown as string) === tab,
    );
  }

  layoutOf(tab: string): TabLayout | null {
    return (
      (this.held?.layouts ?? []).find(
        (layout) => (layout.tab as unknown as string) === tab,
      ) ?? null
    );
  }

  /**
   * Applies a change to one rank.
   *
   * An upsert, not a replace-if-present. The wire has no "added" event: a change
   * naming an id the client has not seen is an insert, and treating it as a miss
   * is why a tab created at the desk never appeared in the strip.
   */
  private static upsert<T extends { id: unknown }>(held: T[], item: T): T[] {
    const at = held.findIndex((seen) => seen.id === item.id);

    if (at < 0) {
      return [...held, item];
    }

    return held.map((seen, index) => (index === at ? item : seen));
  }

  private static without<T extends { id: unknown }>(held: T[], id: unknown): T[] {
    return held.filter((seen) => seen.id !== id);
  }

  private async subscribe(): Promise<void> {
    this.unlisten ??= await this.listen<MachineEvent>(MachineManager.CHANNEL, (event) => {
      // Addressed, because one webview holds several machines and an event
      // applied to the wrong tree looks exactly like the right tree changing.
      if (event.payload.server !== this.server) {
        return;
      }

      this.apply(event.payload.event);
    });

    this.unlistenEnded ??= await this.listen<string>(MachineManager.ENDED, (event) => {
      if (event.payload === this.server) {
        this.reopen();
      }
    });
  }

  private adopt(tree: MachineTree): void {
    this.held = tree;
    this.workspaceStore.set(tree.workspaces);
    this.tabStore.set(tree.tabs);
    this.paneStore.set(tree.panes);
    this.conversationStore.set(tree.conversations);
    this.layoutStore.set(tree.layouts);
  }

  private apply(event: WatchEvent): void {
    const held = this.held;

    if (held === null) {
      return;
    }

    if ("workspace_changed" in event) {
      held.workspaces = MachineManager.upsert(held.workspaces, event.workspace_changed);
      this.workspaceStore.set(held.workspaces);
    } else if ("workspace_removed" in event) {
      held.workspaces = MachineManager.without(held.workspaces, event.workspace_removed);
      this.workspaceStore.set(held.workspaces);
    } else if ("tab_changed" in event) {
      held.tabs = MachineManager.upsert(held.tabs, event.tab_changed);
      this.tabStore.set(held.tabs);
    } else if ("tab_removed" in event) {
      held.tabs = MachineManager.without(held.tabs, event.tab_removed);
      this.tabStore.set(held.tabs);
    } else if ("pane_changed" in event) {
      held.panes = MachineManager.upsert(held.panes, event.pane_changed);
      this.paneStore.set(held.panes);
    } else if ("pane_removed" in event) {
      held.panes = MachineManager.without(held.panes, event.pane_removed);
      this.paneStore.set(held.panes);
    } else if ("conversation_changed" in event) {
      held.conversations = MachineManager.upsert(
        held.conversations,
        event.conversation_changed,
      );
      this.conversationStore.set(held.conversations);
    } else if ("conversation_removed" in event) {
      held.conversations = MachineManager.without(
        held.conversations,
        event.conversation_removed,
      );
      this.conversationStore.set(held.conversations);
    } else if ("layout_changed" in event) {
      // Carried whole rather than as a delta, so it replaces its own tab's entry
      // and touches no other. A delta against a layout this client may have
      // missed would be a hole it cannot see.
      const next = event.layout_changed;
      const at = held.layouts.findIndex((layout) => layout.tab === next.tab);

      held.layouts =
        at < 0
          ? [...held.layouts, next]
          : held.layouts.map((layout, index) => (index === at ? next : layout));

      this.layoutStore.set(held.layouts);
    }
  }

  /**
   * Opens another subscription after an unasked-for end.
   *
   * The reopen replaces the whole tree from the fresh snapshot rather than
   * merging into what is held: the client missed events while it was away and
   * has no way to know which.
   */
  private reopen(): void {
    if (this.closed) {
      return;
    }

    this.liveStore.set(false);

    if (this.retry) {
      clearTimeout(this.retry);
    }

    this.retry = setTimeout(() => {
      this.retry = null;
      void this.open();
    }, MachineManager.REOPEN_WAIT);
  }

  /**
   * Records why something did not happen.
   *
   * Every refusal goes through here rather than returning quietly. A tree that
   * silently stopped updating is indistinguishable from a machine where nothing
   * is happening, which is the whole failure this manager was written to end.
   */
  private refuse(why: string): void {
    console.error("machine watch refused", why);
    this.errorStore.set(why);
  }
}
