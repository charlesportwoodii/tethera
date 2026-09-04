<script lang="ts">
  import Icon from "$console/components/Icon/Icon.svelte";
  import type { SwipeRowProps } from "./SwipeRow.types";

  let {
    action,
    icon = "close",
    enabled = true,
    threshold = 0.3,
    onaction = null,
    children,
  }: SwipeRowProps = $props();

  let start = $state<number | null>(null);
  let offset = $state(0);
  let width = $state(0);

  const dragging = $derived(start !== null);
  const open = $derived(offset < -8);

  function begin(event: PointerEvent): void {
    if (!enabled) {
      return;
    }

    start = event.clientX;
    width = (event.currentTarget as HTMLElement).clientWidth;
  }

  function move(event: PointerEvent): void {
    if (start === null) {
      return;
    }

    // Leftward only, and never past the row's own width. A row that could be
    // dragged off screen leaves nothing to spring back to.
    offset = Math.max(-width, Math.min(0, event.clientX - start));
  }

  function end(): void {
    if (start === null) {
      return;
    }

    const travelled = width > 0 ? Math.abs(offset) / width : 0;

    start = null;
    offset = 0;

    if (travelled >= threshold) {
      onaction?.();
    }
  }
</script>

<div class="tc-swipe" data-open={open ? "true" : "false"}>
  {#if enabled}
    <div class="tc-swipe__bed" aria-hidden="true">
      <Icon name={icon} size={16} />
      <span>{action}</span>
    </div>
  {/if}

  <!--
    The track is not a control, so it takes no role. It is a container for
    whatever the caller put in the row, and those children carry their own
    controls; the drag is a shortcut for the button below, which is already in
    the DOM and already focusable.

    A role here would announce a second control that does the same thing as that
    button, which is worse for a screen reader than the warning is for anyone.
  -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="tc-swipe__track"
    class:is-settling={!dragging}
    style:transform="translateX({offset}px)"
    onpointerdown={begin}
    onpointermove={move}
    onpointerup={end}
    onpointercancel={end}
  >
    {@render children?.()}
  </div>

  <!-- The gesture is a shortcut, not the capability. Without this the action is
       unreachable by keyboard and invisible to a screen reader. -->
  {#if enabled}
    <button class="tc-swipe__act" type="button" onclick={() => onaction?.()}>{action}</button>
  {/if}
</div>

<style lang="scss">
  @use "./SwipeRow.scss";
</style>
