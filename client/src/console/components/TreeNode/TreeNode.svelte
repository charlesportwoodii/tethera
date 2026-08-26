<script lang="ts">
  import type { Snippet } from "svelte";
  import StatusGlyph from "$console/components/StatusGlyph/StatusGlyph.svelte";
  import type { TreeNodeProps } from "./TreeNode.types";

  let {
    state,
    branches = false,
    dim = false,
    spaced = false,
    glyph,
    children,
  }: TreeNodeProps & { glyph?: Snippet; children?: Snippet } = $props();
</script>

<div
  class="tc-node"
  class:is-dim={dim}
  class:is-spaced={spaced}
  role="listitem"
  data-branches={branches}
>
  <span class="tc-node__glyph">
    {#if glyph}
      {@render glyph()}
    {:else if state}
      <StatusGlyph {state} bg="var(--tc-surface)" />
    {/if}
  </span>
  {#if branches}
    <span class="tc-node__trunk" aria-hidden="true"></span>
  {/if}
  {@render children?.()}
</div>

<style lang="scss">
  @use "./TreeNode.scss";
</style>
