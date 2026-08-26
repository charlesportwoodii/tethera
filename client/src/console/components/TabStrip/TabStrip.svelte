<script lang="ts">
  import Icon from "$console/components/Icon/Icon.svelte";
  import StatusGlyph from "$console/components/StatusGlyph/StatusGlyph.svelte";
  import type { TabStripProps } from "./TabStrip.types";

  let { tabs, activeId, onselect, onadd = null }: TabStripProps = $props();
</script>

<div class="tc-tabs" role="tablist" aria-label="Tabs in this workspace">
  {#each tabs as tab (tab.id)}
    <button
      class="tc-tabs__tab"
      class:is-active={tab.id === activeId}
      type="button"
      role="tab"
      aria-selected={tab.id === activeId}
      onclick={() => onselect?.(tab.id)}
    >
      {tab.label}
      {#if tab.state}
        <StatusGlyph state={tab.state} size={9} />
      {/if}
    </button>
  {/each}
  {#if onadd}
    <button class="tc-tabs__tab" type="button" aria-label="New tab" onclick={onadd}>
      <Icon name="plus" size={12} />
    </button>
  {/if}
</div>

<style lang="scss">
  @use "./TabStrip.scss";
</style>
