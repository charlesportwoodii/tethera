<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { Parts } from "$managers/parts";
  import { page } from "$app/state";
  import { invoke } from "@tauri-apps/api/core";
  import { Button, ConnDot, Label, NavBar, Tree, TreeNode, TreeTwig } from "$console";
  import { ServerManager } from "$managers/server_manager";
  import { Conversations } from "$managers/conversations";
  import ConversationGlyph from "$components/ConversationGlyph.svelte";
  import type { Conversation } from "$bindings/Conversation";
  import type { Page } from "$bindings/Page";
  import type { ServerRow } from "$bindings/ServerRow";

  const PER_PAGE = 25;

  const manager = new ServerManager(invoke);
  const rows = manager.rows;
  const sweeping = manager.sweeping;

  const id = page.url.searchParams.get("id") ?? "";

  let held = $state<Conversation[]>([]);
  let cursor = $state<string | null>(null);
  let hasEarlier = $state(false);
  let loading = $state(false);
  let listError = $state<string | null>(null);
  let unfollow: (() => void) | null = null;

  onMount(() => {
    void start();
  });

  onDestroy(() => {
    unfollow?.();
  });

  async function start(): Promise<void> {
    // Paint what is remembered, then measure. `load` reports every link as
    // Unknown because nothing has been dialled, and this page has its own
    // manager: whatever the list page measured does not reach it. Without the
    // sweep the header reads "finding a route" for the life of the screen.
    await manager.load();
    await Promise.all([manager.sweep(), more()]);

    unfollow = manager.follow(() => void refresh());
  }

  /**
   * The newest page again, merged over what is already held.
   *
   * Which sessions are attached and what each is doing changes while this
   * screen is open, and a list paged in once says what was true when it was
   * read. Merged by id rather than replaced, because replacing would throw
   * away every earlier page somebody scrolled to reach.
   */
  async function refresh(): Promise<void> {
    if (loading) {
      return;
    }

    try {
      const first = (await invoke("list_conversations", {
        id,
        before: null,
        limit: PER_PAGE,
      })) as Page<Conversation>;

      const fresh = new Map(
        first.items.map((item) => [item.id as unknown as string, item]),
      );

      const kept = held.map((item) => fresh.get(item.id as unknown as string) ?? item);
      const known = new Set(kept.map((item) => item.id as unknown as string));
      const added = first.items.filter((item) => !known.has(item.id as unknown as string));

      held = [...added, ...kept];
    } catch {
      // A failed refresh leaves the screen as it stands. Losing the newest
      // marks is not a reason to lose the list under them, and the next tick
      // is five seconds away.
    }
  }

  const row = $derived(
    $rows.find((candidate) => (candidate.entry.server.id as unknown as string) === id) ?? null,
  );

  // While the route is still being measured this reads reachable, which is what
  // the page already assumes elsewhere: it lists remembered work and says so.
  // The dots turn hollow when the sweep comes back with nothing.
  const reachable = $derived(row === null || row.link.kind !== "offline");

  const live = $derived(Conversations.live(held));
  const earlier = $derived(Conversations.byDay(Conversations.dormant(held)));

  /**
   * A fresh dial rather than the five the sweep carried back. Failure keeps
   * whatever has already been paged in: losing the next page is not a reason to
   * discard the ones already on screen.
   */
  async function more(): Promise<void> {
    if (loading) {
      return;
    }

    loading = true;
    listError = null;

    try {
      const next = (await invoke("list_conversations", {
        id,
        before: cursor,
        limit: PER_PAGE,
      })) as Page<Conversation>;

      held = [...held, ...next.items];
      cursor = (next.next_before as unknown as string | null) ?? null;
      hasEarlier = next.has_earlier;
    } catch (error) {
      listError = String(error);
    } finally {
      loading = false;
    }
  }

  function lastAnswered(candidate: ServerRow): string {
    if (!candidate.entry.last_seen_at) {
      return "never";
    }

    // Epoch milliseconds, like every other Timestamp on the wire.
    return new Date(Number(candidate.entry.last_seen_at)).toLocaleString();
  }

  /**
   * Opens whichever side this session actually has.
   *
   * A session with no readable transcript has nothing to render as chat, so it
   * opens at its terminal rather than at an empty screen that blames the
   * machine for it.
   */
  function read(conversation: Conversation): void {
    if (!conversation.has_transcript && conversation.workspace) {
      const held = encodeURIComponent(conversation.workspace as unknown as string);

      void goto(`/terminal?server=${encodeURIComponent(id)}&workspace=${held}`);

      return;
    }

    const held = encodeURIComponent(conversation.id as unknown as string);

    void goto(`/conversation?server=${encodeURIComponent(id)}&id=${held}`);
  }

  async function forget(): Promise<void> {
    await manager.forget(id);
    await goto("/");
  }
</script>

