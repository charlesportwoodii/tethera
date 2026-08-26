<script lang="ts">
  import { DEFAULT_KEYS, type KeyBarProps } from "./KeyBar.types";

  let { rows = DEFAULT_KEYS, onkey }: KeyBarProps = $props();
</script>

<div class="tc-keys" role="group" aria-label="Terminal keys">
  {#each rows as row, r (r)}
    <div class="tc-keys__row">
      {#each row as key (key)}
        <button class="tc-keys__key" type="button" onclick={() => onkey?.(key)}>
          {key}
        </button>
      {/each}
    </div>
  {/each}
</div>

<style lang="scss">
  @use "./KeyBar.scss";
</style>
