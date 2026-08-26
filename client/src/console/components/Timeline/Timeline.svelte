<script lang="ts">
  import type { Snippet } from "svelte";
  import type { TimelineProps } from "./Timeline.types";

  let { label = "Transcript", children }: TimelineProps & { children?: Snippet } = $props();
</script>

<div class="tc-timeline" role="log" aria-label={label} aria-live="polite">
  {@render children?.()}
</div>

<style lang="scss">
  @use "./Timeline.scss";
</style>
