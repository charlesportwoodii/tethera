<script lang="ts">
  import BrailleSpinner from "$console/components/BrailleSpinner/BrailleSpinner.svelte";
  import ContextBar from "$console/components/ContextBar/ContextBar.svelte";
  import { formatDuration, formatTokens } from "$console/lib/format";
  import type { ThinkingRowProps } from "./ThinkingRow.types";

  let {
    stats,
    activity = null,
    verb = "Thinking",
    dense = false,
    offset = 0,
    onstop = null,
  }: ThinkingRowProps = $props();

  const hasContext = $derived(
    typeof stats.contextUsed === "number" &&
      typeof stats.contextWindow === "number" &&
      stats.contextWindow > 0,
  );
</script>

<div class="tc-think" class:is-dense={dense} data-dense={dense}>
  <div class="tc-think__top">
    <BrailleSpinner {offset} label={null} />
    <span class="tc-think__verb">{verb}</span>
    <span class="tc-think__elapsed">{formatDuration(stats.elapsedSeconds)}</span>
    {#if dense && activity}
      <span class="tc-think__activity">{activity}</span>
    {/if}
    {#if onstop}
      <button class="tc-think__stop" type="button" onclick={onstop}>esc to stop</button>
    {/if}
  </div>

  {#if !dense && activity}
    <div class="tc-think__activity">{activity}</div>
  {/if}

  <div class="tc-think__stats">
    <span class="tc-think__stat"><b>{formatTokens(stats.tokensIn)}</b><span>in</span></span>
    <span class="tc-think__stat"><b>{formatTokens(stats.tokensOut)}</b><span>out</span></span>
    <span class="tc-think__stat"><b>{stats.tools}</b><span>tools</span></span>
    {#if dense && stats.model}
      <span class="tc-think__stat"><b>{stats.model}</b><span>model</span></span>
    {/if}
    {#if dense && typeof stats.costUsd === "number"}
      <span class="tc-think__stat"><b>${stats.costUsd.toFixed(2)}</b><span>this turn</span></span>
    {/if}
    {#if dense && hasContext}
      <span class="tc-think__ctx">
        <ContextBar used={stats.contextUsed ?? 0} window={stats.contextWindow ?? 1} />
      </span>
    {/if}
  </div>

  {#if !dense && hasContext}
    <ContextBar used={stats.contextUsed ?? 0} window={stats.contextWindow ?? 1} />
  {/if}
</div>

<style lang="scss">
  @use "./ThinkingRow.scss";
</style>
