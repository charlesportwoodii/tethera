<script lang="ts">
  import StatusGlyph from "$console/components/StatusGlyph/StatusGlyph.svelte";
  import type { QuestionCardProps } from "./QuestionCard.types";

  let { question, waiting = null, live = true, onopen = null }: QuestionCardProps = $props();

  const asks = $derived(question.asks);
  const single = $derived(asks.length === 1 ? asks[0] : null);
</script>

<!--
  Nothing here answers. There are no numbered rows and nothing tappable except
  the way in, so the only surface that can send an answer is the flow.
-->
<div
  class="tc-qcard"
  class:is-history={!live}
  role="group"
  aria-label={single ? single.prompt : asks.length + " questions"}
  data-live={live}
>
  <div class="tc-qcard__head">
    {#if live}
      <StatusGlyph state="blocked" size={11} bg="var(--tc-surface-2)" />
      <span>waiting on you{waiting ? " · " + waiting : ""}</span>
    {:else}
      <span>no longer waiting</span>
    {/if}
  </div>

  <div class="tc-qcard__body">
    {#if single}
      <p class="tc-qcard__prompt">{single.prompt}</p>
    {:else}
      <p class="tc-qcard__prompt">
        {asks.length} questions before it goes further.
      </p>
      <ul class="tc-qcard__list">
        {#each asks as ask, i (i)}
          <li><em>{i + 1}</em><span>{ask.prompt}</span></li>
        {/each}
      </ul>
    {/if}

    {#if live && onopen}
      <button class="tc-qcard__open" type="button" onclick={onopen}>
        Answer{asks.length > 1 ? " " + asks.length + " questions" : ""}
      </button>
    {/if}
  </div>
</div>

<style lang="scss">
  @use "./QuestionCard.scss";
</style>
