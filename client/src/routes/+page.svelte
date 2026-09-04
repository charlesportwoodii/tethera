<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import { Button, Icon, Label, NavBar, ServerTile } from "$console";
  import { ServerManager } from "$managers/server_manager";
  import { DeepLink } from "$managers/deep_link";
  import { Conversations } from "$managers/conversations";
  import { Fleet } from "$managers/fleet";
  import ConversationGlyph from "$components/ConversationGlyph.svelte";
  import type { Conversation } from "$bindings/Conversation";
  import type { ServerRow } from "$bindings/ServerRow";

  /**
   * How many sessions the column under the fleet shows.
   *
   * The fleet is what this screen is for; the sessions below it are a way in,
   * not an index. The machine page pages the rest properly.
   */
  const SESSIONS_SHOWN = 5;

  const manager = new ServerManager(invoke);
  const rows = manager.rows;
  const sweeping = manager.sweeping;

  let link: DeepLink | null = null;
  let unfollow: (() => void) | null = null;

  const waiting = $derived(Fleet.waiting($rows));
  const sessions = $derived(Fleet.recent($rows, SESSIONS_SHOWN));

  onMount(() => {
    void start();
  });

  onDestroy(() => {
    link?.stop();
    unfollow?.();
  });

  async function start(): Promise<void> {
    await manager.load();

    link = new DeepLink((uri) => {
      void goto(`/pair?uri=${encodeURIComponent(uri)}`);
    });
    await link.start();

    await manager.sweep();

    // What a sweep carries back is a snapshot of which sessions are attached
    // and what each is doing. Taken once, the marks on this screen stop
    // meaning anything the moment a session starts or finishes work.
    unfollow = manager.follow();
  }

  function subtitle(all: ServerRow[]): string {
    const quiet = all.filter((row) => row.link.kind === "offline").length;
    const paired = `${all.length} paired`;

    return quiet === 0 ? paired : `${paired} · ${quiet} not answering`;
  }

  /**
   * How long a machine has been quiet, for the tile that replaced its route.
   *
   * Absent while the machine is answering: the round trip is the live fact
   * there, and a last-seen date beside it would read as a second measurement.
   */
  function lastSeen(row: ServerRow): string | null {
    const at = row.entry.last_seen_at;

    if (row.link.kind !== "offline" || at === null || at === undefined) {
      return null;
    }

    const days = Math.floor((Date.now() - Number(at)) / 86_400_000);

    return days > 0 ? `${days}d` : "today";
  }

  /**
   * Straight into the conversation from the list.
   *
   * The row already says which one it is, so making the person open the machine
   * first to reach the same row is a step that answers nothing.
   */
  function read(row: ServerRow, held: Conversation): void {
    const server = encodeURIComponent(row.entry.server.id as unknown as string);
    const id = encodeURIComponent(held.id as unknown as string);

    void goto(`/conversation?server=${server}&id=${id}`);
  }

  function open(row: ServerRow): void {
    void goto(`/server?id=${encodeURIComponent(row.entry.server.id as unknown as string)}`);
  }

  function startOn(row: ServerRow): void {
    void goto(`/session?server=${encodeURIComponent(row.entry.server.id as unknown as string)}`);
  }
</script>

