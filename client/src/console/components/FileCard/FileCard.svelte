<script lang="ts">
  import Icon from "$console/components/Icon/Icon.svelte";
  import type { FileCardProps } from "./FileCard.types";

  let { name, size, at = null, ondownload }: FileCardProps = $props();

  // Binary units, because that is what a file manager on either desktop reports.
  function formatSize(bytes: number | bigint): string {
    const n = typeof bytes === "bigint" ? Number(bytes) : bytes;
    if (!Number.isFinite(n) || n < 0) return "unknown size";
    if (n < 1024) return n + " B";
    const units = ["KB", "MB", "GB", "TB"];
    let v = n / 1024;
    let i = 0;
    while (v >= 1024 && i < units.length - 1) {
      v /= 1024;
      i++;
    }
    return (v < 10 ? v.toFixed(1) : Math.round(v).toString()) + " " + units[i];
  }

  // split(".").pop() returns the whole name when there is no dot, so "Makefile"
  // would read as MAKE. A leading dot is not an extension either: .gitignore is
  // the name of the file, not a gitignore-typed thing.
  const ext = $derived.by(() => {
    const dot = name.lastIndexOf(".");
    if (dot <= 0 || dot === name.length - 1) return "FILE";
    return name.slice(dot + 1, dot + 5).toUpperCase();
  });
  const sub = $derived([formatSize(size), at].filter(Boolean).join(" · "));
</script>

<button class="tc-file" type="button" onclick={ondownload}>
  <span class="tc-file__ext" aria-hidden="true">{ext}</span>
  <span class="tc-file__meta">
    <span class="tc-file__name">{name}</span>
    <span class="tc-file__sub">{sub}</span>
  </span>
  <Icon name="download" />
</button>

<style lang="scss">
  @use "./FileCard.scss";
</style>
