<script lang="ts">
  import ConversationGlyph from "$components/ConversationGlyph.svelte";
  import { Floorplan, type PaneStatus } from "$managers/floorplan";
  import type { Pane } from "$bindings/Pane";
  import type { TabLayout } from "$bindings/TabLayout";

  interface Props {
    panes: Pane[];
    /** Null when the machine will not report geometry. The map then draws nothing. */
    layout: TabLayout | null;
    /** Agent state per pane id — the same shape `TabStrip` takes per tab id. */
    status?: PaneStatus;
    selected?: string | null;
    /**
     * `strip` is the always-visible one under the tab bar: thumbnail size, no
     * text but the ordinals. `sheet` is the full-width editor, where every
     * rectangle has room to say what it is.
     */
    variant?: "strip" | "sheet";
    onselect?: (pane: string) => void;
    /** Tapping an already-selected rectangle. Absent means selection is all this does. */
    onenter?: (pane: string) => void;
  }

  let {
    panes,
    layout,
    status = {},
    selected = null,
    variant = "strip",
    onselect,
    onenter,
  }: Props = $props();

  const placed = $derived(Floorplan.place(panes, layout, status));

  /**
   * Select, then enter.
   *
   * A single tap that entered a pane would switch what the screen is showing
   * every time a thumb brushed the map, which on a phone is often. The second
   * tap is the commitment.
   */
  function press(id: string): void {
    if (id === selected && onenter) {
      onenter(id);

      return;
    }

    onselect?.(id);
  }
</script>

<!--
  Absent rather than empty. `Floorplan.place` returns nothing for a tab whose
  layout does not describe every pane, and an empty bordered box would read as
  a workspace with no panes in it rather than as geometry this machine will not
  report.
-->
{#if placed.length > 0}
  <div class="map" class:sheet={variant === "sheet"} role="group" aria-label="Pane layout">
    {#each placed as cell (cell.pane.id)}
      {@const id = cell.pane.id as unknown as string}
      <button
        class="cell"
        class:on={id === selected}
        class:zoomed={cell.zoomed}
        type="button"
        aria-pressed={id === selected}
        aria-label={cell.detail === null ? cell.name : `${cell.name}, ${cell.detail}`}
        style="left:{cell.left}%;top:{cell.top}%;width:{cell.width}%;height:{cell.height}%"
        onclick={() => press(id)}
      >
        <span class="head">
          {#if cell.status}
            <ConversationGlyph state={cell.status} size={variant === "sheet" ? 9 : 6} />
          {/if}
          <span class="name">{cell.name}</span>
        </span>

        {#if variant === "sheet"}
          {#if cell.detail}
            <span class="detail">{cell.detail}</span>
          {/if}
          <span class="size">{cell.pane.size.cols} × {cell.pane.size.rows}</span>
        {/if}
      </button>
    {/each}
  </div>
{/if}

<style lang="scss">
  .map {
    position: relative;
    flex: none;
    width: 62px;
    height: 36px;
    overflow: hidden;
    border-radius: 5px;
    background: var(--tc-term-chrome);
    box-shadow: inset 0 0 0 1px var(--tc-rule-2);
  }

  .sheet {
    width: 100%;
    height: 246px;
    border-radius: 12px;
  }

  // Absolute, because the rects are a tiling and not a flow. Percentages come
  // from `Floorplan`, which has already normalised them against the tab's own
  // origin — the desk's window may start this tab at column 29.
  .cell {
    position: absolute;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: flex-start;
    gap: 2px;
    overflow: hidden;
    margin: 0;
    padding: 0;
    border: 0;
    background: var(--tc-term-bg);
    box-shadow: inset 0 0 0 1px var(--tc-term-chrome);
    color: var(--tc-term-dim);
    font-family: var(--tc-mono);
    font-size: 7px;
    text-align: left;
  }

  .on {
    background: color-mix(in srgb, var(--tc-accent) 82%, transparent);
    color: var(--tc-accent-ink);
  }

  // A zoomed pane is covering its neighbours at the desk, so it is drawn over
  // them rather than beside them. Without this the map says the tab is split
  // while the screen it mirrors shows one pane.
  .zoomed {
    outline: 1px solid var(--tc-accent);
    outline-offset: -1px;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }

  .name {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .sheet {
    .cell {
      justify-content: space-between;
      padding: 7px 8px;
      font-size: 10.5px;
      box-shadow: inset 0 0 0 1px var(--tc-rule);
      color: var(--tc-ink);
    }

    .on {
      background: color-mix(in srgb, var(--tc-accent) 12%, var(--tc-term-bg));
      box-shadow: inset 0 0 0 2px var(--tc-accent);
      color: var(--tc-ink);
    }

    .detail,
    .size {
      max-width: 100%;
      overflow: hidden;
      white-space: nowrap;
      text-overflow: ellipsis;
      font-size: 8.5px;
      color: var(--tc-term-dim);
    }
  }
</style>
