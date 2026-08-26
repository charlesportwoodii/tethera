<script lang="ts">
  import type { Snippet } from "svelte";
  import type { DrawerHeight } from "$console/types/state";
  import type { DrawerProps } from "./Drawer.types";

  let {
    height = "peek",
    label,
    summary = null,
    onheight,
    children,
  }: DrawerProps & { children?: Snippet } = $props();

  // Three heights, cycled by the handle. Peek is not a closed state: the pane is
  // always there, which is the point of the drawer.
  const NEXT: Record<DrawerHeight, DrawerHeight> = {
    peek: "half",
    half: "full",
    full: "peek",
  };
</script>

<section class="tc-drawer is-{height}" data-height={height} aria-label="Pane">
  <span class="tc-drawer__pull" aria-hidden="true"></span>
  <button
    class="tc-drawer__head"
    type="button"
    aria-expanded={height !== "peek"}
    onclick={() => onheight?.(NEXT[height])}
  >
    <span class="tc-drawer__label">{label}</span>
    {#if summary}
      <span class="tc-drawer__summary">{summary}</span>
    {/if}
  </button>
  {#if height !== "peek"}
    <div class="tc-drawer__body">
      {@render children?.()}
    </div>
  {/if}
</section>

<style lang="scss">
  @use "./Drawer.scss";
</style>
