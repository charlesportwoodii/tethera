<script lang="ts">
  import type { StatusGlyphProps } from "./StatusGlyph.types";

  let { state, size = 14, bg }: StatusGlyphProps = $props();

  // The wedge is sized in em so it tracks `size` without a second calculation.
  const LABELS: Record<string, string> = {
    working: "Working",
    idle: "Idle",
    done: "Done",
    blocked: "Waiting on you",
    offline: "Not answering",
    set: "Chosen",
    unset: "Not chosen",
  };
</script>

<span
  class="tc-glyph is-{state}"
  style:width="{size}px"
  style:height="{size}px"
  style:font-size="{size}px"
  style:background={bg}
  role="img"
  aria-label={LABELS[state] ?? state}
  data-state={state}
>
  <i></i>
</span>

<style lang="scss">
  @use "./StatusGlyph.scss";
</style>
