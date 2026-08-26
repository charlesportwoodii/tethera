<script lang="ts">
  import Icon from "$console/components/Icon/Icon.svelte";
  import type { ToolFoldProps } from "./ToolFold.types";

  let {
    name,
    detail = null,
    tone = "muted",
    expanded = false,
    onclick,
  }: ToolFoldProps = $props();
</script>

<button
  class="tc-fold"
  class:is-expanded={expanded}
  type="button"
  aria-expanded={expanded}
  {onclick}
>
  <Icon name="chevron" size={12} />
  <span class="tc-fold__name">{name}</span>
  {#if detail}
    <span class="tc-fold__detail is-{tone}">{detail}</span>
  {/if}
</button>

<style lang="scss">
  @use "./ToolFold.scss";
</style>
