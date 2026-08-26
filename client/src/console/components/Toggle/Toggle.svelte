<script lang="ts">
  import type { ToggleProps } from "./Toggle.types";

  let { checked = false, label, disabled = false, onchange }: ToggleProps = $props();

  function flip() {
    onchange?.(!checked);
  }
</script>

<button
  class="tc-toggle"
  class:is-on={checked}
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={flip}
></button>

<style lang="scss">
  @use "./Toggle.scss";
</style>
