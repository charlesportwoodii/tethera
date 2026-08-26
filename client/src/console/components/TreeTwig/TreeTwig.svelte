<script lang="ts">
  import type { Snippet } from "svelte";
  import StatusGlyph from "$console/components/StatusGlyph/StatusGlyph.svelte";
  import type { TreeTwigProps } from "./TreeTwig.types";

  let {
    state,
    compact = false,
    glyph,
    children,
  }: TreeTwigProps & { glyph?: Snippet; children?: Snippet } = $props();
</script>

<div class="tc-twig" class:is-compact={compact} role="listitem">
  <span class="tc-twig__elbow" aria-hidden="true"></span>
  <span class="tc-twig__glyph">
    {#if glyph}
      {@render glyph()}
    {:else if state}
      <StatusGlyph {state} bg="var(--tc-surface)" />
    {/if}
  </span>
  {@render children?.()}
</div>

<style lang="scss">
  @use "./TreeTwig.scss";
</style>
