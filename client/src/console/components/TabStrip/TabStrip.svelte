<script lang="ts">
  import Icon from "$console/components/Icon/Icon.svelte";
  import StatusGlyph from "$console/components/StatusGlyph/StatusGlyph.svelte";
  import type { TabStripProps } from "./TabStrip.types";

  let { tabs, activeId = null, states = {}, onselect, onadd = null }: TabStripProps = $props();

  // Sorted by the backend's ordinal rather than by arrival, so the row does not
  // reorder when a watch event lands out of order.
  const ordered = $derived([...tabs].sort((a, b) => a.index - b.index));
</script>

<div class="tc-tabs" role="tablist" aria-label="Tabs in this workspace">
  {#if ordered.length === 0}
    <!--
      No tabs is not an error and not an empty row: the only useful thing on this
      strip is the way to make the first one, so it says so in words.
    -->
    {#if onadd}
      <button class="tc-tabs__first" type="button" onclick={onadd}>
        <Icon name="plus" size={12} />
        New tab
      </button>
    {:else}
      <span class="tc-tabs__none">No tabs open</span>
    {/if}
  {:else}
    {#each ordered as tab (tab.id)}
      <button
        class="tc-tabs__tab"
        class:is-active={tab.id === activeId}
        type="button"
        role="tab"
        aria-selected={tab.id === activeId}
        onclick={() => onselect?.(tab.id)}
      >
        {tab.index}:{tab.title}
        {#if states[tab.id]}
          <StatusGlyph state={states[tab.id]} size={9} />
        {/if}
      </button>
    {/each}
    {#if onadd}
      <button class="tc-tabs__tab" type="button" aria-label="New tab" onclick={onadd}>
        <Icon name="plus" size={12} />
      </button>
    {/if}
  {/if}
</div>

<style lang="scss">
  @use "./TabStrip.scss";
</style>
