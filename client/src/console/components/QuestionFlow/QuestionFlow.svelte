<script lang="ts">
  import Button from "$console/components/Button/Button.svelte";
  import Icon from "$console/components/Icon/Icon.svelte";
  import StatusGlyph from "$console/components/StatusGlyph/StatusGlyph.svelte";
  import {
    EMPTY_DRAFT,
    isComplete,
    toAnswer,
    toAnswers,
    type Draft,
  } from "$console/types/questions";
  import type { Answer } from "$bindings/Answer";
  import type { QuestionFlowProps } from "./QuestionFlow.types";

  let {
    question,
    anchor = "sheet",
    waiting = null,
    autoSubmit = true,
    onsubmit,
    oncancel,
  }: QuestionFlowProps = $props();

  const asks = $derived(question.asks);

  // index === asks.length is the review step. One reply answers the whole set,
  // because the harness stays blocked until it has every answer.
  let index = $state(0);

  // A sparse map rather than an array seeded from the prop: seeding from a prop
  // captures its first value only, so a set swapped under the component would
  // keep the old answers.
  let drafts = $state<Record<number, Draft>>({});

  const reviewing = $derived(index >= asks.length);
  const current = $derived(reviewing ? null : asks[index]);
  const draft = $derived(reviewing ? EMPTY_DRAFT : (drafts[index] ?? EMPTY_DRAFT));
  const multi = $derived(current?.multi_select === true);
  const allowText = $derived(current?.allows_free_text === true);

  /** The synthetic "something else" row sits after the ask's own options. */
  const otherKey = $derived(current ? current.options.length + 1 : 0);

  const answered = $derived(current !== null && toAnswer(draft, multi) !== null);
  const complete = $derived(isComplete(asks, drafts));

  /**
   * One ask, one choice: selecting is the whole interaction.
   *
   * This is what the harness's own picker does — a lone single-select submits the
   * instant it is pressed, and a review screen appears only where there is more
   * than one answer to review. Permission prompts are the highest-frequency
   * question by an order of magnitude, and three taps to allow one command is
   * friction exactly where it is felt most.
   *
   * Choosing free text never takes this path: there would be nothing typed yet.
   */
  const fastPath = $derived(autoSubmit && asks.length === 1 && !multi);

  function replace(next: Draft) {
    drafts = { ...drafts, [index]: next };
  }

  function choose(option: number) {
    if (multi) {
      const has = draft.selected.includes(option);
      replace({
        ...draft,
        selected: has
          ? draft.selected.filter((s) => s !== option)
          : [...draft.selected, option].sort((a, b) => a - b),
      });
      return;
    }
    // Single select: picking an option also abandons a free-text answer, because
    // the wire has one Answer per ask and both cannot be it.
    const next: Draft = { selected: [option], text: null };
    replace(next);

    if (fastPath) {
      // Built from the value rather than read back from state: the write above
      // has not settled yet, and submitting a stale draft would send nothing.
      send([toAnswer(next, false)]);
    }
  }

  function chooseText() {
    if (draft.text !== null) {
      replace({ ...draft, text: null });
      return;
    }
    replace({ selected: multi ? draft.selected : [], text: "" });
  }

  function typeText(value: string) {
    replace({ ...draft, text: value });
  }

  function next() {
    if (!answered) return;
    index = Math.min(index + 1, asks.length);
  }

  function back() {
    index = Math.max(index - 1, 0);
  }

  function submit() {
    if (!complete) return;
    send(toAnswers(asks, drafts));
  }

  function send(answers: Array<Answer | null>) {
    // The set's own fingerprint, once. It belongs to the set rather than to any
    // one ask, because the set is what gets answered.
    onsubmit?.(answers, question.fingerprint);
  }

  function summarise(i: number): string {
    const d = drafts[i] ?? EMPTY_DRAFT;
    const labels = d.selected.map((s) => asks[i].options[s]?.label).filter(Boolean);
    const own = d.text !== null && d.text.trim() !== "" ? d.text.trim() : null;
    return [...labels, own].filter(Boolean).join(", ") || "Not answered";
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      oncancel?.();
      return;
    }
    if (reviewing || !current) return;
    // Number keys pick an option, which is also how the numbered menu on the
    // machine is answered.
    const digit = Number.parseInt(event.key, 10);
    if (Number.isNaN(digit) || digit < 1) return;
    if (digit <= current.options.length) {
      choose(digit - 1);
    } else if (allowText && digit === otherKey) {
      chooseText();
    }
  }
</script>

<svelte:window {onkeydown} />

