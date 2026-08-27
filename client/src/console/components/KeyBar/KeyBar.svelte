<script lang="ts">
  import { DEFAULT_KEYS, MOD, type KeyBarProps } from "./KeyBar.types";

  let { rows = DEFAULT_KEYS, onkey }: KeyBarProps = $props();
</script>

<div class="tc-keys" role="group" aria-label="Terminal keys">
  {#each rows as row, r (r)}
    <div class="tc-keys__row">
      {#each row as cap, c (c)}
        <button
          class="tc-keys__key"
          class:is-wide={cap.wide}
          type="button"
          onclick={() => onkey?.(cap.key, cap.mods ?? MOD.none)}
        >
          {cap.label}
        </button>
      {/each}
    </div>
  {/each}
</div>

<style lang="scss">
  @use "./KeyBar.scss";
</style>
