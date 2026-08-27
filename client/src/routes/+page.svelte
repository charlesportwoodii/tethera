<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { Parts } from "$managers/parts";
  import { invoke } from "@tauri-apps/api/core";
  import { Button, ConnDot, Icon, NavBar, Tree, TreeNode, TreeTwig } from "$console";
  import { ServerManager } from "$managers/server_manager";
  import { DeepLink } from "$managers/deep_link";
  import { Conversations } from "$managers/conversations";
  import ConversationGlyph from "$components/ConversationGlyph.svelte";
  import type { Conversation } from "$bindings/Conversation";
  import type { ServerRow } from "$bindings/ServerRow";

  const manager = new ServerManager(invoke);
  const rows = manager.rows;
  const sweeping = manager.sweeping;

  let link: DeepLink | null = null;
  let unfollow: (() => void) | null = null;

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

  // ConnDot has no "unknown" state, so a row that has not settled shows words
  // instead of a dot rather than claiming a route nothing has measured.
  /**
   * Straight into the conversation from the list.
   *
   * The twig already says which one it is, so making the person open the
   * machine first to reach the same row is a step that answers nothing.
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

  <Tree label="Servers">
    {#each $rows as row (row.entry.server.id)}
      <TreeNode
        dim={row.link.kind === "offline"}
        branches={row.entry.conversations.length > 0}
        spaced
      >
        <!-- The console draws `idle` as a grey hollow ring, which is the mark
             for a machine that did not answer. On the column somebody scans
             down, a machine answering in 5 ms and one that has been quiet for a
             day were the same shape. -->
        {#snippet glyph()}
          <ConversationGlyph
            state={row.link.kind === "offline" ? "offline" : "idle"}
            bg="var(--tc-surface)"
          />
        {/snippet}

        <div class="line">
          <button class="row" onclick={() => open(row)}>
            <strong class="name">{row.entry.server.label}</strong>
            <span class="meta">{row.entry.server.os} · {row.entry.server.arch}</span>

            {#if row.refusal}
              <span class="meta refused">would not accept this device</span>
            {:else if row.link.kind !== "unknown"}
              <ConnDot link={row.link.kind} rttMs={row.link.rtt_ms} />
            {:else}
              <span class="meta">finding a route…</span>
            {/if}
          </button>

          <!-- Per server, on the server's own row. A single button at the foot
               would belong to whichever machine you last thought about, which is
               not a question this screen can answer. -->
          <button
            class="bare start"
            onclick={() => startOn(row)}
            aria-label={`New session on ${row.entry.server.label}`}
          >
            <Icon name="plus" />
          </button>
        </div>

        <!-- Remembered when the machine is not answering, which is the one
             useful thing on an otherwise empty row: what was running when it
             went quiet. -->
        {#each row.entry.conversations as held (held.id)}
          <TreeTwig>
            {#snippet glyph()}
              <ConversationGlyph state={Conversations.glyph(held, row.link.kind !== "offline")} />
            {/snippet}
            <button class="open" type="button" onclick={() => read(row, held)}>
              <div class="twig">
                <b>{Conversations.title(held)}</b>
                {#if Conversations.age(held)}<span class="age">{Conversations.age(held)}</span>{/if}
              </div>
              <div class="meta">{Conversations.meta(held)}</div>
              {#if held.preview}
                <div class="said">“{Parts.plain(held.preview)}”</div>
              {/if}
            </button>
          </TreeTwig>
        {/each}
      </TreeNode>
    {/each}
  </Tree>

  <!-- The scan mark in the NavBar is the same action, but it is small and sits
       against the status bar. Pairing a second machine is a first-class thing to
       do from this screen, so it gets a control that reads as one. -->
  <div class="foot">
    <Button icon="scan" onclick={() => goto("/pair")}>Pair another server</Button>
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

  .bare,
  .row {
    background: none;
    border: 0;
    padding: 0;
    color: inherit;
    cursor: pointer;
    font: inherit;
  }

  .line {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    width: 100%;
  }

  .row {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 3px;
    flex: 1;
    min-width: 0;
    text-align: left;
  }

  // A finger, not a cursor. The mark is ~18px; without a target built around it
  // the control is a quarter of the 48dp Android asks for and misses more often
  // than it hits.
  //
  // Sized with padding rather than negative margins: pulling the box outside the
  // node's content area put it where the tap never arrived, which reads exactly
  // like a dead button.
  .start {
    flex: none;
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    color: var(--tc-ink-3);
  }

  .name {
    font-family: var(--tc-mono);
    font-size: 16px;
    letter-spacing: -0.02em;
  }

  .meta {
    font-family: var(--tc-mono);
    font-size: 10px;
    color: var(--tc-ink-3);
  }

  .refused {
    color: var(--tc-warn, var(--tc-ink-2));
  }

  .open {
    display: block;
    width: 100%;
    padding: 0;
    border: none;
    background: none;
    text-align: left;
    color: inherit;
    font: inherit;
  }

  .twig {
    display: flex;
    align-items: baseline;
    gap: 8px;

    b {
      font-size: 13.5px;
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

  .said {
    margin-top: 3px;
    font-size: 12.5px;
    color: var(--tc-ink-2);
    line-height: 1.4;
  }

  .foot {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
    padding: 24px 18px 32px;
  }
</style>
