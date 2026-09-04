<script lang="ts">
  import StatusGlyph from "$console/components/StatusGlyph/StatusGlyph.svelte";
  import type { StatusStripProps } from "./StatusStrip.types";

  let { states, cap = 8, size = 11, glyph }: StatusStripProps = $props();

  const shown = $derived(states.slice(0, cap));
  const hidden = $derived(Math.max(0, states.length - cap));
</script>

{#if states.length > 0}
  <div class="tc-strip" role="img" aria-label="{states.length} sessions">
    {#each shown as state, i (i)}
      {#if glyph}
        {@render glyph(state, size)}
      {:else}
        <StatusGlyph {state} {size} />
      {/if}
    {/each}

    <!-- A figure rather than a smaller mark. Nine marks and a tenth half-drawn
         reads as ten sessions; "+3" is the only form that stays countable. -->
    {#if hidden > 0}
      <span class="tc-strip__more">+{hidden}</span>
    {/if}
  </div>
{/if}

<style lang="scss">
  @use "./StatusStrip.scss";
</style>
