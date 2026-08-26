<script lang="ts">
  import type { Snippet } from "svelte";
  import type { TreeProps } from "./Tree.types";

  let { label, children }: TreeProps & { children?: Snippet } = $props();
</script>

<div class="tc-tree" role="list" aria-label={label}>
  {@render children?.()}
</div>

<style lang="scss">
  @use "./Tree.scss";
</style>
