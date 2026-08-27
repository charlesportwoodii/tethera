<script lang="ts">
  import type { PaneMapProps } from "./PaneMap.types";

  let { panes, activeId = null, onselect, onsplit = null }: PaneMapProps = $props();

  const active = $derived(panes.find((p) => p.id === activeId) ?? null);
  const percent = (value: number) => Math.max(0, Math.min(1, value)) * 100 + "%";
</script>

<!--
  Absent for a single pane. A tab that was never split has no layout to show, and
  a diagram of one rectangle is furniture. The phone views one pane at a time; the
  map is how it says which one, and which others exist.
-->
{#if panes.length > 1}
  <div class="tc-panemap" data-panes={panes.length}>
    <div class="tc-panemap__grid" role="group" aria-label="Panes in this tab">
      {#each panes as pane (pane.id)}
        <button
          class="tc-panemap__cell"
          class:is-active={pane.id === activeId}
          type="button"
          aria-label={pane.label}
          aria-pressed={pane.id === activeId}
          style:left={percent(pane.x)}
          style:top={percent(pane.y)}
          style:width={percent(pane.w)}
          style:height={percent(pane.h)}
          onclick={() => onselect?.(pane.id)}
        ></button>
      {/each}
    </div>

    <span class="tc-panemap__label">{active ? active.label : panes.length + " panes"}</span>

    {#if onsplit}
      <span class="tc-panemap__split">
        <button
          class="tc-panemap__button"
          type="button"
          aria-label="Split beside"
          onclick={() => onsplit("vertical")}>&#9707;</button
        >
        <button
          class="tc-panemap__button"
          type="button"
          aria-label="Split below"
          onclick={() => onsplit("horizontal")}>&#9707;</button
        >
      </span>
    {/if}
  </div>
{/if}

<style lang="scss">
  @use "./PaneMap.scss";
</style>
