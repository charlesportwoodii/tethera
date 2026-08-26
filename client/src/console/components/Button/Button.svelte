<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "$console/components/Icon/Icon.svelte";
  import type { ButtonProps } from "./Button.types";

  let {
    variant = "primary",
    icon,
    disabled = false,
    type = "button",
    onclick,
    children,
  }: ButtonProps & { children?: Snippet } = $props();
</script>

<button
  class="tc-button"
  class:is-quiet={variant === "quiet"}
  data-variant={variant}
  {type}
  {disabled}
  {onclick}
>
  {#if icon}
    <Icon name={icon} />
  {/if}
  {@render children?.()}
</button>

<style lang="scss">
  @use "./Button.scss";
</style>
