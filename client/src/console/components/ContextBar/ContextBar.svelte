<script lang="ts">
  import { formatTokens } from "$console/lib/format";
  import type { ContextBarProps } from "./ContextBar.types";

  let { used, window: total, warnAt = 0.75, bare = false }: ContextBarProps = $props();

  const fraction = $derived(total > 0 ? Math.min(1, Math.max(0, used / total)) : 0);
  const percent = $derived(Math.round(fraction * 100));
  const warn = $derived(fraction >= warnAt);
</script>

<div class="tc-ctx" data-warn={warn ? "true" : "false"}>
  <div
    class="tc-ctx__track"
    role="progressbar"
    aria-label="Context used"
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow={percent}
    aria-valuetext="{percent}% of context used"
  >
    <span class="tc-ctx__fill" class:is-warn={warn} style:width="{percent}%"></span>
  </div>
  {#if !bare}
    <div class="tc-ctx__labels">
      <span>context {percent}%</span>
      <span>{formatTokens(used)} / {formatTokens(total)}</span>
    </div>
  {/if}
</div>

<style lang="scss">
  @use "./ContextBar.scss";
</style>