<div
  class="tc-qf is-{anchor}"
  role="dialog"
  aria-modal="true"
  aria-label={reviewing ? "Review your answers" : (current?.prompt ?? "Question")}
  data-step={reviewing ? "review" : index}
>
  <div class="tc-qf__head">
    <span class="tc-qf__who" class:is-review={reviewing}>
      {#if !reviewing}
        <StatusGlyph state="blocked" size={11} bg="var(--tc-surface)" />
        Claude is asking{waiting ? " · " + waiting : ""}
      {:else}
        Before you send
      {/if}
    </span>
    <button class="tc-qf__close" type="button" aria-label="Cancel" onclick={oncancel}>
      <Icon name="close" size={13} />
    </button>
  </div>

  <div class="tc-qf__prog">
    {#each asks as _ask, i (i)}
      <span
        class="tc-qf__pip"
        class:is-done={i < index}
        class:is-now={i === index && !reviewing}
      ></span>
    {/each}
    <span class="tc-qf__of">
      {reviewing ? asks.length + " answered" : index + 1 + " of " + asks.length}
    </span>
  </div>

  <div class="tc-qf__body">
    {#if reviewing}
      <div class="tc-qf__rev">
        {#each asks as ask, i (i)}
          <div class="tc-qf__item">
            <span class="k">{ask.header ?? "Question " + (i + 1)}</span>
            <span class="q">{ask.prompt}</span>
            <span class="a">
              <b>{summarise(i)}</b>
              <button type="button" onclick={() => (index = i)}>edit</button>
            </span>
          </div>
        {/each}
      </div>
      <p class="tc-qf__note">
        One reply answers the whole set, carrying its fingerprint.<br />
        Refused if the pane has moved on.
      </p>
    {:else if current}
      {#if current.header}
        <span class="tc-qf__chip">{current.header}</span>
      {/if}
      <p class="tc-qf__q">{current.prompt}</p>
      <p class="tc-qf__hint">{multi ? "pick any that apply" : "pick one"}</p>
      <div class="tc-qf__opts" role={multi ? "group" : "radiogroup"} aria-label={current.prompt}>
        {#each current.options as option, i (i)}
          <button
            class="tc-qf__opt"
            class:is-multi={multi}
            class:is-on={draft.selected.includes(i)}
            type="button"
            role={multi ? "checkbox" : "radio"}
            aria-checked={draft.selected.includes(i)}
            onclick={() => choose(i)}
          >
            <span class="tc-qf__mark"><Icon name="check" size={12} /></span>
            <span class="tc-qf__tx">
              <b>{option.label}</b>
              {#if option.description}<span>{option.description}</span>{/if}
            </span>
            <span class="tc-qf__key" aria-hidden="true">{i + 1}</span>
          </button>
        {/each}
        {#if allowText}
          <button
            class="tc-qf__opt"
            class:is-multi={multi}
            class:is-on={draft.text !== null}
            type="button"
            role={multi ? "checkbox" : "radio"}
            aria-checked={draft.text !== null}
            onclick={chooseText}
          >
            <span class="tc-qf__mark"><Icon name="check" size={12} /></span>
            <span class="tc-qf__tx"><b>Something else</b><span>Type your own answer.</span></span>
            <span class="tc-qf__key" aria-hidden="true">{otherKey}</span>
          </button>
        {/if}
      </div>
      {#if draft.text !== null}
        <div class="tc-qf__other">
          <p class="tc-qf__hint" style="margin:0">your own answer</p>
          <textarea
            class="tc-qf__field"
            aria-label="Your own answer"
            placeholder="Type your answer"
            value={draft.text}
            oninput={(e) => typeText(e.currentTarget.value)}
          ></textarea>
        </div>
      {/if}
    {/if}
  </div>

  <div class="tc-qf__foot">
    <button class="tc-qf__back" type="button" disabled={index === 0} onclick={back}>back</button>
    <span class="tc-qf__next">
      {#if reviewing}
        <Button icon="send" disabled={!complete} onclick={submit}>
          Send {asks.length} answer{asks.length === 1 ? "" : "s"}
        </Button>
      {:else if fastPath}
        <!--
          Pressing an option is the send, so there is no button for one. Typing an
          answer of your own still needs one: nothing can tell when a sentence has
          finished being typed.
        -->
        {#if draft.text !== null}
          <Button icon="send" disabled={!answered} onclick={submit}>Send answer</Button>
        {/if}
      {:else}
        <Button disabled={!answered} onclick={next}>
          {index === asks.length - 1 ? "Review" : "Next question"}
        </Button>
      {/if}
    </span>
  </div>
</div>

<style lang="scss">
  @use "./QuestionFlow.scss";
</style>
