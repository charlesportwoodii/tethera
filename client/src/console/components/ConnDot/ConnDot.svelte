<script lang="ts">
  import type { ConnDotProps } from "./ConnDot.types";

  let { link, rttMs = null, lastSeen = null, note = null }: ConnDotProps = $props();

  const ROUTE: Record<string, string> = {
    direct: "direct",
    relayed: "via relay",
    offline: "no route",
  };

  const figure = $derived(
    link === "offline" ? lastSeen : rttMs === null ? null : rttMs + " ms",
  );

  const text = $derived(
    [ROUTE[link] ?? link, figure, note].filter(Boolean).join(" · "),
  );
</script>

<div class="tc-conn" class:is-offline={link === "offline"} data-link={link}>
  <i aria-hidden="true"></i>
  <span>{text}</span>
</div>

<style lang="scss">
  @use "./ConnDot.scss";
</style>
