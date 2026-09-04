<script lang="ts">
  import WorkspaceMap from "$components/WorkspaceMap.svelte";
  import { type PaneStatus } from "$managers/floorplan";
  import type { Pane } from "$bindings/Pane";
  import type { SplitDirection } from "$bindings/SplitDirection";
  import type { TabLayout } from "$bindings/TabLayout";
  import type { TerminalControls } from "$bindings/TerminalControls";

  interface Props {
    panes: Pane[];
    layout: TabLayout | null;
    status?: PaneStatus;
    selected: string | null;
    controls: TerminalControls;
    onselect: (pane: string) => void;
    /** Show this pane, whatever is running in it. */
    onenter: (pane: string) => void;
    /** Switch chat to the agent in this pane. */
    onchat: (conversation: string) => void;
    onsplit: (pane: string, direction: SplitDirection) => void;
    onclosepane: (pane: string) => void;
  }

  let {
    panes,
    layout,
    status = {},
    selected,
    controls,
    onselect,
    onenter,
    onchat,
    onsplit,
    onclosepane,
  }: Props = $props();

  const held = $derived(
    panes.find((pane) => (pane.id as unknown as string) === selected) ?? null,
  );

  /**
   * Whether going into this pane means its chat rather than its terminal.
   *
   * Not a different *order* of actions, only a different destination for the
   * first one. Chat and the raw pane are the same intent — go into this thing —
   * so offering both put a choice on screen where there is none, and moved
   * every button below them by a row depending on what was selected.
   *
   * The raw terminal of an agent's pane is still reachable: the map enters a
   * pane on the second tap of its rectangle.
   */
  const talks = $derived(held !== null && held.agent !== null && held.conversation !== null);

  // Named so the button reads the way the map does. Falls back to the pane's
  // own label on the one frame where the map has not placed it yet.
  const ordinal = $derived.by(() => {
    if (held === null) {
      return "";
    }

    const agents = panes.filter((pane) => pane.agent !== null);
    const at = agents.findIndex((pane) => pane.id === held.id);

    return at < 0 ? held.label : `C${at + 1}`;
  });
</script>

<div class="sheet">
  <WorkspaceMap
    {panes}
    {layout}
    {status}
    {selected}
    variant="sheet"
    {onselect}
    onenter={controls.attach ? onenter : undefined}
  />

  <!--
    A fixed grid, and every action names its own cell. The slot a control sits
    in is a property of the control rather than of what happens to be selected,
    so switching from an agent's pane to a shell changes one label and moves
    nothing: split right stays bottom-left whichever pane is held, and a thumb
    reaching for it does not land on close.
  -->
  <div class="acts">
    {#if held}
      {#if talks}
        <button
          class="a primary enter"
          type="button"
          onclick={() => onchat(held.conversation as unknown as string)}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"
            ><path d="M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.4A8 8 0 1 1 21 12z" /></svg
          >
          Open {ordinal} chat
        </button>
      {:else if controls.attach && held.streamed}
        <button
          class="a primary enter"
          type="button"
          onclick={() => onenter(held.id as unknown as string)}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"
            ><path d="M9 10l-4 4 4 4M5 14h9a5 5 0 0 0 5-5V6" /></svg
          >
          Enter the pane
        </button>
      {/if}

      {#if controls.split}
        <button
          class="a right"
          type="button"
          onclick={() => onsplit(held.id as unknown as string, "horizontal")}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"
            ><rect x="3" y="3" width="18" height="18" rx="2" /><path d="M12 3v18" /></svg
          >
          Split right
        </button>

        <button
          class="a down"
          type="button"
          onclick={() => onsplit(held.id as unknown as string, "vertical")}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"
            ><rect x="3" y="3" width="18" height="18" rx="2" /><path d="M3 12h18" /></svg
          >
          Split down
        </button>
      {/if}

      {#if controls.close && panes.length > 1}
        <button
          class="a warn close"
          type="button"
          onclick={() => onclosepane(held.id as unknown as string)}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M18 6L6 18M6 6l12 12" /></svg>
          Close pane
        </button>
      {/if}
    {/if}
  </div>

  <p class="hint">
    {#if held === null}
      Tap a rectangle to choose a pane.
    {:else}
      Tap a rectangle to select it, and the selected one again to enter it. Everything here
      reaches your desk — the same layout is on both screens a moment later.
    {/if}
  </p>
</div>

<style lang="scss">
  .sheet {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    gap: 11px;
    padding: 12px 14px 0;
    background: var(--tc-bg);
    overflow: hidden;
  }

  // Three rows of two, each action pinned to its own cell rather than flowing.
  // Under `grid-auto-flow` an absent control pulls every later one up a slot, so
  // selecting a shell instead of an agent moved close pane to where split right
  // had been - which is the one button on this sheet you do not want somebody
  // reaching for by memory.
  .acts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-template-rows: repeat(3, auto);
    gap: 8px;
    flex: none;
  }

  .enter {
    grid-area: 1 / 1;
  }

  .right {
    grid-area: 1 / 2;
  }

  .down {
    grid-area: 2 / 1;
  }

  // Its own row, left, rather than beside a split. The destructive action does
  // not share an edge with one a thumb reaches for constantly.
  .close {
    grid-area: 3 / 1;
  }

  .a {
    display: flex;
    align-items: center;
    gap: 9px;
    margin: 0;
    padding: 11px 12px;
    border: 1px solid var(--tc-rule);
    border-radius: var(--tc-r-control);
    background: var(--tc-surface-2);
    color: var(--tc-ink);
    font-family: var(--tc-font);
    font-size: 12.5px;
    text-align: left;

    svg {
      flex: none;
      width: 16px;
      height: 16px;
      fill: none;
      stroke: var(--tc-accent);
      stroke-width: 1.6;
      stroke-linecap: round;
      stroke-linejoin: round;
    }
  }

  .primary {
    border-color: var(--tc-accent);
    background: var(--tc-accent);
    color: var(--tc-accent-ink);
    font-weight: 600;

    svg {
      stroke: var(--tc-accent-ink);
    }
  }

  .warn {
    border-color: color-mix(in srgb, var(--tc-attn) 40%, var(--tc-rule));
    color: var(--tc-attn);

    svg {
      stroke: var(--tc-attn);
    }
  }

  .hint {
    margin: 0;
    padding-bottom: 14px;
    font-size: 11.5px;
    line-height: 1.55;
    color: var(--tc-ink-3);
  }
</style>
