<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "$console/components/Icon/Icon.svelte";
  import { fileExtension, formatBytes } from "$console/lib/format";
  import type { FileViewerProps } from "./FileViewer.types";

  let {
    file,
    anchor = "sheet",
    tabs = [],
    activeTab,
    noPreviewReason = null,
    onselecttab,
    onclose,
    children,
    actions,
  }: FileViewerProps & { children?: Snippet; actions?: Snippet } = $props();

  const subtitle = $derived(
    [formatBytes(file.size), file.mime ?? file.preview, file.at].filter(Boolean).join(" · "),
  );

  // Escape closes. No focus trap here on purpose: trapping focus needs to know
  // what it is trapping against, which is the host app's business.
  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") onclose?.();
  }
</script>

<svelte:window {onkeydown} />

<div
  class="tc-fv is-{anchor}"
  role="dialog"
  aria-modal="true"
  aria-label={file.name}
  data-anchor={anchor}
>
  {#if anchor === "sheet"}
    <span class="tc-fv__grip" aria-hidden="true"></span>
  {/if}

  <div class="tc-fv__head">
    <span class="tc-fv__ext" aria-hidden="true">{fileExtension(file.name)}</span>
    <span class="tc-fv__meta"><b>{file.name}</b><span>{subtitle}</span></span>
    <button class="tc-fv__close" type="button" aria-label="Close" onclick={onclose}>
      <Icon name="close" />
    </button>
  </div>

  {#if tabs.length > 0}
    <div class="tc-fv__tabs" role="tablist" aria-label="View">
      {#each tabs as tab (tab)}
        <button
          class="tc-fv__tab"
          class:is-active={tab === activeTab}
          type="button"
          role="tab"
          aria-selected={tab === activeTab}
          onclick={() => onselecttab?.(tab)}
        >
          {tab}
        </button>
      {/each}
    </div>
  {/if}

  <div class="tc-fv__body">
    {#if noPreviewReason}
      <div class="tc-fv__none">
        <Icon name="terminal" size={30} />
        <p>{noPreviewReason}</p>
      </div>
    {:else}
      {@render children?.()}
    {/if}
  </div>

  {#if actions}
    <div class="tc-fv__foot">{@render actions()}</div>
  {/if}
</div>

<style lang="scss">
  @use "./FileViewer.scss";
</style>
