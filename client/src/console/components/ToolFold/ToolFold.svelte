<script lang="ts">
  import BrailleSpinner from "$console/components/BrailleSpinner/BrailleSpinner.svelte";
  import Icon from "$console/components/Icon/Icon.svelte";
  import type { ToolFoldProps } from "./ToolFold.types";

  let {
    name,
    detail = null,
    status = "ok",
    expanded = false,
    onclick,
  }: ToolFoldProps = $props();

  const tone = $derived(status === "failed" ? "attn" : status === "ok" ? "muted" : "run");
</script>

<button
  class="tc-fold"
  class:is-expanded={expanded}
  type="button"
  aria-expanded={expanded}
  data-status={status}
  {onclick}
>
  {#if status === "running"}
    <BrailleSpinner size={12} label={null} />
  {:else}
    <Icon name="chevron" size={12} />
  {/if}
  <span class="tc-fold__name">{name}</span>
  {#if detail}
    <span class="tc-fold__detail is-{tone}">{detail}</span>
  {/if}
</button>

<style lang="scss">
  @use "./ToolFold.scss";
</style>
