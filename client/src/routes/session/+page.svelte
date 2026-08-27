<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { invoke } from "@tauri-apps/api/core";
  import { Button, Chip, ConnDot, Label, NavBar, Tree, TreeNode } from "$console";
  import { ServerManager } from "$managers/server_manager";
  import { SessionManager } from "$managers/session_manager";
  import type { AgentProfile } from "$bindings/AgentProfile";
  import type { ServerRow } from "$bindings/ServerRow";

  const servers = new ServerManager(invoke);
  const rows = servers.rows;

  const session = new SessionManager(invoke);
  const draft = session.draft;
  const profiles = session.profiles;
  // Not `state`: `$state` is a rune, and a store of that name makes `$state("")`
  // resolve to the store's value called as a function. It compiles, and fails at
  // run time with "P(...) is not a function" from the minified bundle.
  const phase = session.state;
  const canStart = session.canStart;
  const loading = session.loading;
  const recent = session.recent;
  const preview = session.preview;

  let cwd = $state("");
  let prompt = $state("");

  onMount(() => {
    void start();
  });

  async function start(): Promise<void> {
    await servers.load();

    const asked = page.url.searchParams.get("server");
    const first = asked ?? firstReachable();

    if (first) {
      await session.chooseServer(first);
    }
  }

  function firstReachable(): string | null {
    const answering = $rows.find((row) => row.link.kind !== "offline") ?? $rows[0];

    return answering ? (answering.entry.server.id as unknown as string) : null;
  }

  const chosen = $derived(
    $rows.find(
      (row) => (row.entry.server.id as unknown as string) === $draft.serverId,
    ) ?? null,
  );

  function chooseServer(row: ServerRow): void {
    void session.chooseServer(row.entry.server.id as unknown as string);
  }

  function chooseProfile(profile: AgentProfile): void {
    session.chooseProfile(profile);
    askPreview();
  }

  // Asked when the typing stops rather than on every keystroke. Each preview is
  // a dial and a round trip, and the answer for a half-typed path is a path the
  // machine will not have.
  const PREVIEW_SETTLE = 400;
  let pending: ReturnType<typeof setTimeout> | null = null;

  function askPreview(): void {
    if (pending) {
      clearTimeout(pending);
    }

    pending = setTimeout(() => {
      session.setCwd(cwd);
      void session.refreshPreview();
    }, PREVIEW_SETTLE);
  }

  onDestroy(() => {
    if (pending) {
      clearTimeout(pending);
    }
  });

  async function launch(): Promise<void> {
    session.setCwd(cwd);
    session.setPrompt(prompt);
    await session.start();

    const phased = $phase;

    if (phased.step !== "started" || !("started" in phased.outcome)) {
      return;
    }

    // Straight to the session that was just created rather than back to the
    // list. Landing on the list would make somebody hunt for the thing they
    // just started, and the machine's answer already describes it.
    const made = phased.outcome.started;

    await goto(
      `/conversation?${new URLSearchParams({
        id: made.id as unknown as string,
        server: chosen?.entry.server.id as unknown as string,
      }).toString()}`,
    );
  }

  /**
   * Whether a start would produce something this app can follow.
   *
   * The machine refuses a harness whose records it cannot read, so this is the
   * same rule stated before the button rather than after it. The preview is
   * consulted second because the answer can depend on the directory: the same
   * harness is readable in one tree and not in another.
   */
  const readable = $derived(
    ($draft.profile?.provides_transcript ?? false) &&
      ($preview ? $preview.will_have_transcript : true),
  );

  const complete = $derived(
    SessionManager.isComplete({ ...$draft, cwd, prompt }) && $canStart && readable,
  );
</script>

<NavBar
  title="New session"
  subtitle="nothing starts until you press start"
  onback={() => goto("/")}
/>

