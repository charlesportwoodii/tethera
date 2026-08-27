<script lang="ts">
  import { runStyle } from "$console/lib/terminal-color";
  import type { TerminalViewProps } from "./TerminalView.types";

  let { grid, revision = 0, label = "Terminal", onfocus = null }: TerminalViewProps = $props();

  // The grid mutates in place, so revision is the only thing that changes when a
  // frame lands. Reading it here is what makes the repaint happen.
  const rows = $derived.by(() => {
    void revision;
    return grid.lines().map((runs) => runs.map((run) => ({ run, ...runStyle(run.style) })));
  });

  const cursor = $derived.by(() => {
    void revision;
    return grid.cursor;
  });

  const closed = $derived.by(() => {
    void revision;
    return grid.closed;
  });

  const CLOSED_REASON: Record<string, string> = {
    exited: "The program exited.",
    pane_gone: "The pane is gone.",
    server_shutdown: "The machine stopped answering.",
  };
</script>

<!--
  A log is not interactive, so a tabindex on it is normally a mistake. Here it is
  the point: a pane has to take focus for a hardware keyboard to reach the
  machine. role="application" would fix the lint and silence a screen reader on
  the output, which is worse.

  Focus rather than click is the signal: a tap, a Tab key and a hardware keyboard
  arriving all mean the same thing to the host — raise the keyboard for this pane.
-->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  class="tc-term"
  role="log"
  aria-label={label}
  tabindex="0"
  data-cols={grid.cols}
  data-rows={grid.rows}
  data-alt={grid.altScreen}
  onfocusin={() => onfocus?.()}
>
  {#each rows as row, y (y)}
    <div class="tc-term__row">
      {#each row as piece, i (i)}<span
          class="tc-term__run {piece.classes}"
          style:--tc-run-fg={piece.fg}
          style:--tc-run-bg={piece.bg}>{piece.run.text}</span
        >{/each}{#if cursor !== null && cursor.visible && cursor.y === y}<span
          class="tc-term__cursor is-{cursor.shape}"
          style:left="{cursor.x}ch"
          aria-hidden="true"
        ></span>{/if}
    </div>
  {/each}

  {#if closed !== null}
    <div class="tc-term__closed" role="status">
      {CLOSED_REASON[closed] ?? "The pane closed."}
    </div>
  {/if}
</div>

<style lang="scss">
  @use "./TerminalView.scss";
</style>
