<script lang="ts">
  import Icon from "$console/components/Icon/Icon.svelte";
  import type { ComposerProps } from "./Composer.types";

  let {
    value = "",
    placeholder = "reply, or 1-3 to answer",
    disabled = false,
    onattach = null,
    oninput,
    onsend,
  }: ComposerProps = $props();

  // Controlled by the caller. Local state here would fight whatever arrives from
  // the gateway on the next frame.
  const empty = $derived(value.trim().length === 0);

  function send() {
    if (empty || disabled) return;
    onsend?.(value);
  }

  function onkeydown(event: KeyboardEvent) {
    // Enter sends. A phone keyboard has no other obvious verb, and the field is
    // one line by design — a long prompt is a job for the machine, not the walk.
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      send();
    }
  }
</script>

<div class="tc-composer">
  {#if onattach}
    <button class="tc-composer__attach" type="button" aria-label="Attach a file" onclick={onattach}>
      <Icon name="attach" />
    </button>
  {/if}
  <input
    class="tc-composer__field"
    type="text"
    aria-label="Message"
    {placeholder}
    {disabled}
    {value}
    oninput={(e) => oninput?.(e.currentTarget.value)}
    {onkeydown}
  />
  <button
    class="tc-composer__send"
    type="button"
    aria-label="Send"
    disabled={disabled || empty}
    onclick={send}
  >
    <Icon name="send" />
  </button>
</div>

<style lang="scss">
  @use "./Composer.scss";
</style>
