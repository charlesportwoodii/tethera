import { writable, type Readable, type Writable } from "svelte/store";
import type { Conversation } from "$bindings/Conversation";
import type { ConversationControls } from "$bindings/ConversationControls";
import type { ConversationEvent } from "$bindings/ConversationEvent";
import type { Page } from "$bindings/Page";
import type { AgentStats } from "$bindings/AgentStats";
import type { AssetPreview } from "$bindings/AssetPreview";
import type { Answer } from "$bindings/Answer";
import type { Attached } from "$bindings/Attached";
import type { Question } from "$bindings/Question";
import type { SendOutcome } from "$bindings/SendOutcome";
import type { Turn } from "$bindings/Turn";
import type { Invoke } from "./server_manager";

/** Subscribes to an event channel and returns the function that stops it. */
export type Listen = (
  channel: string,
  handler: (event: { payload: ConversationEvent }) => void,
) => Promise<() => void>;

const NOTHING: ConversationControls = {
  // One turn, so a machine that will not describe itself still shows something
  // rather than asking for a page of zero.
  transcript_page: 1,
  send: false,
  answer: false,
  interrupt: false,
  resume: false,
  stop: false,
  read_files: false,
  attach_files: false,
};

/**
 * One conversation: its history, its live tail, and what can be done to it.
 *
 * Turns are held oldest first, which is the order they are drawn in. The
 * machine answers a transcript page oldest first too, so nothing is reversed
 * on the way in.
 */
export class TranscriptManager {
  private readonly turnStore: Writable<Turn[]>;
  private readonly conversationStore: Writable<Conversation | null>;
  private readonly controlStore: Writable<ConversationControls>;
  private readonly blockedStore: Writable<Question | null>;
  private readonly loadingStore: Writable<boolean>;
  private readonly earlierStore: Writable<boolean>;
  private readonly errorStore: Writable<string | null>;
  private readonly liveStore: Writable<boolean>;
  private readonly resumingStore: Writable<boolean>;
  private readonly statsStore: Writable<AgentStats | null>;

  public readonly turns: Readable<Turn[]>;
  public readonly conversation: Readable<Conversation | null>;
  public readonly controls: Readable<ConversationControls>;
  /** The question the agent is waiting on, if it is waiting on one. */
  public readonly blocked: Readable<Question | null>;
  public readonly loading: Readable<boolean>;
  public readonly hasEarlier: Readable<boolean>;
  public readonly error: Readable<string | null>;
  /** Whether the live tail is attached. */
  public readonly live: Readable<boolean>;
  public readonly resuming: Readable<boolean>;
  /**
   * What the agent is doing right now, in figures.
   *
   * `null` between turns. The machine sends these when they change rather than
   * on a timer, so a screen ticks its own clock from `turn_started_at` instead
   * of waiting to be told the time.
   */
  public readonly stats: Readable<AgentStats | null>;

  /** How many times to wait for a conversation to write its first record. */
  private static readonly ATTACH_TRIES = 4;
  private static readonly ATTACH_WAIT = 2500;

  /** How long to leave a tail that dropped before opening another. */
  private static readonly REATTACH_WAIT = 1200;

  /**
   * How long figures stay believable without an update.
   *
   * The machine sends them when they change, and a turn that has ended sends
   * nothing at all - so silence is the only signal that it is over. Without a
   * limit the last set of a finished turn sits on screen for ever, and a
   * spinner over a stopped agent is worse than no row: it says work is
   * happening when none is.
   *
   * Generous, because a single long tool call legitimately moves no figure.
   */
  private static readonly STATS_STALE = 45_000;

