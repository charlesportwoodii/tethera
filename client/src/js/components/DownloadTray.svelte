<script lang="ts">
  import { Parts } from "$managers/parts";
  import { DownloadManager, type Download } from "$managers/download_manager";

  interface Props {
    rows: Download[];
    oncancel?: (id: string) => void;
    ondismiss?: (id: string) => void;
  }

  let { rows, oncancel, ondismiss }: Props = $props();

  /**
   * What a row says under its name.
   *
   * `paused` is the line that matters. A phone that switches apps loses the
   * connection under a transfer that was working, and the bytes stay on disk -
   * so the honest report is that it is coming back, not that it failed. A
   * person told "failed" starts the download again, which is the one action
   * that throws those bytes away.
   */
  function saying(row: Download): string {
    switch (row.state) {
      case "opening":
        return "asking the machine…";
      case "running":
        return row.total > 0
          ? `${Parts.size(row.received)} of ${Parts.size(row.total)}`
          : `${Parts.size(row.received)} so far`;
      case "paused":
        return `paused at ${Parts.size(row.received)} — asking again`;
      case "done":
        return row.savedTo ? `saved to ${row.savedTo}` : "saved";
      case "cancelled":
        return `stopped — ${Parts.size(row.received)} kept, ask again to carry on`;
      case "failed":
        return row.failure ?? "that download did not finish";
    }
  }
</script>

{#if rows.length > 0}
  <div class="tray" role="status" aria-live="polite">
    {#each rows as row (row.id)}
      {@const fraction = DownloadManager.fraction(row)}
      <div class="row" class:bad={row.state === "failed"}>
        <div class="top">
          <span class="name">{row.name}</span>

          {#if DownloadManager.settled(row.state)}
            <button
              class="act"
              type="button"
              aria-label={`Dismiss ${row.name}`}
              onclick={() => ondismiss?.(row.id)}>×</button
            >
          {:else}
            <button
              class="act"
              type="button"
              aria-label={`Stop downloading ${row.name}`}
              onclick={() => oncancel?.(row.id)}>stop</button
            >
          {/if}
        </div>

        {#if !DownloadManager.settled(row.state)}
          <!--
            An indeterminate bar until the machine says how big the file is. It
            hashes the whole asset before it answers, and on a large one that is
            most of a second - a bar pinned at zero for that long says nothing
            is happening, which is the opposite of what is true.
          -->
          <div
            class="bar"
            class:waiting={fraction === null}
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={fraction === null ? undefined : 1}
            aria-valuenow={fraction ?? undefined}
          >
            <div class="fill" style={fraction === null ? "" : `width: ${fraction * 100}%`}></div>
          </div>
        {/if}

        <span class="say">{saying(row)}</span>
      </div>
    {/each}
  </div>
{/if}

<style lang="scss">
  .tray {
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: var(--tc-rule);
    border-top: 1px solid var(--tc-rule);
  }

  .row {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 14px;
    background: var(--tc-surface);
  }

  .top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }

  .name {
    overflow: hidden;
    font-size: 13px;
    color: var(--tc-ink);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .act {
    flex: none;
    padding: 2px 6px;
    background: none;
    border: 0;
    color: var(--tc-ink-3);
    font-family: var(--tc-mono);
    font-size: 11px;
    cursor: pointer;

    &:hover {
      color: var(--tc-ink);
    }
  }

  .bar {
    position: relative;
    height: 3px;
    overflow: hidden;
    background: var(--tc-rule-2);
  }

  .fill {
    height: 100%;
    background: var(--tc-accent);
    transition: width 160ms linear;
  }

  // Nothing is known about how far along this is, so the stripe travels rather
  // than filling. A full-width bar here would claim a completeness nobody has
  // measured.
  .waiting .fill {
    width: 40%;
    animation: drift 1200ms ease-in-out infinite;
  }

  .say {
    overflow: hidden;
    color: var(--tc-ink-3);
    font-family: var(--tc-mono);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bad .say {
    color: var(--tc-attn);
  }

  @keyframes drift {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(250%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .waiting .fill {
      animation: none;
      width: 100%;
      opacity: 0.35;
    }

    .fill {
      transition: none;
    }
  }
</style>
