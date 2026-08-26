<script lang="ts">
  import type { Snippet } from "svelte";
  import type { TurnProps } from "./Turn.types";

  let { role, time, at = null, marked = false, children }: TurnProps & { children?: Snippet } =
    $props();

  const iso = $derived(at === null ? undefined : new Date(at).toISOString());
</script>

<div
  class="tc-turn"
  class:is-you={role === "you"}
  class:is-marked={marked}
  data-role={role}
>
  <time class="tc-turn__time" datetime={iso}>
    {time}
    {#if role === "you"}
      <em class="tc-turn__caret" aria-hidden="true">&#10095;</em>
    {/if}
  </time>
  <div class="tc-turn__body">
    {@render children?.()}
  </div>
</div>

<style lang="scss">
  @use "./Turn.scss";
</style>
