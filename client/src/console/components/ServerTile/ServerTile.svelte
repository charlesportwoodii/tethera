<script lang="ts">
  import ConnDot from "$console/components/ConnDot/ConnDot.svelte";
  import Icon from "$console/components/Icon/Icon.svelte";
  import StatusStrip from "$console/components/StatusStrip/StatusStrip.svelte";
  import type { ServerTileProps } from "./ServerTile.types";

  let {
    label,
    os,
    arch,
    link,
    rttMs = null,
    lastSeen = null,
    refusal = null,
    states,
    summary,
    attention = false,
    glyph,
    onopen = null,
    onstart = null,
  }: ServerTileProps = $props();

  const quiet = $derived(link === "offline");
</script>

<div class="tc-tile" class:is-quiet={quiet} data-attention={attention ? "true" : "false"}>
  <!-- Under the content rather than around it. A button wrapping the tile could
       not hold the + button inside it, and a target drawn round the title alone
       leaves most of a 250px card dead — which reads as a card that does not
       work rather than one that is not meant to be pressed. -->
  <button
    class="tc-tile__open"
    type="button"
    aria-label="Open {label}"
    onclick={() => onopen?.()}
  ></button>

  <div class="tc-tile__head">
    <div class="tc-tile__id">
      <span class="tc-tile__name">{label}</span>

      <!-- One line. What it is and how it is reached are four facts of two words
           each, and stacking them turns a tile that should be scannable at fleet
           height into three rows of mono.

           A refusal replaces the route rather than sitting beside it. Both at
           once reads as a machine that is unreachable and rude, which is one
           fact too many and the wrong one. -->
      <span class="tc-tile__line">
        <span class="tc-tile__meta">{os} · {arch}</span>

        {#if refusal}
          <span class="tc-tile__refusal">{refusal}</span>
        {:else}
          <ConnDot {link} {rttMs} {lastSeen} />
        {/if}
      </span>
    </div>

    <!-- Per machine, on the machine's own tile. A single button at the foot
         would belong to whichever machine you last thought about, which is not
         a question this screen can answer. -->
    <button
      class="tc-tile__start"
      type="button"
      aria-label="New session on {label}"
      onclick={() => onstart?.()}
    >
      <Icon name="plus" size={18} />
    </button>
  </div>

  {#if states.length > 0}
    <div class="tc-tile__strip">
      <StatusStrip {states} {glyph} />
    </div>
    <p class="tc-tile__say">{summary}</p>
  {:else}
    <p class="tc-tile__say tc-tile__say--empty">Nothing running</p>
  {/if}
</div>

<style lang="scss">
  @use "./ServerTile.scss";
</style>
