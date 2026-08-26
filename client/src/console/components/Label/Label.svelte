<script lang="ts">
  import type { Snippet } from "svelte";
  import type { LabelProps } from "./Label.types";

  let { kind = "section", flush = false, children }: LabelProps & { children?: Snippet } =
    $props();
</script>

<span class="tc-label is-{kind}" class:is-flush={flush} data-kind={kind}>
  {@render children?.()}
</span>

<style lang="scss">
  @use "./Label.scss";
</style>
