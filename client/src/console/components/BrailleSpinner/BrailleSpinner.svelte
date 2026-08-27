<script lang="ts">
  import type { BrailleSpinnerProps } from "./BrailleSpinner.types";

  let { size = 15, offset = 0, interval = 90, label = "Working" }: BrailleSpinnerProps =
    $props();

  const FRAMES = ["\u280B", "\u2819", "\u2839", "\u2838", "\u283C", "\u2834", "\u2826", "\u2827", "\u2807", "\u280F"];

  // Read once. A viewer who changes the setting mid-session gets it on the next
  // mount, which is a better trade than a listener on every spinner.
  const reduced =
    typeof window !== "undefined" && typeof window.matchMedia === "function"
      ? window.matchMedia("(prefers-reduced-motion: reduce)").matches
      : false;

  let tick = $state(0);

  $effect(() => {
    if (reduced) return;
    const id = setInterval(() => {
      tick += 1;
    }, interval);
    return () => clearInterval(id);
  });

  const frame = $derived(FRAMES[(tick + offset) % FRAMES.length]);
</script>

<span
  class="tc-braille"
  style:font-size="{size}px"
  style:width="{size}px"
  role={label ? "img" : "presentation"}
  aria-label={label}
  aria-hidden={label ? undefined : "true"}
  data-static={reduced ? "true" : undefined}
>{frame}</span>

<style lang="scss">
  @use "./BrailleSpinner.scss";
</style>
