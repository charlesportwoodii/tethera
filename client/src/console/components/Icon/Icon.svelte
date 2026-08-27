<script lang="ts">
  import type { IconProps } from "./Icon.types";

  let { name, size = 16, label }: IconProps = $props();

  // Paths are inline rather than a sprite: a sprite needs a host element mounted
  // somewhere, which is exactly the integration assumption these components are
  // meant not to make.
  const PATHS: Record<string, string[]> = {
    back: ["M12.5 3.5 5.5 10l7 6.5"],
    plus: ["M10 3.5v13M3.5 10h13"],
    terminal: ["M2.5 5.5a2 2 0 0 1 2-2h11a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-11a2 2 0 0 1-2-2z", "M6 9l2.4 2-2.4 2M11 13h3.5"],
    send: ["M3.5 10h12M10.5 5l5 5-5 5"],
    attach: ["M14 8 7.5 14.5A3.1 3.1 0 0 1 3.1 10.1l6.6-6.6a2.2 2.2 0 0 1 3.1 3.1l-6.5 6.5a1.2 1.2 0 0 1-1.7-1.7l6-6"],
    chevron: ["M7.5 6l4.5 4-4.5 4"],
    collapse: ["M5.5 12 10 7.5l4.5 4.5"],
    expand: ["M5.5 8 10 12.5 14.5 8"],
    grip: ["M4 8.5h12M4 11.5h12"],
    scan: ["M4 7.5V6a2 2 0 0 1 2-2h1.5M12.5 4H14a2 2 0 0 1 2 2v1.5M16 12.5V14a2 2 0 0 1-2 2h-1.5M7.5 16H6a2 2 0 0 1-2-2v-1.5", "M4 10h12"],
    settings: ["M10 3v2M10 15v2M4 6.6l1.8 1M14.2 12.4l1.8 1M4 13.4l1.8-1M14.2 7.6l1.8-1"],
    retry: ["M16 10a6 6 0 1 1-1.8-4.3", "M16 3.5V7h-3.5"],
    download: ["M10 3.5v9M6 9l4 4 4-4M3.5 16.5h13"],
    check: ["M4.5 10.5l3.5 3.5 7.5-8"],
    close: ["M5.5 5.5l9 9M14.5 5.5l-9 9"],
  };

  const paths = $derived(PATHS[name] ?? []);
</script>

<svg
  class="tc-icon"
  width={size}
  height={size}
  viewBox="0 0 20 20"
  role={label ? "img" : "presentation"}
  aria-label={label}
  aria-hidden={label ? undefined : "true"}
>
  {#if name === "settings"}
    <circle cx="10" cy="10" r="2.6" />
  {/if}
  {#each paths as d (d)}
    <path {d} />
  {/each}
</svg>

<style lang="scss">
  @use "./Icon.scss";
</style>
