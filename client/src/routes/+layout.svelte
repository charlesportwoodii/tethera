<script lang="ts">
  import type { Snippet } from "svelte";
  import "../app.scss";

  let { children }: { children: Snippet } = $props();
</script>

{@render children()}
