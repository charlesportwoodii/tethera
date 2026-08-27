<script lang="ts">
  import type { StatusLineProps } from "./StatusLine.types";

  let { label, detail = null }: StatusLineProps = $props();
</script>

<div class="tc-status">
  <span class="tc-status__label">{label}</span>
  {#if detail}
    <span class="tc-status__detail">{detail}</span>
  {/if}
</div>

<style lang="scss">
  @use "./StatusLine.scss";
</style>