  private oldest: string | null = null;
  private unlisten: (() => void) | null = null;
  private unlistenEnded: (() => void) | null = null;
  private closed = false;
  private attempts = 0;
  /** When the last figures arrived, for deciding they have gone stale. */
  private statsAt = 0;
  private retry: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly invoke: Invoke,
    private readonly listen: Listen,
    private readonly server: string,
    private readonly id: string,
  ) {
    this.turnStore = writable([]);
    this.conversationStore = writable(null);
    this.controlStore = writable(NOTHING);
    this.blockedStore = writable(null);
    this.loadingStore = writable(false);
    this.earlierStore = writable(false);
    this.errorStore = writable(null);
    this.liveStore = writable(false);
    this.resumingStore = writable(false);
    this.statsStore = writable(null);

    this.turns = { subscribe: this.turnStore.subscribe };
    this.conversation = { subscribe: this.conversationStore.subscribe };
    this.controls = { subscribe: this.controlStore.subscribe };
    this.blocked = { subscribe: this.blockedStore.subscribe };
    this.loading = { subscribe: this.loadingStore.subscribe };
    this.hasEarlier = { subscribe: this.earlierStore.subscribe };
    this.error = { subscribe: this.errorStore.subscribe };
    this.live = { subscribe: this.liveStore.subscribe };
    this.resuming = { subscribe: this.resumingStore.subscribe };
    this.stats = { subscribe: this.statsStore.subscribe };
  }

  /**
   * Draws the screen: what can be done, the tail, then history.
   *
   * The watch comes before the page because its snapshot carries the
   * conversation itself — the title, the status, whether a pane is attached —
   * and that is the whole header. Paging first leaves the screen anonymous for
   * as long as the transcript read takes.
   *
   * Nothing is lost by the order: turns are merged by id, so one arriving live
   * before the page that also contains it is the same turn either way.
   */
  async open(): Promise<void> {
    this.loadingStore.set(true);
    this.errorStore.set(null);

    try {
      await this.loadControls();
      await this.attach();
    } catch (error) {
      this.errorStore.set(String(error));
    }

    try {
      await this.older();
    } finally {
      this.loadingStore.set(false);
    }
  }

  /**
   * Fills the header when the live tail could not be opened.
   *
   * Quiet on failure: this is a fallback, and a second error about the same
   * unreachable machine adds nothing to the first.
   */
  private async describe(): Promise<void> {
    try {
      const conversation = (await this.invoke("get_conversation", {
        server: this.server,
        conversation: this.id,
      })) as Conversation;

      this.conversationStore.set(conversation);
    } catch {
      // Nothing to add.
    }
  }

  private async loadControls(): Promise<void> {
    try {
      const controls = (await this.invoke("conversation_controls", {
        server: this.server,
      })) as ConversationControls;

      this.controlStore.set(controls ?? NOTHING);
    } catch {
      // A machine that will not say what it allows is treated as allowing
      // nothing. Assuming otherwise draws controls that fail on press.
      this.controlStore.set(NOTHING);
    }
  }

  /**
   * One page further back.
   *
   * A failure keeps whatever is already on screen. Losing the next page is not
   * a reason to discard the pages that arrived.
   */
  async older(): Promise<void> {
    try {
      const page = (await this.invoke("conversation_transcript", {
        server: this.server,
        conversation: this.id,
        before: this.oldest,
        // The machine's own ceiling. It bounds a page by bytes as well, so this
        // returns however many fit and reads no more than it sends.
        limit: this.pageSize(),
      })) as Page<Turn>;

      this.absorb(page.items);
      this.oldest = (page.next_before as unknown as string | null) ?? null;
      this.earlierStore.set(page.has_earlier);
    } catch (error) {
      this.errorStore.set(String(error));
    }
  }

  /**
   * Opens the live tail, resuming after the newest turn already held.
   *
   * A conversation that has begun no turn has no records to watch, so the
   * machine ends the watch rather than opening it. That is the ordinary state of
   * a session somebody has just started, so it is retried rather than reported:
   * the agent writes its first record within seconds, and the header is filled
   * from the machine's description meanwhile.
   */
  private async attach(): Promise<void> {
    this.unlisten ??= await this.listen("conversation", (event) => {
      this.receive(event.payload);
    });

    this.unlistenEnded ??= await (
      this.listen as unknown as (
        channel: string,
        handler: (event: { payload: string }) => void,
      ) => Promise<() => void>
    )("conversation_tail_ended", (event) => {
      if (event.payload === this.id) {
        this.reattach();
      }
    });

    const asked = this.newestCursor();

    let answer: [Conversation, string];

    try {
      answer = (await this.invoke("watch_conversation", {
        server: this.server,
        conversation: this.id,
        after: asked,
      })) as [Conversation, string];
    } catch (error) {
      this.liveStore.set(false);
      await this.describe();

      if (this.attempts < TranscriptManager.ATTACH_TRIES) {
        this.attempts += 1;
        this.retry = setTimeout(() => void this.attach(), TranscriptManager.ATTACH_WAIT);

        return;
      }

      this.errorStore.set(String(error));

      return;
    }

    this.attempts = 0;

    const [conversation, from] = answer;

    this.conversationStore.set(conversation);
    this.liveStore.set(true);

    // The machine says where the stream actually starts. Later than what was
    // asked for means the held cursor predates the earliest surviving record,
    // so what is on screen is not continuous with what is about to arrive.
    // Refetching beats drawing a hole that looks like an ending. On a first
    // open nothing is held, so there is nothing to be discontinuous with.
    if (asked !== null && from !== asked) {
      this.turnStore.set([]);
      this.oldest = null;
      await this.older();
    }
  }

  /**
   * Applies one live event.
   *
   * Events for other conversations are dropped rather than trusted. One process
   * can hold several watches, and they all arrive on the same channel.
   */
  private receive(notice: ConversationEvent): void {
    if ((notice.conversation as unknown as string) !== this.id) {
      return;
    }

    const event = notice.event;

    if ("turn" in event) {
      this.absorb([event.turn]);

      return;
    }

    if ("stats" in event) {
      this.statsStore.set(event.stats);
      this.statsAt = Date.now();

      return;
    }

    if ("conversation_changed" in event) {
      const changed = event.conversation_changed;

      if ((changed.id as unknown as string) === this.id) {
        this.conversationStore.set(changed);

        // Figures describe a turn in progress. Holding the last set past the
        // end of one leaves a finished agent showing a token count that will
        // never move again, which reads as work still happening.
        if (changed.status !== "working") {
          this.statsStore.set(null);
        }
      }

      return;
    }

    if ("blocked" in event) {
      this.blockedStore.set(event.blocked.question);

      return;
    }

    if ("unblocked" in event) {
      this.blockedStore.set(null);

      return;
    }

    if ("conversation_removed" in event) {
      this.liveStore.set(false);
    }
  }

  /**
   * Merges turns into the list, newest last, without duplicating.
   *
   * A turn id is stable across reads, which is what makes this safe: the same
   * turn reaching the screen from both a page and the tail is one turn, and the
   * later copy is the more complete one.
   */
  private absorb(arriving: Turn[]): void {
    if (arriving.length === 0) {
      return;
    }

    this.turnStore.update((held) => {
      const byId = new Map<string, Turn>();

      for (const turn of [...held, ...arriving]) {
        byId.set(turn.id as unknown as string, turn);
      }

      return [...byId.values()].sort((left, right) => Number(left.at) - Number(right.at));
    });
  }

  /** Whether the figures are recent enough to draw. */
  freshStats(now: number): boolean {
    return this.statsAt > 0 && now - this.statsAt < TranscriptManager.STATS_STALE;
  }

  private pageSize(): number {
    let size = NOTHING.transcript_page;

    const stop = this.controlStore.subscribe((held) => {
      size = held.transcript_page;
    });
    stop();

    return size;
  }

  private newestCursor(): string | null {
    let newest: string | null = null;

    const stop = this.turnStore.subscribe((held) => {
      newest = held.length > 0 ? (held[held.length - 1].cursor as unknown as string) : null;
    });
    stop();

    return newest;
  }

  /**
   * Sends a message, and says whether there was anybody to receive it.
   *
   * `"not_running"` is not a failure: the conversation has no pane, so the
   * answer is to offer a resume rather than to report a send that did not work.
   */
  async send(text: string, attachments: string[] = []): Promise<SendOutcome | null> {
    // A message carrying a file is worth sending with no words. A bare empty
    // one is not.
    if (text.trim().length === 0 && attachments.length === 0) {
      return null;
    }

    this.errorStore.set(null);

    try {
      return (await this.invoke("send_prompt", {
        server: this.server,
        conversation: this.id,
        text,
        attachments,
      })) as SendOutcome;
    } catch (error) {
      this.errorStore.set(String(error));

      return null;
    }
  }

  /**
   * Starts an agent again on a conversation that has stopped.
   *
   * The conversation keeps its id across a resume, so every cursor already held
   * stays valid and the transcript continues rather than forking. Nothing here
   * is reset.
   */
  async resume(): Promise<boolean> {
    this.errorStore.set(null);
    this.resumingStore.set(true);

    try {
      const conversation = (await this.invoke("resume_conversation", {
        server: this.server,
        conversation: this.id,
      })) as Conversation;

      this.conversationStore.set(conversation);

      return true;
    } catch (error) {
      this.errorStore.set(String(error));

      return false;
    } finally {
      this.resumingStore.set(false);
    }
  }

  /**
   * Answers the question the agent is waiting on.
   *
   * The fingerprint travels back exactly as it arrived. The machine refuses a
   * stale one rather than answering a question that has changed underneath.
   */
  /**
   * Says why something this screen decided cannot be sent.
   *
   * The same line the machine's own refusals reach, on purpose. A person cannot
   * tell whose rule stopped an answer and should not have to: what they need is
   * that a control they pressed did nothing, and why.
   */
  refuse(why: string): void {
    // Also to the console, which on a phone is the only place a failure can
    // be read back after the fact.
    console.error("answer refused", why);
    this.errorStore.set(why);
  }

  async answer(question: Question, answers: Answer[], fingerprint?: string): Promise<boolean> {
    this.errorStore.set(null);

    try {
      await this.invoke("answer_question", {
        server: this.server,
        conversation: this.id,
        question: question.id as unknown as string,
        // The fingerprint the control carried, not the one held here. A card
        // drawn from the transcript and a live `Blocked` event can describe the
        // same question at different moments, and the machine is the thing
        // entitled to judge which is current.
        fingerprint: fingerprint ?? (question.fingerprint as unknown as string),
        answers,
      });

      return true;
    } catch (error) {
      const text = String(error);

      console.error("answer_question failed", text);

      // The set moved while it was being read. Not a fault, and not something
      // to show as one: the watch carries the current question, so the screen
      // is already about to be right.
      this.errorStore.set(
        /stale/i.test(text)
          ? "That question changed while you were answering. The current one is below."
          : text,
      );

      return false;
    }
  }

  /**
   * Picks a file and stages it on the machine.
   *
   * Staged, not delivered: this answers an id, and the id reaches the agent only
   * when a prompt is sent carrying it. `null` means the picker was dismissed,
   * which is not a failure.
   */
  async stageFile(): Promise<Attached | null> {
    this.errorStore.set(null);

    try {
      return (await this.invoke("attach_file", { server: this.server })) as Attached | null;
    } catch (error) {
      this.errorStore.set(String(error));

      return null;
    }
  }

  /**
   * The head of a file, for showing it without saving it.
   *
   * The machine drops the stream once enough has arrived, so opening a large
   * file costs a chunk rather than the file.
   */
  async preview(asset: string, limit: number, mime: string | null): Promise<AssetPreview | null> {
    this.errorStore.set(null);

    try {
      return (await this.invoke("preview_asset", {
        server: this.server,
        asset,
        limit,
        mime,
      })) as AssetPreview;
    } catch (error) {
      this.errorStore.set(String(error));

      return null;
    }
  }

  async interrupt(): Promise<void> {
    this.errorStore.set(null);

    try {
      await this.invoke("interrupt_conversation", {
        server: this.server,
        conversation: this.id,
      });
    } catch (error) {
      this.errorStore.set(String(error));
    }
  }

  /**
   * Ends the subscription.
   *
   * Both halves matter: the listener stops the webview handling events for a
   * screen that is gone, and the command tells the machine, which is what
   * distinguishes a closed screen from a phone that lost its route.
   */
  /**
   * Opens the tail again after it stopped on its own.
   *
   * A QUIC path closed for being idle is the ordinary fate of a phone left
   * alone for a minute, and until this existed it ended the live tail for the
   * life of the screen. Nothing on screen said so: the header still claimed to
   * be following, and the last question the machine had reported stayed put -
   * so answering it sent nothing, because the live question an answer is
   * matched against had stopped arriving.
   *
   * The attempt counter is reset first. It bounds one burst of retries, not the
   * lifetime of a screen, and a tail that has already recovered twice today has
   * earned a third go.
   */
  private reattach(): void {
    if (this.closed) {
      return;
    }

    this.liveStore.set(false);
    this.attempts = 0;

    if (this.retry) {
      clearTimeout(this.retry);
    }

    // Not immediately. Whatever closed the path is a second or so from being
    // over, and dialling into it is how a reconnect becomes a loop.
    this.retry = setTimeout(() => void this.attach(), TranscriptManager.REATTACH_WAIT);
  }

  async close(): Promise<void> {
    this.closed = true;

    if (this.retry) {
      clearTimeout(this.retry);
      this.retry = null;
    }

    if (this.unlisten) {
      this.unlisten();
      this.unlisten = null;
    }

    if (this.unlistenEnded) {
      this.unlistenEnded();
      this.unlistenEnded = null;
    }

    this.liveStore.set(false);

    try {
      await this.invoke("unwatch_conversation", { conversation: this.id });
    } catch {
      // The screen is going away regardless. A machine that did not hear the
      // close will see the stream reset, which it already handles.
    }
  }
}
