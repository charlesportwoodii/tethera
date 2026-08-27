<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "$console/components/Icon/Icon.svelte";
  import type { EmptyStateProps } from "./EmptyState.types";

  let {
    icon = null,
    title,
    body = null,
    actions,
  }: EmptyStateProps & { actions?: Snippet } = $props();
</script>

<div class="tc-empty">
  {#if icon}
    <span class="tc-empty__mark" aria-hidden="true"><Icon name={icon} size={30} /></span>
  {/if}
  <h3 class="tc-empty__title">{title}</h3>
  {#if body}
    <p class="tc-empty__body">{body}</p>
  {/if}
  {#if actions}
    <div class="tc-empty__actions">{@render actions()}</div>
  {/if}
</div>

<style lang="scss">
  @use "./EmptyState.scss";
</style>
