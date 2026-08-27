<script lang="ts">
  import Icon from "$console/components/Icon/Icon.svelte";
  import { fileExtension } from "$console/lib/format";
  import type { AttachChipProps } from "./AttachChip.types";

  let { name, progress = null, onremove = null }: AttachChipProps = $props();

  const uploading = $derived(typeof progress === "number");
  const percent = $derived(Math.round(Math.min(1, Math.max(0, progress ?? 0)) * 100));
</script>

<span class="tc-attach" data-uploading={uploading}>
  <span class="tc-attach__ext" aria-hidden="true">{fileExtension(name)}</span>
  <span class="tc-attach__name">{name}</span>
  {#if uploading}
    <span
      class="tc-attach__track"
      role="progressbar"
      aria-label="Uploading {name}"
      aria-valuemin="0"
      aria-valuemax="100"
      aria-valuenow={percent}
    >
      <span class="tc-attach__fill" style:width="{percent}%"></span>
    </span>
  {:else if onremove}
    <button class="tc-attach__remove" type="button" aria-label="Remove {name}" onclick={onremove}>
      <Icon name="close" size={12} />
    </button>
  {/if}
</span>

<style lang="scss">
  @use "./AttachChip.scss";
</style>