<Tree label="New session">
  <TreeNode state={$draft.serverId ? "set" : "unset"} branches>
    <div class="klbl">server</div>
    {#if $rows.length === 0}
      <div class="note">No machines paired yet.</div>
    {:else}
      <div class="chips">
        {#each $rows as row (row.entry.server.id)}
          <Chip
            label={row.entry.server.label}
            selected={(row.entry.server.id as unknown as string) === $draft.serverId}
            onclick={() => chooseServer(row)}
          />
        {/each}
      </div>
    {/if}
    {#if chosen && chosen.link.kind !== "unknown"}
      <div class="conn"><ConnDot link={chosen.link.kind} rttMs={chosen.link.rtt_ms} /></div>
    {/if}
  </TreeNode>

  <TreeNode state={$draft.profile ? "set" : "unset"} branches spaced>
    <div class="klbl">harness</div>
    {#if $loading}
      <div class="note">asking the machine what it can run…</div>
    {:else if $profiles.length === 0}
      <div class="note">This machine lists no agents it can run.</div>
    {:else}
      <div class="chips">
        {#each $profiles as profile (profile.id)}
          <Chip
            label={profile.label}
            detail={profile.version}
            selected={profile.id === $draft.profile?.id}
            onclick={() => chooseProfile(profile)}
          />
        {/each}
      </div>
      <div class="klbl soft">only what this machine accepts is listed</div>
      {#if $draft.profile && !$draft.profile.provides_transcript}
        <!-- Not merely a warning: the machine refuses this, so the button stays
             off. The reason belongs beside the choice that caused it. -->
        <div class="note">
          This machine can open a pane for {$draft.profile.label} but cannot follow its
          conversation, so it will not start one.
        </div>
      {/if}
    {/if}
  </TreeNode>

  <TreeNode state={cwd.trim() ? "set" : "unset"} branches spaced>
    <div class="klbl">directory</div>
    <input
      class="field one"
      bind:value={cwd}
      oninput={askPreview}
      placeholder="/home/you/projects/thing"
      spellcheck="false"
    />
    {#if $recent.length > 0}
      <div class="chips">
        {#each $recent.slice(0, 4) as path (path)}
          <Chip
            label={path}
            selected={path === cwd}
            onclick={() => {
              cwd = path;
              askPreview();
            }}
          />
        {/each}
      </div>
      <div class="klbl soft">{$recent.length} used on this machine</div>
    {/if}
    <!-- An absolute path is the machine's rule, not a preference: a relative one
         would resolve against wherever the server happens to be running, so it
         is refused. Saying so here beats a refusal after the button. -->
    {#if cwd.trim() && !cwd.trim().startsWith("/") && !/^[A-Za-z]:[\\/]/.test(cwd.trim())}
      <div class="note">
        This must be a full path. A relative one would land somewhere nobody chose.
      </div>
    {/if}
  </TreeNode>

  {#if $preview}
    <TreeNode state="set" branches spaced>
      <div class="klbl">lands in</div>
      <p class="note tight">
        {$preview.creates_workspace ? "A new workspace" : "Workspace"}
        <b class="mono">{$preview.workspace_label}</b>, tab
        <b class="mono">{$preview.tab_label}</b>.
      </p>
      {#if !$preview.will_have_transcript}
        <p class="note">
          Started here, this harness leaves no readable transcript, so the session will not
          appear as a conversation.
        </p>
      {/if}
    </TreeNode>
  {/if}

  <TreeNode state={prompt.trim() ? "set" : "unset"} spaced>
    <div class="klbl">first message</div>
    <textarea class="field" bind:value={prompt} rows="4" placeholder="Optional. What should it start on?"
    ></textarea>
  </TreeNode>
</Tree>

<div class="foot">
  {#if !$canStart && $draft.serverId && !$loading}
    <!-- Capability-gated rather than hidden. A button that fails on press is
         worse than one that explains itself before it is pressed. -->
    <p class="note warn">
      {chosen?.entry.server.label ?? "This machine"} cannot start sessions yet — it does not
      advertise <code>conversation_start</code>. Pairing, listing and terminals still work.
    </p>
  {/if}

  {#if $phase.step === "started" && "awaiting_agent" in $phase.outcome}
    <!-- Not a failure. The pane is open and the harness is in it, held by
         something only somebody at the desk can answer — a directory it has not
         been trusted with, a sign-in, an onboarding screen. Nothing on the wire
         tells those apart, so the copy does not guess between them. -->
    <p class="note warn">
      It started, and it is waiting at the machine. {chosen?.entry.server.label ?? "The machine"} opened
      pane <b class="mono">{$phase.outcome.awaiting_agent.pane}</b> and the agent is asking for
      something there before it will begin. Answer it, then start again.
    </p>
  {:else if $phase.step === "failed"}
    <p class="note warn">{$phase.reason}</p>
  {/if}

  <Button disabled={!complete || $phase.step === "starting"} onclick={launch}>
    {$phase.step === "starting" ? "Starting…" : "Start session"}
  </Button>
</div>

<style lang="scss">
  .klbl {
    font-family: var(--tc-mono);
    font-size: 9px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--tc-ink-3);
    margin-bottom: 2px;
  }

  .soft {
    margin-top: 7px;
    letter-spacing: 0.12em;
  }

  .chips {
    display: flex;
    gap: 7px;
    flex-wrap: wrap;
    margin-top: 8px;
  }

  .conn {
    margin-top: 6px;
  }

  .field {
    width: 100%;
    margin-top: 9px;
    padding: 13px 15px;
    border-radius: 14px;
    border: 1px solid var(--tc-line, rgba(255, 255, 255, 0.14));
    background: var(--tc-surface);
    color: var(--tc-ink);
    font-family: inherit;
    font-size: 13.5px;
    line-height: 1.5;
    resize: vertical;
  }

  .one {
    font-family: var(--tc-mono);
    font-size: 13px;
  }

  .note {
    margin: 8px 0 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--tc-ink-3);

    code {
      font-family: var(--tc-mono);
      font-size: 11px;
    }
  }

  .warn {
    color: var(--tc-ink-2);
  }

  .tight {
    margin-top: 6px;

    b {
      color: var(--tc-ink-2);
      font-weight: 600;
    }
  }

  .mono {
    font-family: var(--tc-mono);
    font-size: 11.5px;
  }

  .foot {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 24px 18px 40px;
  }
</style>
