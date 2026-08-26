<script lang="ts">
  import type { ChipProps } from "./Chip.types";

  let { label, detail = null, selected = false, disabled = false, onclick }: ChipProps =
    $props();
</script>

<button
  class="tc-chip"
  class:is-selected={selected}
  type="button"
  role="radio"
  aria-checked={selected}
  {disabled}
  {onclick}
>
  {label}
  {#if detail}
    <span class="tc-chip__detail">{detail}</span>
  {/if}
</button>

<style lang="scss">
  @use "./Chip.scss";
</style>
