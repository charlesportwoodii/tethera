<script lang="ts">
  import StatusStrip from "./StatusStrip.svelte";
  import type { GlyphState } from "$console/types/state";

  let { states }: { states: GlyphState[] } = $props();
</script>

<StatusStrip {states}>
  {#snippet glyph(state: GlyphState)}
    <span class="mine" data-state={state}></span>
  {/snippet}
</StatusStrip>
