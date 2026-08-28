<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import {
    QuestionCard,
    BrailleSpinner,
    FilePreview,
    FileViewer,
    Button,
    Composer,
    Label,
    NavBar,
    PartView,
    QuestionFlow,
    ThinkingRow,
    Timeline,
    ToolFold,
    Turn,
  } from "$console";
  import { previewKind } from "$console";
  import HtmlPreview from "$components/HtmlPreview.svelte";
  import MarkdownBody from "$components/MarkdownBody.svelte";
  import { MarkdownTables } from "$managers/tables";
  import { Activity } from "$managers/activity";
  import { Parts } from "$managers/parts";
  import { Scroll } from "$managers/scroll";
  import { TranscriptManager, type Listen } from "$managers/transcript_manager";
  import { DownloadManager, type ListenDownloads } from "$managers/download_manager";
  import DownloadTray from "$components/DownloadTray.svelte";
  import ConversationGlyph from "$components/ConversationGlyph.svelte";
  import { Conversations } from "$managers/conversations";
  import ComposerRail from "$components/ComposerRail.svelte";
  import LayoutSheet from "$components/LayoutSheet.svelte";
  import { Floorplan } from "$managers/floorplan";
  import { MachineManager } from "$managers/machine_manager";
  import { TerminalManager, type Listen as TreeListen } from "$managers/terminal_manager";
  import type { Answer } from "$bindings/Answer";
  import type { AssetPreview } from "$bindings/AssetPreview";
  import type { Attached } from "$bindings/Attached";
  import type { Question } from "$bindings/Question";
  import type { Turn as TurnRecord } from "$bindings/Turn";

  const server = page.url.searchParams.get("server") ?? "";
  const id = page.url.searchParams.get("id") ?? "";

  const manager = new TranscriptManager(invoke, listen as unknown as Listen, server, id);
  const turns = manager.turns;
  const conversation = manager.conversation;
  const controls = manager.controls;
  const blocked = manager.blocked;
  const loading = manager.loading;
  const hasEarlier = manager.hasEarlier;
  const failure = manager.error;
  const live = manager.live;
  const resuming = manager.resuming;
  const stats = manager.stats;

  // Its own manager rather than a method on the transcript's. A download
  // outlives the screen that started it - the transfer runs in a task on the
  // Rust side and reports on a channel - so what a screen holds is a view of
  // every transfer this app is carrying, including ones it never asked for.
  const downloads = new DownloadManager(invoke, listen as unknown as ListenDownloads);
  const carrying = downloads.rows;

  // The machine's tree, for one purpose: three agents can share one herdr tab,
  // and this is how the second one is reached. Without it the only way from C1
  // to C2 is back out to the machine screen and in again.
  const tree = new MachineManager(invoke, listen as unknown as TreeListen, server);
  const terminals = new TerminalManager(invoke, listen as unknown as TreeListen, server, 1, 1);
  const terminalControls = terminals.controls;
  const treePanes = tree.panes;
  const treeLayouts = tree.layouts;

  /** The pane this conversation is running in, when the machine says so. */
  const mine = $derived(
    $treePanes.find((pane) => (pane.conversation as unknown as string) === id) ?? null,
  );

  const tabPanes = $derived(
    mine === null
      ? []
      : $treePanes.filter((pane) => pane.tab_id === mine.tab_id),
  );

  const tabLayout = $derived(
    mine === null
      ? null
      : ($treeLayouts.find((layout) => layout.tab === mine.tab_id) ?? null),
  );

  /** Open over the transcript when somebody asks which agents are in this tab. */
  let mapping = $state(false);
  let selectedPane = $state<string | null>(null);

  let draft = $state("");
  let sending = $state(false);
  let doc: HTMLDivElement | null = $state(null);

  /**
   * How tall the file sheet opens, and how far a drag may take it.
   *
   * The floor leaves the sheet unmistakably present — dragging it to nothing
   * would be a close by another name, and there is already a close.
   */
  const SHEET_OPEN = 86;
  const SHEET_MIN = 25;
  const SHEET_MAX = 94;

  function sizeSheet(percent: number): void {
    const held = Math.min(SHEET_MAX, Math.max(SHEET_MIN, percent));

    document.documentElement.style.setProperty("--tethera-sheet", `${held}%`);
  }

  /**
   * Drag-to-resize, driven from the grip the library already draws.
   *
   * The sheet covers the conversation it came from, and the thing a reader
   * wants while looking at a file is usually the message that mentioned it. A
   * fixed height answers "open it bigger" and not "let me see behind it".
   *
   * Delegated from the window rather than bound to the element: the sheet is
   * the console library's and this workspace does not edit it, so there is
   * nothing of ours to put a handler on. A build of theirs that renames the
   * grip loses the drag and keeps the sheet, which is the right way for this to
   * fail.
   */
  function onGrip(event: PointerEvent): void {
    const target = event.target as HTMLElement | null;

    if (!target?.closest?.(".tc-fv__grip")) {
      return;
    }

    event.preventDefault();

    const drag = (moved: PointerEvent) =>
      sizeSheet(((window.innerHeight - moved.clientY) / window.innerHeight) * 100);

    const done = () => {
      window.removeEventListener("pointermove", drag);
      window.removeEventListener("pointerup", done);
      window.removeEventListener("pointercancel", done);
    };

    window.addEventListener("pointermove", drag);
    window.addEventListener("pointerup", done);
    window.addEventListener("pointercancel", done);
  }

  onMount(() => {
    ticking = setInterval(() => (now = Date.now()), 1000);

    sizeSheet(SHEET_OPEN);
    window.addEventListener("pointerdown", onGrip);

    void begin();
    void downloads.attach();
    void tree.open();
    void terminals.loadControls();
  });

  onDestroy(() => {
    if (ticking) {
      clearInterval(ticking);
    }

    window.removeEventListener("pointerdown", onGrip);
    stopProgress?.();
    // Only the listener. The transfers themselves are not this screen's to
    // stop: leaving a conversation is not a decision to abandon a file that is
    // halfway to this phone.
    downloads.cleanup();
    void manager.close();
    tree.close();
  });

  async function begin(): Promise<void> {
    await manager.open();
    await settle();
  }

  /**
   * Puts the newest turn in view.
   *
   * The box is scrolled directly rather than by `scrollIntoView`, which walks up
   * to the nearest scrollable ancestor: it moved the whole screen instead, took
   * the header with it, and left a blank page whenever the keyboard opened.
   *
   * After a tick, not before: the turn that arrived is not in the DOM until
   * Svelte has flushed, and scrolling first lands on the previous last turn.
   */
  async function settle(): Promise<void> {
    await tick();

    if (doc) {
      doc.scrollTop = doc.scrollHeight;
    }
  }

  // Whether an arriving turn scrolls the view. `Scroll.SLACK` is where the
  // threshold and the reason for it live.
  let following = $state(true);

  /**
   * Follows the tail, and walks back into history when the reader nears the top.
   *
   * The guard against a burst is `anchoring`, which `earlier` sets before it
   * awaits anything: a scroll gesture fires this many times over, and each one
   * would otherwise ask for the same page.
   *
   * Restoring the anchor fires this again, which is deliberate. A reader still
   * within reach of the top after a page landed wants the page before that one,
   * and the walk stops on its own when the machine says there is no more.
   */
  function onScroll(event: Event): void {
    const box = event.currentTarget as HTMLElement;

    following = Scroll.following(box.scrollTop, box.scrollHeight, box.clientHeight);

    if (Scroll.atTop(box.scrollTop) && $hasEarlier && !anchoring) {
      void earlier();
    }
  }

  // Not `$state`: the tail-follow effect reads it, and a reactive flag would
  // make that effect re-run on the way back out and scroll the reader away
  // after all.
  let anchoring = false;

  /**
   * Loads older turns without moving what the reader is looking at.
   *
   * Two things go wrong if this is left to the effect below. Prepending grows
   * `scrollHeight`, so a box that keeps its `scrollTop` is suddenly showing a
   * different part of the conversation. And the effect keys on the turn
   * *count*, which prepending changes too — so asking for history scrolled to
   * the end of it, and the way back to what you were reading was to scroll
   * down and then up again.
   *
   * The distance from the bottom is what is held fixed, because that is what
   * the reader is anchored to: everything added went on above them.
   */
  async function earlier(): Promise<void> {
    if (!doc) {
      await manager.older();

      return;
    }

    anchoring = true;

    const fromBottom = doc.scrollHeight - doc.scrollTop;

    try {
      await manager.older();
      await tick();

      doc.scrollTop = doc.scrollHeight - fromBottom;
    } finally {
      anchoring = false;
    }
  }

  // Depends on the count, so it runs when a turn arrives rather than whenever an
  // unrelated store changes.
  const arrived = $derived($turns.length);

  $effect(() => {
    if (arrived > 0 && following && !anchoring) {
      void settle();
    }
  });

  const title = $derived($conversation?.title ?? "Conversation");
  const subtitle = $derived(
    $conversation
      ? [$conversation.profile_label, $conversation.cwd].filter(Boolean).join(" · ")
      : "",
  );

  /**
   * The question on screen, from the transcript or from the tail.
   *
   * Both carry the same `Question`, and the transcript copy already has a place
   * on the timeline. This is for the header badge, which needs to know only
   * whether one exists.
   */
  const waitingOn = $derived<Question | null>($blocked);

  /**
   * Whether the question the agent is blocked on is already on screen.
   *
   * A question reaches this screen two ways and they do not arrive together. A
   * `Blocked` event carries the whole set the moment the agent stops; the
   * matching `Part::Question` appears only once the harness has written its
   * record, which can be later and, for a prompt read off the screen, never.
   * Without this the header says "waiting on you" over a transcript with
   * nothing to tap.
   */
  /**
   * Whether the agent is mid-turn.
   *
   * `stalled` is deliberately not included. It means a tool call is in flight
   * and has not grown, which is the one case where a spinner would be a lie —
   * the header's own glyph says stalled, and a second control claiming motion
   * would contradict it.
   *
   * Live figures also count as working, and not as a convenience. They arrive
   * on the watch the moment a turn begins, while `status` reaches this screen
   * only when a separate `conversation_changed` event does — so gating on
   * status alone hides the row for exactly as long as the two disagree.
   *
   * Never while a question is open. An agent waiting on a person is not
   * thinking, and a spinner over a card that is asking for an answer says the
   * opposite of what is true - that something is happening and the person can
   * wait. The card already carries its own elapsed time.
   *
   * But only while they are *fresh*. A finished turn sends no final figures, so
   * silence is the only signal it ended; without the staleness check the last
   * set of a completed turn keeps a spinner on screen indefinitely, saying work
   * is happening when none is. `now` ticks, so this re-evaluates on its own.
   */
  /**
   * A ticking clock.
   *
   * Every elapsed figure on this screen is drawn from it - how long the agent
   * has been working, how long a question has waited, whether the last figures
   * are stale. It was a plain `Date.now()` const, which is fixed at mount: the
   * durations only moved when something else happened to redraw, and the
   * staleness check could never fire at all.
   */
  let now = $state(Date.now());
  let ticking: ReturnType<typeof setInterval> | null = null;

  /**
   * Which tool calls and diffs are open, keyed by turn and position.
   *
   * Held here rather than inside the part, because a part is redrawn whenever
   * its turn is merged again from a page or the live tail - and state inside it
   * would close every fold the person had opened.
   */
  let opened = $state<Record<string, boolean>>({});

  function toggle(key: string): void {
    opened = { ...opened, [key]: !opened[key] };
  }

  const working = $derived(
    waitingOn === null &&
      ($conversation?.status === "working" || ($stats !== null && manager.freshStats(now))),
  );

  /**
   * The timeline, with each run of tool calls folded into one row.
   *
   * `working` rather than the raw status: it already accounts for a screen
   * whose figures are still moving, and it is false while an agent waits on an
   * answer. Only a run the agent is still adding to keeps its newest step
   * drawn, so the fold closes when the turn ends rather than leaving a finished
   * call sitting under it.
   */
  const rows = $derived(Activity.rows($turns, working));

  const questionOnScreen = $derived(
    waitingOn !== null &&
      $turns.some((turn) =>
        turn.parts.some(
          (part) =>
            "question" in part &&
            (part.question.question.id as unknown as string) ===
              (waitingOn.id as unknown as string) &&
            !Parts.wholeAnswered(part.question),
        ),
      ),
  );

  // Set when a send is refused because nothing is listening. The composer keeps
  // the text: a message somebody typed must survive being told the agent has
  // stopped, so resuming and pressing send again costs nothing.
  let stopped = $state(false);

  /**
   * What was just sent, until the machine echoes it back as a turn.
   *
   * The round trip is a send, then a record written by the harness, then a
   * watch event - hundreds of milliseconds at best. Waiting for that before
   * showing anything makes a phone feel like it dropped the message, which is
   * the moment people press send twice.
   */
  let pending = $state<{ text: string; files: string[]; at: number } | null>(null);

  /** Cleared once the real turn carrying the same words has arrived. */
  const echoed = $derived.by(() => {
    const held = pending;

    if (held === null) {
      return false;
    }

    // Matched on words and time rather than on an id, because the machine
    // mints the turn and this end never sees that id until the turn itself
    // arrives. The window is generous: a clock that disagrees by a second
    // must not leave a duplicate on screen for ever.
    return $turns.some(
      (turn) =>
        turn.role === "operator" &&
        Number(turn.at) >= held.at - 2000 &&
        turn.parts.some(
          (part) => "text" in part && part.text.text.trim() === held.text.trim(),
        ),
    );
  });

  $effect(() => {
    if (echoed) {
      pending = null;
    }
  });

  async function send(text: string): Promise<void> {
    if (sending) {
      return;
    }

    sending = true;
    stopped = false;

    // Cleared before the await, not after. The box collapses and the words
    // appear the moment the button is pressed, because that is when the person
    // let go of them - and a composer still holding what you just sent reads as
    // a send that did not happen.
    const files = staged;
    draft = "";
    staged = [];
    pending = { text, files: files.map((held) => held.name), at: Date.now() };
    following = true;
    await settle();

    const outcome = await manager.send(
      text,
      files.map((held) => held.asset as unknown as string),
    );

    sending = false;

    // Put back exactly what was taken away. A message that could not be sent
    // must not be lost to an optimistic clear, and the attachments have to
    // return with it or a resend would go without them.
    if (outcome !== "sent") {
      pending = null;
      draft = text;
      staged = files;
    }

    if (outcome === "not_running") {
      stopped = true;

      return;
    }

    if (outcome === "sent") {
      await settle();
    }
  }

  /**
   * The terminal side of the same workspace.
   *
   * The conversation travels with it: the terminal is a side of *this* session's
   * workspace rather than a place of its own, so it has to be able to name what
   * it belongs to and offer the way back.
   */
  function toTerminal(): void {
    const workspace = $conversation?.workspace;

    if (!workspace) {
      void goto(`/server?id=${encodeURIComponent(server)}`);

      return;
    }

    void goto(
      `/terminal?server=${encodeURIComponent(server)}` +
        `&workspace=${encodeURIComponent(workspace as unknown as string)}` +
        `&conversation=${encodeURIComponent(id)}`,
    );
  }

  /**
   * Pulls the desk to the workspace and tab this session is running in.
   *
   * One call, and it is the one the terminal screen already makes on a tab tap:
   * `tab focus` moves the focused *workspace* as well when the tab lives in
   * another one, so there is nothing extra to ask for.
   *
   * The tab rather than the pane, because that is the finest grain herdr
   * offers — `pane focus` takes a direction and not an id. A tab holding three
   * agents is focused as a tab, and the cursor does not land in C2.
   */
  async function pullDesk(): Promise<void> {
    const tab = mine?.tab_id as unknown as string | undefined;

    if (!tab) {
      return;
    }

    try {
      await invoke("focus_tab", { server, tab });
    } catch (error) {
      console.error("the desk did not move to this session", error);
    }
  }

  /** Switches this screen to another agent in the same tab. */
  function toChat(conversation: string): void {
    mapping = false;

    void goto(
      `/conversation?server=${encodeURIComponent(server)}` +
        `&id=${encodeURIComponent(conversation)}`,
    );
  }

  async function split(pane: string, direction: unknown): Promise<void> {
    try {
      await invoke("split_pane", { server, pane, direction });
    } catch (error) {
      console.error("could not split this pane", error);
    }
  }

  async function closePane(pane: string): Promise<void> {
    try {
      await invoke("close_pane", { server, pane });
    } catch (error) {
      console.error("could not close this pane", error);
    }
  }

  async function resume(): Promise<void> {
    if (await manager.resume()) {
      stopped = false;
    }
  }

  /**
   * Sends a whole set of answers.
   *
   * The array arrives one entry per ask, holes and all, because a shorter one
   * would shift every later answer onto the wrong question. A hole means the
   * person is not finished, and nothing goes to the machine until there are
   * none: the harness stays blocked until it has every answer, so a partial
   * reply would move its picker and leave it waiting on the rest.
   */
  async function answer(
    question: Question,
    answers: Array<Answer | null>,
    fingerprint: string,
  ): Promise<boolean> {
    // The set being answered is passed in rather than read from the watch. They
    // are usually the same set, and when they are not it is because the watch
    // has nothing - which is exactly the case a person still needs to answer.
    //
    // Every refusal below says why. A guard that returned quietly made Send a
    // control that sometimes did nothing, and a person cannot tell that from a
    // send that worked: the sheet closes either way and the answer is gone.
    //
    // The fingerprint is carried through rather than compared here. A card from
    // the transcript and a live `Blocked` event can describe one question at two
    // moments, and comparing would drop a perfectly good answer. The machine
    // refuses a stale one and says so.
    const whole: Answer[] = [];

    for (const held of answers) {
      if (held === null) {
        manager.refuse("One of these still needs an answer before the set can go.");

        return false;
      }

      whole.push(held);
    }

    if (whole.length !== question.asks.length) {
      manager.refuse(
        `This set has ${question.asks.length} question${question.asks.length === 1 ? "" : "s"} ` +
          `and ${whole.length} came back. Nothing was sent.`,
      );

      return false;
    }

    return await manager.answer(question, whole, fingerprint);
  }

  // Declared rather than omitted, so the capability gate below is the only thing
  // deciding whether these controls appear. Neither machine capability is
  // advertised yet; when one is, the body lands here and nothing else moves.
  /**
   * The files staged for the next message.
   *
   * Armed rather than delivered: each is on the machine and none has reached
   * the agent. Sending is what commits them, so removing a chip needs to tell
   * nobody.
   */
  let staged = $state<Attached[]>([]);
  let attaching = $state(false);

  /**
   * The file currently going up, and how far.
   *
   * A chip appears the moment bytes start moving rather than when they land.
   * The whole transfer is awaited inside one command, so without this a person
   * taps the paperclip, picks a file, and watches nothing happen for as long as
   * the link takes — which on a phone reads as an app that has stopped.
   */
  let rising = $state<{ name: string; fraction: number } | null>(null);
  let stopProgress: (() => void) | null = null;

  async function attach(): Promise<void> {
    if (attaching) {
      return;
    }

    attaching = true;
    rising = null;

    stopProgress = await listen<{ name: string; sent: number; total: number }>(
      "upload-progress",
      (event) => {
        const { name, sent, total } = event.payload;

        // A total of zero is the opening announcement, before the size is even
        // known. It draws the chip with no fraction rather than a full bar,
        // which is what dividing by nothing would have produced.
        rising = { name, fraction: total > 0 ? Math.min(1, sent / total) : 0 };
      },
    );

    const landed = await manager.stageFile();

    stopProgress?.();
    stopProgress = null;
    rising = null;
    attaching = false;

    if (landed) {
      staged = [...staged, landed];
    }
  }

  function unstage(id: string): void {
    // A transfer in flight is not removable. Its chip is a report, not a
    // control, and the command that owns it cannot be told to stop half way.
    if (id === "rising") {
      return;
    }

    staged = staged.filter((held) => (held.asset as unknown as string) !== id);
  }

  /**
   * The set being worked through in the stepper.
   *
   * A set of more than one ask is never answered in place: the harness stays
   * blocked until it has every answer, so a tap that sent the first choice alone
   * would move its picker and leave it waiting on the rest. The card hands those
   * here instead.
   */
  let stepping = $state<Question | null>(null);

  /**
   * The file open in the viewer.
   *
   * Tapping a card opens it rather than saving it. Saving is still one tap from
   * here, but reading is the common act and it should not require a trip
   * through the system file manager to do.
   */
  let viewing = $state<{
    asset: string;
    name: string;
    mime: string | null;
    size: number | null;
    preview: AssetPreview | null;
    refused: string | null;
  } | null>(null);

  /**
   * Whether a file should be drawn as a page rather than as its source.
   *
   * `previewKind` folds `text/html` in with plain text, which is the right
   * answer for a library that will not execute what it is given. This app
   * renders it in a sandboxed frame instead, so it needs to recognise the kind
   * the library does not name.
   *
   * The served type decides, and the extension is only a fallback for the
   * machines that answer `application/octet-stream` for everything.
   */
  function isHtml(name: string, mime: string | null): boolean {
    if (mime) {
      return mime.toLowerCase().includes("html");
    }

    return /\.x?html?$/i.test(name);
  }

  async function open(asset: string, name: string, mime: string | null, size: number | null) {
    const kind = previewKind(name, mime);

    viewing = { asset, name, mime, size, preview: null, refused: null };

    // Nothing is fetched for a kind that cannot render. Starting a transfer to
    // discover that costs a phone bytes for no reason.
    if (kind === "none") {
      viewing = { ...viewing, refused: "This one has nothing to show. Save it to open elsewhere." };

      return;
    }

    const head = await manager.preview(asset, TranscriptManager.TEXT_PREVIEW_BYTES, mime);

    // The viewer may have been closed while the bytes were arriving, and a
    // different file may have been opened since.
    if (viewing?.asset !== asset) {
      return;
    }

    viewing = head
      ? { ...viewing, preview: head }
      : { ...viewing, refused: "That file could not be read." };
  }

  // Answers as soon as the file has somewhere to go, not when the bytes land.
  // Waiting for the whole transfer here is what left a person with no progress,
  // no way to stop it, and no way to tell a file still arriving from one that
  // had arrived.
  async function download(asset: string, name: string): Promise<void> {
    await downloads.start(server, asset, name);
  }
