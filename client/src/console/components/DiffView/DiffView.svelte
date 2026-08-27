<script lang="ts">
  import Icon from "$console/components/Icon/Icon.svelte";
  import type { DiffViewProps } from "./DiffView.types";

  let { path, unified, added = null, removed = null, open = false, ontoggle }: DiffViewProps =
    $props();

  type Kind = "add" | "del" | "hunk" | "ctx";

  // Classified by first character, which is what a unified diff guarantees.
  // "+++" and "---" are file headers rather than changes, so they are hunks.
  function classify(line: string): Kind {
    if (line.startsWith("+++") || line.startsWith("---") || line.startsWith("@@")) return "hunk";
    if (line.startsWith("+")) return "add";
    if (line.startsWith("-")) return "del";
    return "ctx";
  }

  const lines = $derived(
    unified.split("\n").map((text) => ({ text, kind: classify(text) })),
  );
</script>

<div class="tc-diff">
  <button
    class="tc-diff__head"
    type="button"
    aria-expanded={open}
    onclick={ontoggle}
  >
    <Icon name="chevron" size={12} />
    <span class="tc-diff__path">{path}</span>
    {#if added !== null}
      <span class="tc-diff__count tc-diff__added">+{added}</span>
    {/if}
    {#if removed !== null}
      <span class="tc-diff__count tc-diff__removed">&minus;{removed}</span>
    {/if}
  </button>
  {#if open}
    <div class="tc-diff__body">
      {#each lines as line, i (i)}
        <div class="tc-diff__line is-{line.kind}">{line.text}</div>
      {/each}
    </div>
  {/if}
</div>

<style lang="scss">
  @use "./DiffView.scss";
</style>
