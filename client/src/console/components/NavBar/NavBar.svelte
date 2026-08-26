<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "$console/components/Icon/Icon.svelte";
  import type { NavBarProps } from "./NavBar.types";

  let {
    title,
    subtitle = null,
    onback = null,
    backLabel = "Back",
    actions,
  }: NavBarProps & { actions?: Snippet } = $props();
</script>

<header class="tc-nav">
  {#if onback}
    <button class="tc-nav__back" type="button" aria-label={backLabel} onclick={onback}>
      <Icon name="back" />
    </button>
  {/if}
  <div class="tc-nav__text">
    <h1 class="tc-nav__title">{title}</h1>
    {#if subtitle}
      <span class="tc-nav__sub">{subtitle}</span>
    {/if}
  </div>
  {#if actions}
    <div class="tc-nav__actions">{@render actions()}</div>
  {/if}
</header>

<style lang="scss">
  @use "./NavBar.scss";
</style>
