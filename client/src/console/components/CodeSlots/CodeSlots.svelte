<script lang="ts">
  import type { CodeSlotsProps } from "./CodeSlots.types";

  let { value, length = 6, label = "Pairing code" }: CodeSlotsProps = $props();

  const digits = $derived(value.slice(0, length).split(""));
  const slots = $derived(
    Array.from({ length }, (_, i) => ({
      char: digits[i] ?? "",
      cursor: i === digits.length && digits.length < length,
    })),
  );
</script>

<div
  class="tc-code"
  role="group"
  aria-label={label}
  data-filled={digits.length}
  data-length={length}
>
  {#each slots as slot, i (i)}
    <span
      class="tc-code__slot"
      class:is-empty={!slot.char && !slot.cursor}
      class:is-cursor={slot.cursor}
    >
      {slot.char}
    </span>
  {/each}
</div>

<style lang="scss">
  @use "./CodeSlots.scss";
</style>
