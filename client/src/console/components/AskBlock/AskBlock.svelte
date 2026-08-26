<script lang="ts">
  import StatusGlyph from "$console/components/StatusGlyph/StatusGlyph.svelte";
  import type { AskBlockProps, AskOption } from "./AskBlock.types";

  let {
    prompt,
    options,
    waiting = null,
    fingerprint = null,
    onanswer,
  }: AskBlockProps = $props();

  const normalised = $derived<AskOption[]>(
    options.map((o) => (typeof o === "string" ? { label: o, detail: null } : o)),
  );
</script>

<div class="tc-ask" role="group" aria-label={prompt}>
  <div class="tc-ask__head">
    <StatusGlyph state="blocked" size={11} bg="var(--tc-surface-2)" />
    <span>waiting on you{waiting ? " · " + waiting : ""}</span>
  </div>
  <p class="tc-ask__prompt">{prompt}</p>
  {#each normalised as option, i (i)}
    <button
      class="tc-ask__option"
      type="button"
      onclick={() => onanswer?.(i, fingerprint)}
    >
      <span class="tc-ask__key" aria-hidden="true">{i + 1}</span>
      <span>
        <span class="tc-ask__label">{option.label}</span>
        {#if option.detail}
          <span class="tc-ask__detail"> {option.detail}</span>
        {/if}
      </span>
    </button>
  {/each}
</div>

<style lang="scss">
  @use "./AskBlock.scss";
</style>
