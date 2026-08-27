<script lang="ts">
  import Icon from "$console/components/Icon/Icon.svelte";
  import { fileExtension, formatBytes } from "$console/lib/format";
  import type { FileCardProps } from "./FileCard.types";

  let { name, size, at = null, ondownload }: FileCardProps = $props();

  const ext = $derived(fileExtension(name));
  const sub = $derived([formatBytes(size), at].filter(Boolean).join(" · "));
</script>

<button class="tc-file" type="button" onclick={ondownload}>
  <span class="tc-file__ext" aria-hidden="true">{ext}</span>
  <span class="tc-file__meta">
    <span class="tc-file__name">{name}</span>
    <span class="tc-file__sub">{sub}</span>
  </span>
  <Icon name="download" />
</button>

<style lang="scss">
  @use "./FileCard.scss";
</style>
