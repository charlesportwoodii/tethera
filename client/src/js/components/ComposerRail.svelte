<script lang="ts">
  import { onDestroy } from "svelte";
  import { MOD, type KeyCap } from "$console";
  import type { Key } from "$bindings/Key";
  import type { Mods } from "$bindings/Mods";

  interface Props {
    /** Where you are. The mode key draws the other one. */
    mode: "chat" | "terminal";
    onmode?: () => void;
    /**
     * Opens the floorplan. Absent when this machine reports no pane geometry,
     * which is also when `WorkspaceMap` draws nothing.
     */
    onmap?: (() => void) | null;
    /** Panes in this tab, on the floorplan key. Absent below two — a badge reading 1 says nothing. */
    mapBadge?: number | null;
    /** Terminal mode only. Absent when the machine will not take input. */
    onkey?: ((key: Key, mods: Mods) => void) | null;
    /**
     * Pulls the desk to the workspace and tab this session runs in.
     *
     * Chat only: the terminal already moves the desk when a tab is tapped.
     * Absent when the machine will not move its own focus, and when this
     * session is not running in a pane — there is nothing to focus in either
     * case.
     */
    onfocus?: (() => Promise<void>) | null;
  }

  let {
    mode,
    onmode,
    onmap = null,
    mapBadge = null,
    onkey = null,
    onfocus = null,
  }: Props = $props();

  /**
   * How long the focus key stays confirmed.
   *
   * This press changes a screen somebody is not looking at, so the button is
   * the only place the result can appear. Without it a press that worked and a
   * press that did nothing are the same experience.
   */
  const CONFIRMED_MS = 1400;

  let confirmed = $state(false);
  let clearing: ReturnType<typeof setTimeout> | null = null;

  onDestroy(() => {
    if (clearing) {
      clearTimeout(clearing);
    }
  });

  async function pullDesk(): Promise<void> {
    if (!onfocus) {
      return;
    }

    await onfocus();

    confirmed = true;

    if (clearing) {
      clearTimeout(clearing);
    }

    clearing = setTimeout(() => {
      confirmed = false;
      clearing = null;
    }, CONFIRMED_MS);
  }

  /**
   * The control keys, in one row rather than the two `KeyBar` uses.
   *
   * `KeyBar`'s own rows are laid out for a bar that owns the bottom of the
   * screen. Here the strip is one line above the composer and scrolls
   * sideways, so the order is by reach: the interrupts a thumb goes for first,
   * then the escapes, then the arrows.
   */
  const KEYS: KeyCap[] = [
    { label: "^C", key: { char: "c" }, mods: MOD.ctrl },
    { label: "^D", key: { char: "d" }, mods: MOD.ctrl },
    { label: "^L", key: { char: "l" }, mods: MOD.ctrl },
    { label: "^Z", key: { char: "z" }, mods: MOD.ctrl },
    { label: "esc", key: "escape" },
    { label: "tab", key: "tab" },
    { label: "←", key: "left" },
    { label: "↑", key: "up" },
    { label: "↓", key: "down" },
    { label: "→", key: "right" },
  ];

  // The destination, not the location. A key labelled with where you already
  // are is a key that looks disabled.
  const going = $derived(mode === "chat" ? "terminal" : "chat");
  const badge = $derived(mapBadge !== null && mapBadge > 1 ? String(mapBadge) : null);
</script>

<div class="rail" class:term={mode === "terminal"}>
  <button
    class="mode"
    type="button"
    aria-label={going === "terminal" ? "Open the terminal" : "Back to chat"}
    onclick={() => onmode?.()}
  >
    {#if going === "terminal"}
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <rect x="3" y="4" width="18" height="16" rx="2" />
        <path d="M7 9l3 3-3 3M13 15h4" />
      </svg>
    {:else}
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.4A8 8 0 1 1 21 12z" />
      </svg>
    {/if}
  </button>

  {#if onmap}
    <span class="sep"></span>
    <button class="k" type="button" aria-label="Pane layout" onclick={() => onmap()}>
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <rect x="3" y="3" width="18" height="18" rx="2" />
        <path d="M10 3v18M10 13h11" />
      </svg>
      {#if badge}
        <span class="badge">{badge}</span>
      {/if}
    </button>
  {/if}

  {#if mode === "chat" && onfocus}
    <button
      class="k"
      class:done={confirmed}
      type="button"
      aria-label={confirmed ? "Show this session — done" : "Show this session at the desk"}
      onclick={() => void pullDesk()}
    >
      {#if confirmed}
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 12.5l5 5L20 6.5" /></svg>
      {:else}
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="12" cy="12" r="7" />
          <path d="M12 2v3M12 19v3M2 12h3M19 12h3" />
        </svg>
      {/if}
    </button>
  {/if}

  {#if mode === "terminal" && onkey}
    <span class="sep"></span>
    {#each KEYS as cap (cap.label)}
      <button
        class="k text"
        type="button"
        onclick={() => onkey(cap.key, cap.mods ?? MOD.none)}
      >
        {cap.label}
      </button>
    {/each}
  {/if}
</div>

<style lang="scss">
  // One line, scrolling sideways. Wrapping to a second line would move the
  // composer down by a row the moment a machine advertised one more key, and
  // the composer not moving is the point of putting the rail here.
  .rail {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
    padding: 7px 10px;
    overflow-x: auto;
    border-top: 1px solid var(--tc-rule);
    scrollbar-width: none;

    &::-webkit-scrollbar {
      display: none;
    }
  }

  .term {
    background: var(--tc-term-bg);
    border-top-color: var(--tc-term-chrome);

    .k {
      background: var(--tc-term-chrome);
      color: var(--tc-term-fg);
    }

    // Says the desk moved. The only place it can be said, because the thing that
  // changed is on a screen somebody is not looking at.
  .done {
    background: var(--tc-ok);
    color: var(--tc-accent-ink);
  }

  .badge {
      border-color: var(--tc-term-bg);
    }
  }

  button {
    flex: none;
    display: grid;
    place-items: center;
    height: 32px;
    min-width: 34px;
    margin: 0;
    padding: 0 9px;
    border: 0;
    border-radius: var(--tc-r-chip);
    font-family: var(--tc-mono);
    font-size: 11px;
  }

  .mode {
    padding: 0;
    background: var(--tc-accent);
    color: var(--tc-accent-ink);
  }

  .k {
    position: relative;
    background: var(--tc-surface-3);
    color: var(--tc-ink-2);
  }

  svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  // Says the desk moved. The only place it can be said, because the thing that
  // changed is on a screen somebody is not looking at.
  .done {
    background: var(--tc-ok);
    color: var(--tc-accent-ink);
  }

  .badge {
    position: absolute;
    top: -3px;
    right: -3px;
    display: grid;
    place-items: center;
    min-width: 15px;
    height: 15px;
    padding: 0 3px;
    border: 2px solid var(--tc-surface-2);
    border-radius: 8px;
    background: var(--tc-accent);
    color: var(--tc-accent-ink);
    font-size: 9px;
  }

  .sep {
    flex: none;
    width: 1px;
    height: 18px;
    margin: 0 2px;
    background: var(--tc-rule-2);
  }
</style>