{#if !row}
  <NavBar title="Unknown machine" onback={() => goto("/")} />
  <div class="pane">
    <p class="note">This machine is not in the list any more.</p>
  </div>
{:else}
  <NavBar
    title={row.entry.server.label}
    subtitle={`${row.entry.server.os} · ${row.entry.server.arch}`}
    onback={() => goto("/")}
  />

  <div class="pane">
    {#if row.link.kind === "offline"}
      <ConnDot link="offline" />
      <p class="note">
        Nothing has answered on any known address since <b>{lastAnswered(row)}</b>. What follows is
        remembered, not live.
      </p>
      <Button disabled={$sweeping} onclick={() => manager.sweep()}>
        {$sweeping ? "Trying…" : "Try again"}
      </Button>
    {:else if row.refusal}
      <p class="note">
        {row.entry.server.label} answered and would not accept this device. If you did not revoke it,
        check that machine.
      </p>
      <Button disabled={$sweeping} onclick={() => manager.sweep()}>Try again</Button>
    {:else if row.link.kind === "unknown"}
      <p class="note">finding a route…</p>
    {:else}
      <ConnDot link={row.link.kind} rttMs={row.link.rtt_ms} />
    {/if}

    <!-- Above the sessions it will join. This is the page where "another one"
         has an unambiguous subject, which is why it is here and not on the list
         of servers. -->
    <Button icon="plus" onclick={() => goto(`/session?server=${encodeURIComponent(id)}`)}>
      New session on {row.entry.server.label}
    </Button>
  </div>

  {#if live.length > 0}
    <Label flush>running now</Label>
    <Tree label="Running now">
      {#each live as conversation (conversation.id)}
        <TreeNode spaced>
          {#snippet glyph()}
            <ConversationGlyph state={Conversations.glyph(conversation, reachable)} />
          {/snippet}
          {@render twig(conversation)}
        </TreeNode>
      {/each}
    </Tree>
  {/if}

  {#each earlier as group (group.label)}
    <Label flush>{group.label}</Label>
    <Tree label={group.label}>
      {#each group.items as conversation (conversation.id)}
        <TreeNode dim spaced>
          {#snippet glyph()}
            <ConversationGlyph state={Conversations.glyph(conversation, reachable)} />
          {/snippet}
          {@render twig(conversation)}
        </TreeNode>
      {/each}
    </Tree>
  {/each}

  <div class="pane">
    {#if listError}
      <p class="note warn">Could not read this machine's work: {listError}</p>
    {:else if held.length === 0 && !loading}
      <p class="note">
        Nothing to show. This machine reports no agent sessions it can read.
      </p>
    {/if}

    {#if hasEarlier}
      <Button variant="quiet" disabled={loading} onclick={more}>
        {loading ? "Loading…" : "Load earlier"}
      </Button>
    {/if}

    <div class="danger">
      <Button variant="quiet" onclick={forget}>Forget this server</Button>
    </div>
  </div>
{/if}

{#snippet twig(conversation: Conversation)}
  <!-- A conversation with no readable transcript has no chat to open, but it
       still has a pane. That one opens the terminal instead, which is what the
       Chat/Terminal split means for a session nothing wrote down. -->
  <button
    class="open"
    type="button"
    disabled={!conversation.has_transcript && !conversation.workspace}
    onclick={() => read(conversation)}
  >
    <div class="head">
      <b>{Conversations.title(conversation)}</b>
      {#if Conversations.age(conversation)}
        <span class="age">{Conversations.age(conversation)}</span>
      {/if}
    </div>
    <div class="meta">{Conversations.meta(conversation)}</div>
    <div class="meta">{conversation.cwd}</div>
    {#if conversation.preview}
      <div class="said">“{Parts.plain(conversation.preview)}”</div>
    {/if}
    {#if !conversation.has_transcript}
      <div class="meta">
        {conversation.workspace ? "terminal only — nothing wrote a transcript" : "no readable transcript"}
      </div>
    {/if}
  </button>
{/snippet}

<style lang="scss">
  .open {
    display: block;
    width: 100%;
    padding: 0;
    border: none;
    background: none;
    text-align: left;
    color: inherit;
    font: inherit;

    &:disabled {
      opacity: 0.62;
    }
  }

  .pane {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px 18px 24px;
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: 8px;

    b {
      font-size: 14px;
      font-weight: 600;
      letter-spacing: -0.015em;
    }
  }

  .age {
    font-family: var(--tc-mono);
    font-size: 10px;
    color: var(--tc-ink-3);
    margin-left: auto;
  }

  .meta {
    font-family: var(--tc-mono);
    font-size: 10px;
    color: var(--tc-ink-3);
  }

  .said {
    margin-top: 3px;
    font-size: 12.5px;
    color: var(--tc-ink-2);
    line-height: 1.4;
  }

  .note {
    margin: 0;
    font-size: 13px;
    line-height: 1.5;
    color: var(--tc-ink-3);
  }

  .warn {
    color: var(--tc-ink-2);
  }

  .danger {
    margin-top: 24px;
  }
</style>
