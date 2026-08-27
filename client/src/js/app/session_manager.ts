import { writable, type Readable, type Writable } from "svelte/store";
import type { AgentProfile } from "$bindings/AgentProfile";
import type { StartOutcome } from "$bindings/StartOutcome";
import type { ConversationPreview } from "$bindings/ConversationPreview";
import type { Invoke } from "./server_manager";

/** How many remembered directories to ask a machine for. */
const RECENT_LIMIT = 8;

export interface SessionDraft {
  serverId: string | null;
  profile: AgentProfile | null;
  cwd: string;
  prompt: string;
}

export type StartState =
  | { step: "idle" }
  | { step: "starting" }
  /**
   * The machine answered. `outcome` says whether a conversation exists or the
   * harness is up and waiting to be answered at the machine — both are the call
   * succeeding, and neither is a failure.
   */
  | { step: "started"; outcome: StartOutcome }
  | { step: "failed"; reason: string };

/**
 * The new-session form.
 *
 * Nothing here starts anything. Every choice is local until `start` is called,
 * which is what lets the screen promise that nothing runs until the button is
 * pressed.
 */
export class SessionManager {
  private readonly draftStore: Writable<SessionDraft>;
  private readonly profileStore: Writable<AgentProfile[]>;
  private readonly stateStore: Writable<StartState>;
  private readonly canStartStore: Writable<boolean>;
  private readonly loadingStore: Writable<boolean>;
  private readonly recentStore: Writable<string[]>;
  private readonly previewStore: Writable<ConversationPreview | null>;

  public readonly draft: Readable<SessionDraft>;
  public readonly profiles: Readable<AgentProfile[]>;
  public readonly state: Readable<StartState>;
  /** Whether the machine advertises that it can start a session at all. */
  public readonly canStart: Readable<boolean>;
  public readonly loading: Readable<boolean>;
  /** Directories this machine has been worked in, newest first. */
  public readonly recent: Readable<string[]>;
  /** Where a start would land, answered by the machine that would create it. */
  public readonly preview: Readable<ConversationPreview | null>;

  constructor(private readonly invoke: Invoke) {
    this.draftStore = writable({
      serverId: null,
      profile: null,
      cwd: "",
      prompt: "",
    });
    this.profileStore = writable([]);
    this.stateStore = writable({ step: "idle" });
    this.canStartStore = writable(false);
    this.loadingStore = writable(false);
    this.recentStore = writable([]);
    this.previewStore = writable(null);

    this.draft = { subscribe: this.draftStore.subscribe };
    this.profiles = { subscribe: this.profileStore.subscribe };
    this.state = { subscribe: this.stateStore.subscribe };
    this.canStart = { subscribe: this.canStartStore.subscribe };
    this.loading = { subscribe: this.loadingStore.subscribe };
    this.recent = { subscribe: this.recentStore.subscribe };
    this.preview = { subscribe: this.previewStore.subscribe };
  }

  /**
   * Points the form at a machine and asks what it can run.
   *
   * Choosing a different machine clears the harness: a `ProfileId` is that
   * machine's own, and carrying one across would hand a machine an id it has
   * never heard of.
   */
  async chooseServer(serverId: string): Promise<void> {
    this.draftStore.update((held) => ({
      ...held,
      serverId,
      profile: null,
    }));

    this.previewStore.set(null);
    this.recentStore.set([]);
    this.loadingStore.set(true);

    // Its own call, and its own failure. A machine that cannot remember where it
    // has been worked can still start a session, so losing this must not empty
    // the harness list beside it.
    void this.loadRecent(serverId);

    try {
      const [profiles, allowed] = await Promise.all([
        this.invoke("list_agent_profiles", { id: serverId }) as Promise<AgentProfile[]>,
        this.invoke("can_start_sessions", { id: serverId }) as Promise<boolean>,
      ]);

      this.profileStore.set(profiles);
      this.canStartStore.set(allowed);

      // One harness is not a choice. Pre-selecting it saves a tap and leaves the
      // directory as the only thing still to answer.
      if (profiles.length === 1) {
        this.chooseProfile(profiles[0]);
      }
    } catch (error) {
      this.profileStore.set([]);
      this.canStartStore.set(false);
      this.stateStore.set({ step: "failed", reason: String(error) });
    } finally {
      this.loadingStore.set(false);
    }
  }

  private async loadRecent(serverId: string): Promise<void> {
    try {
      const paths = (await this.invoke("recent_cwds", {
        id: serverId,
        limit: RECENT_LIMIT,
      })) as string[];

      // Only if the person has not moved on. A slow answer for the machine they
      // just left would offer directories from the wrong one.
      if (this.current().serverId === serverId) {
        this.recentStore.set(Array.isArray(paths) ? paths : []);
      }
    } catch {
      this.recentStore.set([]);
    }
  }

  /**
   * Asks where a start would land, without starting it.
   *
   * The workspace and tab names are the machine's to generate, and
   * `will_have_transcript` can depend on the directory rather than only on the
   * harness — which is why this is asked again once a directory is typed.
   */
  async refreshPreview(): Promise<void> {
    const draft = this.current();

    if (!draft.serverId || !draft.profile || draft.cwd.trim().length === 0) {
      this.previewStore.set(null);

      return;
    }

    const asked = { server: draft.serverId, profile: draft.profile.id, cwd: draft.cwd.trim() };

    try {
      const preview = (await this.invoke("preview_conversation", {
        id: asked.server,
        profile: asked.profile,
        cwd: asked.cwd,
      })) as ConversationPreview | null;

      const now = this.current();

      // A preview describes one machine, one harness and one directory. Showing
      // a late answer against a changed form would name a workspace that this
      // start will not use.
      if (
        now.serverId === asked.server &&
        now.profile?.id === asked.profile &&
        now.cwd.trim() === asked.cwd
      ) {
        this.previewStore.set(preview);
      }
    } catch {
      this.previewStore.set(null);
    }
  }

  chooseProfile(profile: AgentProfile): void {
    this.draftStore.update((held) => ({ ...held, profile }));
  }

  setCwd(cwd: string): void {
    this.draftStore.update((held) => ({ ...held, cwd }));
  }

  setPrompt(prompt: string): void {
    this.draftStore.update((held) => ({ ...held, prompt }));
  }

  /** A machine, a harness and somewhere to run. The first message is optional. */
  static isComplete(draft: SessionDraft): boolean {
    return (
      draft.serverId !== null && draft.profile !== null && draft.cwd.trim().length > 0
    );
  }

  async start(): Promise<void> {
    const draft = this.current();

    if (!SessionManager.isComplete(draft) || !draft.profile || !draft.serverId) {
      return;
    }

    this.stateStore.set({ step: "starting" });

    try {
      const outcome = (await this.invoke("start_conversation", {
        id: draft.serverId,
        profile: draft.profile.id,
        cwd: draft.cwd.trim(),
        prompt: draft.prompt.trim() === "" ? null : draft.prompt,
      })) as StartOutcome;

      this.stateStore.set({ step: "started", outcome });
    } catch (error) {
      this.stateStore.set({ step: "failed", reason: String(error) });
    }
  }

  private current(): SessionDraft {
    let held: SessionDraft = {
      serverId: null,
      profile: null,
      cwd: "",
      prompt: "",
    };

    const stop = this.draftStore.subscribe((value) => {
      held = value;
    });
    stop();

    return held;
  }
}
