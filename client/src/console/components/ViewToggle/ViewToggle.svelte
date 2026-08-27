<script lang="ts">
  import StatusGlyph from "$console/components/StatusGlyph/StatusGlyph.svelte";
  import type { ViewToggleProps } from "./ViewToggle.types";

  let {
    view,
    chatAvailable = true,
    chatBadge = null,
    onchange,
  }: ViewToggleProps = $props();
</script>

<!--
  Two sides, never a third. If the workspace has no transcript the toggle does
  not render at all: the screen is a terminal, and offering a chat side that
  turns out to be empty is the failure the capability set exists to prevent.
-->
{#if chatAvailable}
  <div class="tc-view" role="tablist" aria-label="View">
    <button
      class="tc-view__side"
      class:is-on={view === "chat"}
      type="button"
      role="tab"
      aria-selected={view === "chat"}
      onclick={() => onchange?.("chat")}
    >
      Chat
      {#if chatBadge}
        <StatusGlyph
          state={chatBadge === "waiting" ? "blocked" : "working"}
          size={9}
          bg="var(--tc-surface-2)"
        />
      {/if}
    </button>
    <button
      class="tc-view__side"
      class:is-on={view === "terminal"}
      type="button"
      role="tab"
      aria-selected={view === "terminal"}
      onclick={() => onchange?.("terminal")}
    >
      Terminal
    </button>
  </div>
{/if}

<style lang="scss">
  @use "./ViewToggle.scss";
</style>