{#if $rows.length === 0}
  <NavBar title="Tethera" subtitle="no servers paired">
    {#snippet actions()}
      <button class="tap" onclick={() => goto("/settings")} aria-label="Settings">
        <Icon name="settings" size={22} />
      </button>
    {/snippet}
  </NavBar>

  <section class="empty">
    <Icon name="scan" size={40} label="Pair a server" />
    <h3>No servers yet</h3>
    <p>
      Tethera reaches the agent sessions running on your own machines. Pair one to begin —
      everything else on this screen appears once you have.
    </p>
    <Button icon="scan" onclick={() => goto("/pair")}>Pair a server</Button>
  </section>
{:else}
  <NavBar title="Servers" subtitle={subtitle($rows)}>
    {#snippet actions()}
      <button class="tap" onclick={() => goto("/settings")} aria-label="Settings">
        <Icon name="settings" size={22} />
      </button>
    {/snippet}
  </NavBar>

  <!-- Absent rather than empty when nothing is waiting. A band that says
       "nothing needs you" is a row read every day to learn nothing, and it
       pushes the fleet down to earn it.

       Read-only, and it stays that way until a conversation carries its pending
       question on the wire. Answering needs the question's id and fingerprint,
       and the fingerprint is what the machine checks to refuse a stale answer —
       so there is nothing here to invent one from. -->
  {#if waiting.length > 0}
    <Label flush rule tone="urgent" count={waiting.length}>Needs you</Label>

    {#each waiting as held (held.conversation.id)}
      <button class="ask" type="button" onclick={() => read(held.row, held.conversation)}>
        <span class="ask__who">
          <ConversationGlyph state={Conversations.glyph(held.conversation, true)} />
          <b>{held.row.entry.server.label}</b>
          <span class="ask__meta">{Conversations.meta(held.conversation)}</span>
          {#if Conversations.age(held.conversation)}
            <em>{Conversations.age(held.conversation)}</em>
          {/if}
        </span>

        {#if Conversations.preview(held.conversation)}
          <span class="ask__q">{Conversations.preview(held.conversation)}</span>
        {/if}

        <span class="ask__src">{Conversations.title(held.conversation)}</span>
      </button>
    {/each}
  {/if}

  <Label flush rule count={$rows.length}>Fleet</Label>

  <div class="tiles">
    {#each $rows as row (row.entry.server.id)}
      <ServerTile
        label={row.entry.server.label}
        os={row.entry.server.os}
        arch={row.entry.server.arch}
        link={row.link.kind}
        rttMs={row.link.rtt_ms}
        lastSeen={lastSeen(row)}
        refusal={row.refusal ? "would not accept this device" : null}
        states={Fleet.states(row)}
        summary={Fleet.sentence(row)}
        attention={Fleet.attention(row)}
        onopen={() => open(row)}
        onstart={() => startOn(row)}
      >
        <!-- The app's own mark, not the console's. `ConversationGlyph` draws an
             idle session filled, where `StatusGlyph` draws it as a grey hollow
             ring — which is this screen's mark for a machine that is not
             answering. Left to the default, a healthy session read as
             unreachable in the tile and as alive in the row beneath it. -->
        {#snippet glyph(state, size)}
          <ConversationGlyph {state} {size} />
        {/snippet}
      </ServerTile>
    {/each}
  </div>

  <!-- Sessions rather than Recent. The sweep asks for what is bound to a pane,
       so this column is what is running now, not what happened lately. -->
  {#if sessions.length > 0}
    <Label flush rule count={sessions.length}>Sessions</Label>

    <div class="rows">
      {#each sessions as held (held.conversation.id)}
        <button class="line" type="button" onclick={() => read(held.row, held.conversation)}>
          <ConversationGlyph
            state={Conversations.glyph(held.conversation, held.row.link.kind !== "offline")}
          />
          <span class="line__t">
            <b>{Conversations.title(held.conversation)}</b>
            <span>{held.row.entry.server.label} · {Conversations.meta(held.conversation)}</span>
          </span>
          {#if Conversations.age(held.conversation)}
            <span class="line__age">{Conversations.age(held.conversation)}</span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}

  <div class="foot">
    <Button icon="scan" variant="quiet" onclick={() => goto("/pair")}>Pair another server</Button>
    <Button variant="quiet" disabled={$sweeping} onclick={() => manager.sweep()}>
      {$sweeping ? "Checking…" : "Refresh"}
    </Button>
  </div>
{/if}

<style lang="scss">
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 64px 24px;
    text-align: center;

    h3 {
      margin: 0;
      font-size: 1.1rem;
    }

    p {
      margin: 0 0 8px;
      max-width: 46ch;
      color: var(--tc-ink-2);
      font-size: 14px;
      line-height: 1.5;
    }
  }

  // A glyph is not a target. Forty-four is the smallest square a thumb hits
  // reliably, and the icon inside it stays small - the padding is the target,
  // not the mark.
  .tap {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    margin: -11px -11px -11px 0;
    background: none;
    border: 0;
    color: inherit;
    cursor: pointer;
  }

  .tiles {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 0 var(--tc-pad);
  }

  .ask {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: calc(100% - var(--tc-pad) * 2);
    margin: 0 var(--tc-pad) 10px;
    padding: 12px 13px;
    background: var(--tc-surface-2);
    border: 0;
    border-radius: var(--tc-r-control);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--tc-attn) 42%, transparent);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .ask__who {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    font-family: var(--tc-mono);
    font-size: 9.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--tc-ink-3);

    b {
      color: var(--tc-ink-2);
      font-weight: 500;
    }

    em {
      font-style: normal;
      margin-left: auto;
      flex: none;
    }
  }

  .ask__meta {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ask__q {
    font-size: 14px;
    color: var(--tc-ink);
    line-height: 1.4;
  }

  .ask__src {
    font-family: var(--tc-mono);
    font-size: 9.5px;
    color: var(--tc-ink-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .rows {
    display: flex;
    flex-direction: column;
    padding: 0 var(--tc-pad);
  }

  .line {
    display: flex;
    align-items: flex-start;
    gap: 11px;
    width: 100%;
    padding: 11px 2px;
    background: none;
    border: 0;
    border-bottom: 1px solid var(--tc-rule);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;

    &:last-child {
      border-bottom: 0;
    }
  }

  .line__t {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;

    b {
      font-size: 14px;
      font-weight: 500;
      letter-spacing: -0.01em;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    span {
      font-family: var(--tc-mono);
      font-size: 9.5px;
      color: var(--tc-ink-3);
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
  }

  .line__age {
    flex: none;
    margin-top: 2px;
    font-family: var(--tc-mono);
    font-size: 10px;
    color: var(--tc-ink-3);
  }

  .ask,
  .line,
  .tap {
    -webkit-tap-highlight-color: transparent;
  }

  .foot {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
    padding: 24px 18px 32px;
  }
</style>
