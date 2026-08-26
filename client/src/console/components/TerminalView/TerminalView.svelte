<script lang="ts">
  import type { TerminalViewProps } from "./TerminalView.types";

  let { lines, cursor = false, label = "Terminal" }: TerminalViewProps = $props();
</script>

<!--
  A log is not interactive, so a positive tabindex on it is normally a mistake.
  Here it is the point: a pane has to be able to take focus for a hardware
  keyboard to reach the machine. The alternative — role="application" — would
  silence a screen reader on the output, which is worse.
-->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="tc-term" role="log" aria-label={label} tabindex="0">
  {#each lines as line, i (i)}
    <div class="tc-term__line is-{line.tone ?? 'plain'}">{line.text}</div>
  {/each}
  {#if cursor}
    <div class="tc-term__line is-plain">
      <span class="tc-term__cursor" aria-hidden="true"></span>
    </div>
  {/if}
</div>

<style lang="scss">
  @use "./TerminalView.scss";
</style>
