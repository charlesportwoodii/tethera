<script lang="ts">
  import Button from "$console/components/Button/Button.svelte";
  import EmptyState from "$console/components/EmptyState/EmptyState.svelte";
  import KeyBar from "$console/components/KeyBar/KeyBar.svelte";
  import PaneMap from "$console/components/PaneMap/PaneMap.svelte";
  import TabStrip from "$console/components/TabStrip/TabStrip.svelte";
  import TerminalView from "$console/components/TerminalView/TerminalView.svelte";
  import { DEFAULT_KEYS } from "$console/components/KeyBar/KeyBar.types";
  import type { TerminalPaneProps } from "./TerminalPane.types";

  let {
    tabs,
    activeTabId = null,
    tabStates = {},
    panes = [],
    activePaneId = null,
    grid,
    revision = 0,
    keys = DEFAULT_KEYS,
    onselecttab,
    onaddtab = null,
    onselectpane,
    onsplit = null,
    onkey,
    onfocuspane = null,
    emptyTitle = "Nothing open here",
    emptyBody = "This workspace has no terminal tab yet. Making one starts a shell in the workspace's own directory.",
    machine = null,
  }: TerminalPaneProps = $props();

  const empty = $derived(tabs.length === 0);
  const label = $derived(grid.cols + "×" + grid.rows);
</script>

<div class="tc-tpane" data-empty={empty}>
  <TabStrip
    {tabs}
    activeId={activeTabId}
    states={tabStates}
    onselect={onselecttab}
    onadd={onaddtab}
  />

  {#if empty}
    <!--
      No tabs is not an error. The strip above already carries the way to make
      the first one; this says what making it will do, because "nothing open"
      alone tells somebody what they can already see.
    -->
    <div class="tc-tpane__empty">
      <EmptyState icon="terminal" title={emptyTitle} body={emptyBody}>
        {#snippet actions()}
          {#if onaddtab}
            <Button icon="plus" onclick={onaddtab}>
              {machine ? "New tab on " + machine : "New tab"}
            </Button>
          {/if}
        {/snippet}
      </EmptyState>
    </div>
  {:else}
    <PaneMap {panes} activeId={activePaneId} onselect={onselectpane} {onsplit} />
    <TerminalView {grid} {revision} {label} onfocus={onfocuspane} />
    <KeyBar rows={keys} {onkey} />
  {/if}
</div>

<style lang="scss">
  @use "./TerminalPane.scss";
</style>
