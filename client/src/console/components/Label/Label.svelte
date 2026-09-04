<script lang="ts">
  import type { Snippet } from "svelte";
  import type { LabelProps } from "./Label.types";

  let {
    kind = "section",
    flush = false,
    rule = false,
    count = null,
    tone = "quiet",
    children,
  }: LabelProps & { children?: Snippet } = $props();

  // Zero is a figure worth drawing. Testing truthiness here would hide it, and a
  // band headed with a zero says something a missing count does not.
  const counted = $derived(count !== null && count !== undefined);
</script>

<span
  class="tc-label is-{kind}"
  class:is-flush={flush}
  class:is-ruled={rule}
  data-kind={kind}
  data-tone={tone}
>
  <span class="tc-label__word">{@render children?.()}</span>

  {#if rule}
    <i class="tc-label__rule" aria-hidden="true"></i>
  {/if}

  {#if counted}
    <span class="tc-label__count">{count}</span>
  {/if}
</span>

<style lang="scss">
  @use "./Label.scss";
</style>