</script>

<div class="screen">
  <!--
    No stop here. There were three on this screen and two of them were
    redundant; the one that survives sits on the row that says the agent is
    working, because that is the thing somebody is trying to stop. A stop in the
    title bar is always present and therefore says nothing about whether there
    is anything to interrupt.
  -->
  <NavBar {title} {subtitle} onback={() => goto(`/server?id=${encodeURIComponent(server)}`)} />

  <div class="head">
    {#if $conversation}
      <ConversationGlyph state={Conversations.glyph($conversation, $live || $loading)} />
      <span class="what">
        {#if waitingOn}
          waiting on you
        {:else if $conversation.binding}
          running
        {:else}
          nothing is running behind this
        {/if}
      </span>
    {/if}
    {#if !$live && !$loading}
      <span class="what dim">not following</span>
    {/if}
  </div>

  <div class="doc" bind:this={doc} onscroll={onScroll}>
    {#if $hasEarlier}
      <div class="earlier">
        <Button variant="quiet" disabled={$loading} onclick={() => void earlier()}>
          {$loading ? "Loading…" : "Earlier"}
        </Button>
      </div>
    {/if}

    {#if $loading && $turns.length === 0}
      <p class="note">reading the transcript…</p>
    {:else if $turns.length === 0}
      <p class="note">
        Nothing has been said yet. {$conversation?.binding
          ? "The agent is running; its first words will appear here."
          : "This conversation has no records."}
      </p>
    {/if}

    {#snippet step(turn: TurnRecord)}
      <Turn
        role={turn.role}
        time={Parts.clock(Number(turn.at))}
        at={Number(turn.at)}
        marked={Parts.pendingQuestion(turn)}
      >
        {#each turn.parts as part, at (at)}
          {@const key = `${turn.id}:${at}`}
          <!--
            Two different facts, and conflating them made the one thing a
            person had to do impossible.

            `live` is the watch's word: this is the set the agent is blocked
            on right now. It drives the elapsed time, because only a live set
            is still waiting on anybody.

            `openable` is the record's: nothing has answered this yet and an
            agent is running behind it. It drives whether the card can be
            opened, because the watch is not the only way a question reaches
            this screen and it is not always the first. A `Blocked` event that
            never arrived, or one withdrawn while the machine could not read
            its own pane, left an unanswered question drawn on screen with
            nothing to tap and no way to say why.

            A card that outlives its question is the cost, and it is the right
            way round: the machine refuses an answer to a set it is no longer
            waiting on, and says so. Silence teaches somebody the app is
            broken; a refusal teaches them what happened.
          -->
          {@const asking = "question" in part ? part.question : null}
          {@const live =
            waitingOn !== null &&
            asking !== null &&
            (asking.question.id as unknown as string) === (waitingOn.id as unknown as string)}
          {@const openable =
            asking !== null && asking.answered === null && Boolean($conversation?.binding)}
          <PartView
            {part}
            expanded={opened[key] ?? false}
            ontoggle={() => toggle(key)}
            ontool={() => toggle(key)}
            waiting={live ? Parts.waited(Number(turn.at), now) : null}
            onexpandquestion={$controls.answer && openable && asking
              ? () => (stepping = waitingOn ?? asking.question)
              : null}
            onopenfile={$controls.read_files
              ? (asset, name) =>
                  open(
                    asset as unknown as string,
                    name,
                    "file" in part ? part.file.mime : null,
                    "file" in part ? part.file.size : null,
                  )
              : undefined}
          />
        {/each}
      </Turn>
    {/snippet}

    <Timeline label="Transcript">
      {#each rows as row, index (row.key)}
        {@const first = Activity.leading(row)}
        {#if Parts.newDay(index > 0 ? Activity.trailing(rows[index - 1]) : undefined, first)}
          <Label flush>{Parts.day(Number(first.at))}</Label>
        {/if}
        {#if row.kind === "turn"}
          {@render step(row.turn)}
        {:else}
          <!--
            The fold carries the run's own timestamp, so the timeline still
            reads as a clock. What it opens is drawn under it rather than
            inside it: a step is a turn, and a turn nested in a turn loses the
            node and the time that make it one.
          -->
          <Turn role="agent" time={Parts.clock(Number(first.at))} at={Number(first.at)}>
            <ToolFold
              name={Activity.label(row.run)}
              detail={Activity.detail(row.run)}
              status={Activity.status(row.run)}
              expanded={opened[row.key] ?? false}
              onclick={() => toggle(row.key)}
            />
          </Turn>
          {#each Activity.shown(row.run, opened[row.key] ?? false) as folded (folded.id)}
            {@render step(folded)}
          {/each}
        {/if}
      {/each}
    </Timeline>

    <!--
      What was just sent, before the machine has echoed it back.

      Drawn as the person's own turn so the transcript does not jump when the
      real one lands - same alignment, same shape, dimmed until it is real.
    -->
    {#if pending}
      <div class="pending">
        <p class="said">{pending.text}</p>
        {#if pending.files.length > 0}
          <p class="files">{pending.files.join(", ")}</p>
        {/if}
      </div>
    {/if}

    <!--
      The agent is mid-turn.

      A spinner and a clock, and deliberately no figures. The Console's
      `ThinkingRow` wants tokens in and out and a tool count, and none of the
      three is on the wire yet — filling them with zeros would put three numbers
      on screen that are wrong rather than absent. Motion and elapsed time are
      what say "this is alive"; the numbers say how much it is costing, and that
      can arrive later without this row moving.
    -->
    {#if working}
      {#if $stats}
        <ThinkingRow
          stats={{
            elapsedSeconds: Math.max(0, Math.round((now - Number($stats.turn_started_at)) / 1000)),
            tokensIn: Number($stats.tokens_in),
            tokensOut: Number($stats.tokens_out),
            tools: Number($stats.tools),
            contextUsed: Number($stats.context_used),
            contextWindow: $stats.context_window === null ? null : Number($stats.context_window),
            model: $stats.model,
            // Absent rather than zero. The machine carries no price, and a
            // figure somebody acts on has to be right.
            costUsd: null,
          }}
          activity={$stats.activity}
          onstop={$controls.interrupt ? () => manager.interrupt() : null}
        />
      {:else}
        <!--
          The figures have not arrived yet, which is the first moment of every
          turn. Motion and a clock rather than zeros: a token count of nought on
          a working agent is a wrong number, not a missing one.
        -->
        <div class="thinking">
          <BrailleSpinner />
          <span class="verb">Working</span>
          <span class="since">{Parts.waited(Number($conversation?.last_active ?? now), now)}</span>

          <!--
            The same stop `ThinkingRow` carries once the figures arrive. Without
            it there is a window at the start of every turn - the whole of it,
            for an agent that never reports stats - where an agent is visibly
            working and nothing on screen will stop it.
          -->
          {#if $controls.interrupt}
            <button class="halt" type="button" onclick={() => manager.interrupt()}>
              esc to stop
            </button>
          {/if}
        </div>
      {/if}
    {/if}

    <!--
      The live question, drawn only when the transcript does not already carry
      it. A duplicate would be two announcements of one question. Neither route
      answers: both open the same sheet, which is the only place an answer is
      composed and sent.
    -->
    {#if waitingOn && !questionOnScreen}
      <div class="asked">
        <QuestionCard
          question={waitingOn}
          waiting={Parts.waited(Number($conversation?.last_active ?? now), now)}
          onopen={$controls.answer ? () => (stepping = waitingOn) : null}
        />
      </div>
    {/if}
  </div>

  {#if viewing}
    <FileViewer
      file={{
        name: viewing.name,
        size: viewing.size,
        mime: viewing.mime,
        preview: previewKind(viewing.name, viewing.mime),
      }}
      anchor="sheet"
      noPreviewReason={viewing.refused}
      onclose={() => (viewing = null)}
    >
      {#snippet actions()}
        <Button
          variant="quiet"
          onclick={() => {
            const held = viewing;
            viewing = null;

            if (held) {
              void download(held.asset, held.name);
            }
          }}
        >
          Save
        </Button>
      {/snippet}
      {#if !viewing.refused}
        {#if isHtml(viewing.name, viewing.preview?.mime ?? viewing.mime) && viewing.preview?.text}
          <HtmlPreview
            name={viewing.name}
            source={viewing.preview.text}
            truncated={viewing.preview.truncated}
          />
          <!--
            A markdown file gets the same table treatment as agent prose. The
            library renders both through the same parser, so a plan read from a
            file lost its tables exactly the way a message did.
          -->
        {:else if viewing.preview?.text && previewKind(viewing.name, viewing.mime) === "markdown" && MarkdownTables.has(viewing.preview.text)}
          <div class="mdfile">
            <MarkdownBody source={viewing.preview.text} />

            {#if viewing.preview.truncated}
              <span class="cut">first part only · save the file to read it all</span>
            {/if}
          </div>
        {:else}
          <FilePreview
            name={viewing.name}
            mime={viewing.preview?.mime ?? viewing.mime}
            text={viewing.preview?.text ?? null}
            imageUrl={viewing.preview?.image_data_url ?? null}
            truncated={viewing.preview?.truncated ?? false}
          />
        {/if}
      {/if}
    </FileViewer>
  {/if}

  {#if stepping}
    <!--
      Above the sheet, not behind it.

      The sheet is z-index 6 and the failure line at the foot of the screen is
      not, so every reason an answer was refused was printed underneath the one
      thing covering it. What that looks like from the outside is a Send button
      that does nothing - which is exactly what it looked like.
    -->
    {#if $failure}
      <p class="over" role="alert">{$failure}</p>
    {/if}

    <!--
      Every answer is reviewed before it is sent, including a single choice.

      The flow can submit a lone single-select on the tap that chooses it, which
      is what the harness at the other end does. On a phone it is wrong: a
      mis-hit option is sent before the thumb has left the glass, and an answer
      that has already reached the agent cannot be taken back. A tap that only
      selects costs one more tap and is recoverable.
    -->
    <QuestionFlow
      question={stepping}
      anchor="sheet"
      autoSubmit={false}
      waiting={Parts.waited(Number($conversation?.last_active ?? now), now)}
      onsubmit={(answers, fingerprint) => {
        // Closed only once the machine has taken it. Closing first threw the
        // answer away on every refusal, and left the person looking at the
        // question they had just answered with no way to tell what happened.
        // Captured before the sheet can close under it: the set being answered
        // is the one it was opened on, not whatever the watch holds by the time
        // the machine replies.
        const asked = stepping;

        if (asked) {
          void answer(asked, answers, fingerprint).then((sent) => {
            if (sent) {
              stepping = null;
            }
          });
        }
      }}
      oncancel={() => (stepping = null)}
    />
  {/if}

  <DownloadTray
    rows={$carrying}
    oncancel={(id) => downloads.cancel(id)}
    ondismiss={(id) => downloads.dismiss(id)}
  />

  {#if $failure}
    <p class="note warn">{$failure}</p>
  {/if}

  {#if $controls.send}
    <!--
      Drawn whether or not a pane is bound. The machine decides: a conversation
      whose agent has stopped answers `not_running`, which offers a resume and
      keeps the text, rather than hiding the box and losing what was typed.

      Never while the agent is demonstrably alive, though. `binding` lags - it is
      absent for the first moments of a started conversation and again while one
      is blocked on a question - and offering to resume a session that is
      answering, or that is waiting on the person, contradicts the row directly
      above it. A refusal from the machine still shows it, because that is the
      machine saying so rather than this screen guessing.
    -->
    {#if stopped || (!$conversation?.binding && $conversation && !working && waitingOn === null)}
      <div class="stopped">
        <p class="note">
          {stopped
            ? "That did not reach anybody — this conversation has stopped."
            : "Nothing is running behind this conversation."}
          {#if $controls.resume}
            Resuming starts an agent on the machine and continues this transcript rather than
            beginning a new one.
          {:else}
            This machine does not offer resuming yet.
          {/if}
        </p>
        {#if $controls.resume}
          <Button disabled={$resuming} onclick={resume}>
            {$resuming ? "Resuming…" : "Resume this session"}
          </Button>
        {/if}
      </div>
    {/if}

    <!--
      Over the transcript rather than beside it. Three agents in one herdr tab
      is a valid arrangement, and this is the only way from C1 to C2 without
      leaving the machine screen and coming back.
    -->
    {#if mapping}
      <div class="mapping">
        <div class="mapping-head">
          <span>Panes in this tab</span>
          <button class="halt" type="button" onclick={() => (mapping = false)}>close</button>
        </div>

        <LayoutSheet
          panes={tabPanes}
          layout={tabLayout}
          selected={selectedPane}
          controls={$terminalControls}
          onselect={(pane) => (selectedPane = pane)}
          onenter={toTerminal}
          onchat={toChat}
          onsplit={(pane, direction) => void split(pane, direction)}
          onclosepane={(pane) => void closePane(pane)}
        />
      </div>
    {/if}

    <!--
      One row above the composer, and the composer never moves. The mode key
      leads to the terminal; the floorplan key is how somebody reaches the other
      agents sharing this tab, and it is absent below two panes because a badge
      reading one says nothing.
    -->
    <ComposerRail
      mode="chat"
      onmode={toTerminal}
      onfocus={mine !== null && $terminalControls.focus_tab ? pullDesk : null}
      onmap={tabLayout === null || tabPanes.length < 2 ? null : () => (mapping = true)}
      mapBadge={Floorplan.agents(tabPanes)}
    />

    <Composer
      value={draft}
      placeholder={waitingOn && $controls.answer
        ? "reply, or tap an answer above"
        : "message this agent"}
      disabled={sending}
      attachments={[
        ...staged.map((held) => ({
          id: held.asset as unknown as string,
          name: held.name,
          progress: null,
        })),
        // The one in flight, drawn with its fraction. It has no id yet — the
        // machine answers that when the last byte lands — so it carries a
        // placeholder and no remove control.
        ...(rising ? [{ id: "rising", name: rising.name, progress: rising.fraction }] : []),
      ]}
      onattach={$controls.attach_files ? attach : null}
      onremoveattachment={unstage}
      oninput={(value) => (draft = value)}
      onsend={send}
    />
  {:else}
    <!-- Capability-gated rather than a box that cannot send. A composer that
         silently does nothing is worse than one that is not there with a line
         saying why. -->
    <p class="note foot">
      {$conversation?.profile_label ?? "This machine"} cannot take messages yet — it does not
      advertise <code>prompt_send</code>. Reading and history work.
    </p>
  {/if}
</div>

<style lang="scss">
  // A file preview must have one scroller, not two nested inside each other.
  //
  // The console library gives `.tc-fp__code` both `flex: 1` and
  // `overflow-y: auto`, which makes the code its own scrolling box inside the
  // viewer's. On a phone a drag that starts over the code moves the code and
  // never the sheet, so a reader who lands on a long file cannot get out of it
  // by scrolling — which is what a code block appearing to trap the page is.
  //
  // Letting the code take its natural height and moving the scroll up to the
  // container leaves exactly one thing that scrolls.
  //
  // Overridden from here rather than corrected in place: `client/src/console/`
  // is another team's library and this workspace does not edit it. The real fix
  // belongs upstream in `FilePreview.scss`.
  // The file sheet opens at the height it is allowed, not the height its
  // contents happen to want.
  //
  // The library gives the sheet a `max-height` and no `height`, so it is sized
  // by its contents: a preview that does not stretch leaves the sheet halfway
  // up the screen with the transcript showing under it, and a long document
  // then reads through a letterbox. Taking their own maximum as the height
  // opens it the same way every time, whatever is inside.
  //
  // Overridden from here rather than corrected in place: `client/src/console/`
  // is another team's library and this workspace does not edit it.
  :global(.tc-fv.is-sheet) {
    height: var(--tethera-sheet, 86%);
  }

  // The grip is 30x4 - a thumb cannot land on that. The pad is invisible and
  // only widens what counts as a hit; `touch-action` stops the drag being
  // stolen by the scroll underneath it.
  :global(.tc-fv__grip) {
    touch-action: none;
    cursor: ns-resize;
  }

  :global(.tc-fv__grip)::after {
    content: "";
    position: absolute;
    inset: -16px -60px;
  }

  :global(.tc-fp) {
    overflow-y: auto;
  }

  .mdfile {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .cut {
    flex: none;
    font-size: 11px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    opacity: 0.7;
  }

  :global(.tc-fp__code) {
    flex: none;
    overflow-y: visible;
  }

  .over {
    position: fixed;
    z-index: 7;
    top: 12px;
    left: 12px;
    right: 12px;
    margin: 0;
    padding: 10px 12px;
    border-radius: 8px;
    background: var(--tc-surface);
    border: 1px solid var(--tc-attn);
    color: var(--tc-ink-1);
    font-size: 13px;
    line-height: 1.4;
  }

  // A transcript is not a document that scrolls to its end. The composer and the
  // header stay put and the turns move between them, so somebody reading history
  // can still type, and pressing send does not mean scrolling back down first.
  .screen {
    display: flex;
    flex-direction: column;
    height: 100dvh;
    overflow: hidden;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 18px 4px;
  }

  .what {
    font-family: var(--tc-mono);
    font-size: 9.5px;
    letter-spacing: 0.13em;
    text-transform: uppercase;
    color: var(--tc-ink-3);
  }

  .dim {
    margin-left: auto;
    opacity: 0.7;
  }

  // Dimmed rather than a spinner. It is the person's own words, already on
  // screen; what is uncertain is only whether the machine has them yet.
  .pending {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
    margin: 4px 0 10px;
    opacity: 0.55;

    .said {
      margin: 0;
      max-width: 100%;
      padding: 10px 13px;
      border-radius: 14px 14px 4px 14px;
      background: var(--tc-surface-2);
      box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--tc-accent) 30%, transparent);
      font-size: 13.5px;
      line-height: 1.55;
      white-space: pre-wrap;
      overflow-wrap: anywhere;
    }

    .files {
      margin: 0;
      font-family: var(--tc-mono);
      font-size: 10px;
      color: var(--tc-ink-3);
    }
  }

  .thinking {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 10px 0 2px;
    font-size: 12.5px;
    color: var(--tc-ink-3);

    .verb {
      color: var(--tc-ink-2);
    }

    .since {
      font-family: var(--tc-mono);
      font-size: 11px;
    }
  }


  // Held clear of the composer so the last option is never under the send
  // button, which on a phone is where a thumb already is.
  .asked {
    margin: 14px 0 4px;
  }

  .doc {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 6px 12px 12px;

    // The Console sizes its gutter for a desk. On a phone every pixel it takes
    // is one the reading does not get, and "01:15 PM" needs less than the
    // default.
    --tc-gutter: 40px;
    --tc-gutter-gap: 8px;
  }

  // Your turns leave the timeline and sit on the right, in a filled block.
  //
  // The Console's default puts both speakers on one line, distinguished only by
  // a filled node against a hollow one. That reads at desk width and does not on
  // a phone: two columns of the same grey, and finding your own last message
  // means hunting for a 7-pixel dot. Side plus fill is the split the eye makes
  // without being asked.
  //
  // Overridden here rather than in the component, which belongs to another team
  // and is right for the wide layouts it was drawn for.
  .doc :global(.tc-turn.is-you) {
    // Off the line entirely. A node with no thread through it is a smudge.
    &::before,
    &::after {
      display: none;
    }
  }

  // The gutter stays where it is on every other turn. Moving the clock as well
  // would leave the two speakers sharing no common edge, and the time is what
  // lets you scan a conversation for when something happened.
  .doc :global(.tc-turn.is-you .tc-turn__caret) {
    display: none;
  }

  // A pasted path has no spaces in it, so without this the bubble grows past
  // the screen edge and takes the whole turn with it.
  .doc :global(.tc-turn__body),
  .doc :global(.tc-turn__body p) {
    overflow-wrap: anywhere;
    word-break: break-word;
    min-width: 0;
  }

  .doc :global(.tc-turn.is-you .tc-turn__body) {
    justify-self: end;
    max-width: 100%;
    margin-bottom: 18px;
    padding: 10px 13px;
    border-radius: 14px 14px 4px 14px;
    background: var(--tc-surface-2);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--tc-accent) 30%, transparent);
  }

  .earlier {
    display: flex;
    justify-content: center;
    padding-bottom: 10px;
  }

  // Over the transcript, above the rail, and bounded so a tab with many panes
  // scrolls its own map rather than pushing the composer off the screen.
  .mapping {
    display: flex;
    flex-direction: column;
    flex: none;
    max-height: 58dvh;
    overflow: hidden;
    border-top: 1px solid var(--tc-rule);
    background: var(--tc-bg);
  }

  .mapping-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px 0;
    font-size: 12px;
    color: var(--tc-ink-2);
  }

  .halt {
    font-family: var(--tc-mono);
    font-size: 9.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 5px 9px;
    border-radius: 5px;
    border: 1px solid var(--tc-rule-2);
    background: none;
    color: var(--tc-ink-2);
  }

  .note {
    margin: 0;
    padding: 14px 18px;
    font-size: 13px;
    line-height: 1.5;
    color: var(--tc-ink-3);

    code {
      font-family: var(--tc-mono);
      font-size: 11px;
    }
  }

  .warn {
    color: var(--tc-ink-2);
    padding-top: 0;
  }

  .foot {
    border-top: 1px solid var(--tc-rule);
  }

  .stopped {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 0 18px 4px;
    border-top: 1px solid var(--tc-rule);

    .note {
      padding: 12px 0 0;
    }
  }
</style>
