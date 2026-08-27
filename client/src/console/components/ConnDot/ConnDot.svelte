<script lang="ts">
  import type { LinkKind } from "$bindings/LinkKind";
  import type { ConnDotProps } from "./ConnDot.types";

  let { link, rttMs = null, lastSeen = null, note = null }: ConnDotProps = $props();

  // "unknown" is a settled connection whose path has not been classified yet, so
  // it reads as reachable — which it is — without claiming to know how.
  const ROUTE: Record<LinkKind, string> = {
    direct: "direct",
    relayed: "via relay",
    unknown: "connected",
    offline: "no route",
  };

  const offline = $derived(link === "offline");

  const figure = $derived(offline ? lastSeen : rttMs === null ? null : rttMs + " ms");

  const text = $derived([ROUTE[link] ?? link, figure, note].filter(Boolean).join(" · "));
</script>

<div class="tc-conn" class:is-offline={offline} data-link={link}>
  <i aria-hidden="true"></i>
  <span>{text}</span>
</div>

<style lang="scss">
  @use "./ConnDot.scss";
</style>
