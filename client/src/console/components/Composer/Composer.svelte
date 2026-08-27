<script lang="ts">
  import AttachChip from "$console/components/AttachChip/AttachChip.svelte";
  import Icon from "$console/components/Icon/Icon.svelte";
  import { autogrow } from "$console/lib/autogrow";
  import type { ComposerProps } from "./Composer.types";

  let {
    value = "",
    placeholder,
    disabled = false,
    busy = false,
    maxRows = 5,
    attachments = [],
    onattach = null,
    onremoveattachment,
    oninput,
    onsend,
  }: ComposerProps = $props();

  // Controlled by the caller. Local state here would fight whatever arrives from
  // the gateway on the next frame.
  const empty = $derived(value.trim().length === 0);
  const uploading = $derived(attachments.some((a) => typeof a.progress === "number"));
  const blocked = $derived(disabled || busy || uploading);

  const hint = $derived(
    placeholder ??
      (busy ? "Reply — it will queue until the agent stops" : "reply, or 1-3 to answer"),
  );

  function send() {
    if (empty || blocked) return;
    onsend?.(value);
  }

  function onkeydown(event: KeyboardEvent) {
    // Enter inserts a newline. The button sends.
    //
    // The opposite mapping is conventional in a chat box and wrong here: a soft
    // keyboard has no Shift+Enter, so Enter-sends does not merely prefer one
    // action, it makes a newline unreachable — in a field built to grow to five
    // lines and wrap. The send button is large and already under the thumb.
    //
    // A hardware keyboard keeps a shortcut, because reaching for a button with a
    // mouse mid-sentence is its own annoyance.
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      send();
    }
  }
</script>

<div class="tc-composer" data-busy={busy} data-uploading={uploading}>
  {#if attachments.length > 0}
    <div class="tc-composer__attached">
      {#each attachments as file (file.id)}
        <AttachChip
          name={file.name}
          progress={file.progress ?? null}
          onremove={onremoveattachment ? () => onremoveattachment(file.id) : null}
        />
      {/each}
    </div>
  {/if}

  <div class="tc-composer__row">
    {#if onattach}
      <button
        class="tc-composer__attach"
        class:is-on={attachments.length > 0}
        type="button"
        aria-label="Attach a file"
        onclick={onattach}
      >
        <Icon name="attach" />
      </button>
    {/if}
    <!--
      A textarea rather than an input: a single-line field has nowhere to put a
      second line, so a long message scrolls sideways and the start of the
      sentence leaves the screen. It grows to `maxRows` and then scrolls itself.
      It stays editable while busy — going read-only mid-turn would lose whatever
      was half-typed when the agent started working.
    -->
    <textarea
      class="tc-composer__field"
      rows="1"
      aria-label="Message"
      placeholder={hint}
      {disabled}
      {value}
      use:autogrow={maxRows}
      oninput={(e) => oninput?.(e.currentTarget.value)}
      {onkeydown}
    ></textarea>
    <button
      class="tc-composer__send"
      type="button"
      aria-label="Send"
      disabled={blocked || empty}
      onclick={send}
    >
      <Icon name="send" />
    </button>
  </div>
</div>

<style lang="scss">
  @use "./Composer.scss";
</style>
