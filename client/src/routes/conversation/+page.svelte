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
    Turn,
  } from "$console";
  import { PREVIEW_BYTES, previewKind } from "$console";
  import { Parts } from "$managers/parts";
  import { TranscriptManager, type Listen } from "$managers/transcript_manager";
  import { DownloadManager, type ListenDownloads } from "$managers/download_manager";
  import DownloadTray from "$components/DownloadTray.svelte";
  import ConversationGlyph from "$components/ConversationGlyph.svelte";
  import { Conversations } from "$managers/conversations";
  import type { Answer } from "$bindings/Answer";
  import type { AssetPreview } from "$bindings/AssetPreview";
  import type { Attached } from "$bindings/Attached";
  import type { Question } from "$bindings/Question";

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

  let draft = $state("");
  let sending = $state(false);
  let doc: HTMLDivElement | null = $state(null);

  onMount(() => {
    ticking = setInterval(() => (now = Date.now()), 1000);

    void begin();
    void downloads.attach();
  });

  onDestroy(() => {
    if (ticking) {
      clearInterval(ticking);
    }

    stopProgress?.();
    // Only the listener. The transfers themselves are not this screen's to
    // stop: leaving a conversation is not a decision to abandon a file that is
    // halfway to this phone.
    downloads.cleanup();
    void manager.close();
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

  // Arriving turns follow the tail only when the tail is already what somebody
  // is looking at. Yanking the view down while they are reading history is the
  // single most annoying thing a transcript can do.
  let following = $state(true);

  function onScroll(event: Event): void {
    const box = event.currentTarget as HTMLElement;
    const slack = box.scrollHeight - box.scrollTop - box.clientHeight;

    following = slack < 80;
  }

  // Depends on the count, so it runs when a turn arrives rather than whenever an
  // unrelated store changes.
  const arrived = $derived($turns.length);

  $effect(() => {
    if (arrived > 0 && following) {
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

  async function open(asset: string, name: string, mime: string | null, size: number | null) {
    const kind = previewKind(name, mime);

    viewing = { asset, name, mime, size, preview: null, refused: null };

    // Nothing is fetched for a kind that cannot render. Starting a transfer to
    // discover that costs a phone bytes for no reason.
    if (kind === "none") {
      viewing = { ...viewing, refused: "This one has nothing to show. Save it to open elsewhere." };

      return;
    }

    const head = await manager.preview(asset, PREVIEW_BYTES, mime);

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
  <NavBar {title} {subtitle} onback={() => goto(`/server?id=${encodeURIComponent(server)}`)}>
    {#snippet actions()}
      {#if $controls.interrupt && $conversation?.binding}
        <button class="stop" type="button" onclick={() => manager.interrupt()}>stop</button>
      {/if}
    {/snippet}
  </NavBar>

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
        <Button variant="quiet" disabled={$loading} onclick={() => manager.older()}>
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

    <Timeline label="Transcript">
      {#each $turns as turn, index (turn.id)}
        {#if Parts.newDay($turns[index - 1], turn)}
          <Label flush>{Parts.day(Number(turn.at))}</Label>
        {/if}
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
        <FilePreview
          name={viewing.name}
          mime={viewing.preview?.mime ?? viewing.mime}
          text={viewing.preview?.text ?? null}
          imageUrl={viewing.preview?.image_data_url ?? null}
          truncated={viewing.preview?.truncated ?? false}
        />
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

  .stop {
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
