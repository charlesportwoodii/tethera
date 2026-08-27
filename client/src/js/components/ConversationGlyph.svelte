<script lang="ts">
  import { StatusGlyph } from "$console";
  import type { GlyphState } from "$console/types/state";

  interface Props {
    state: GlyphState;
    size?: number;
    /** Painted behind the mark, to break a tree trunk that runs under it. */
    bg?: string;
  }

  let { state, size = 14, bg }: Props = $props();

  const LABELS: Record<string, string> = {
    working: "Working",
    idle: "Idle",
    done: "Done",
    offline: "Not answering",
  };

  // Blocked and stalled stay with the console's own marks: the angular wedge and
  // the pink disc are their vocabulary for "you are the blocker" and "this
  // stopped moving", and neither is one of the three states below.
  const own = $derived(state in LABELS);
</script>

<!--
  Three states, told apart the way somebody glancing at a list tells them apart:

  - moving, so the dot moves — a slow yellow pulse
  - reached and not moving, so the dot is filled — green, whether the session
    finished or is merely sitting there
  - not reached, so the dot is hollow — the same hollow ring ConnDot draws for a
    machine that did not answer, because it is saying the same thing

  Supplied through the `glyph` snippet rather than by changing StatusGlyph, which
  belongs to another team. Theirs draws `idle` as a grey hollow ring, which puts
  a live session and an unreachable one in the same shape.
-->
{#if own}
  <span
    class="mark is-{state}"
    style:width="{size}px"
    style:height="{size}px"
    style:background={bg}
    role="img"
    aria-label={LABELS[state]}
    data-state={state}
  >
    <i></i>
  </span>
{:else}
  <StatusGlyph {state} {size} {bg} />
{/if}

<style lang="scss">
  .mark {
    position: relative;
    flex: none;
    display: grid;
    place-items: center;
    line-height: 0;

    i {
      display: block;
      width: 64%;
      height: 64%;
      border-radius: 50%;
    }
  }

  .is-working i {
    background: var(--tc-working, #ffc14d);

    // Slow on purpose. A fast pulse reads as an alert; this is only saying that
    // something is still going, on a screen somebody glances at.
    animation: breathe 2.4s ease-in-out infinite;
  }

  .is-idle i,
  .is-done i {
    background: var(--tc-ok, #3ddc97);
  }

  // Hollow is the whole signal, and it is deliberately the same ring ConnDot
  // uses for a machine that did not answer. A dot under a quiet machine and the
  // dot on the machine's own row are reporting one fact between them.
  .is-offline i {
    background: transparent;
    border: 1.5px solid var(--tc-ink-3, #7c8594);
  }

  @keyframes breathe {
    0%,
    100% {
      opacity: 1;
      box-shadow: 0 0 0 0 color-mix(in srgb, var(--tc-working, #ffc14d) 55%, transparent);
    }

    50% {
      opacity: 0.55;
      box-shadow: 0 0 0 5px color-mix(in srgb, var(--tc-working, #ffc14d) 0%, transparent);
    }
  }

  // A pulse that cannot be turned off is a problem for anybody who asked for
  // less motion. The colour still separates it from the other two.
  @media (prefers-reduced-motion: reduce) {
    .is-working i {
      animation: none;
    }
  }
</style>
