<script lang="ts">
  import { MOD } from "$console";
  import type { Key } from "$bindings/Key";
  import type { Mods } from "$bindings/Mods";
  import type { KeyStreamProps } from "./KeyStream.types";

  let {
    onkey,
    ontext,
    disabled = false,
    placeholder = "type into the pane",
  }: KeyStreamProps = $props();

  /**
   * The keys a soft keyboard cannot produce, by their DOM name.
   *
   * Named rather than sent as text because the server encodes them per terminal
   * — `Tab` is one byte, `ArrowUp` is three or four depending on a mode the
   * server reads off the pane — and a client guessing at that is the injection
   * surface `TerminalInput` exists to close.
   */
  const NAMED: Record<string, Key> = {
    Enter: "enter",
    Escape: "escape",
    Tab: "tab",
    Backspace: "backspace",
    Delete: "delete",
    Insert: "insert",
    Home: "home",
    End: "end",
    PageUp: "page_up",
    PageDown: "page_down",
    ArrowUp: "up",
    ArrowDown: "down",
    ArrowLeft: "left",
    ArrowRight: "right",
  };

  /**
   * The field, kept permanently empty.
   *
   * A terminal has no submit, so there is nothing for a value to accumulate
   * towards. Text left in it would be sent a second time by anything that
   * flushed it, and would draw a phantom line over a pane that already received
   * every character.
   */
  let field: HTMLInputElement | null = null;

  function bits(event: KeyboardEvent): Mods {
    let mods = MOD.none;

    if (event.shiftKey) {
      mods |= MOD.shift;
    }

    if (event.altKey) {
      mods |= MOD.alt;
    }

    if (event.ctrlKey) {
      mods |= MOD.ctrl;
    }

    if (event.metaKey) {
      mods |= MOD.meta;
    }

    return mods;
  }

  /**
   * Whether this keypress is the browser's to act on.
   *
   * A modifier alone is not a keystroke, and forwarding one would send a bare
   * shift to the shell every time somebody reached for a capital.
   */
  function modifier(name: string): boolean {
    return ["Shift", "Alt", "Control", "Meta", "AltGraph", "CapsLock"].includes(name);
  }

  function press(event: KeyboardEvent): void {
    if (disabled || modifier(event.key)) {
      return;
    }

    const named = NAMED[event.key];

    if (named !== undefined) {
      // Prevented, or the browser acts on it itself: Tab leaves the field and
      // Backspace can navigate.
      event.preventDefault();
      onkey(named, bits(event));

      return;
    }

    const fn = /^F(\d{1,2})$/.exec(event.key);

    if (fn !== null) {
      event.preventDefault();
      onkey({ f: Number(fn[1]) }, bits(event));

      return;
    }

    if (event.key.length !== 1) {
      return;
    }

    // A modified character is a chord the server encodes; an unmodified one is
    // just text, and letting `oninput` carry it is what makes an IME and a
    // soft keyboard work at all.
    const chord = bits(event) & ~MOD.shift;

    if (chord !== MOD.none) {
      event.preventDefault();
      onkey({ char: event.key }, bits(event));
    }
  }

  /**
   * Printable text, forwarded and then cleared.
   *
   * `oninput` rather than `keydown` for the unmodified case, because that is the
   * only event a soft keyboard, an IME, or a paste reliably produces.
   */
  function typed(): void {
    if (field === null) {
      return;
    }

    const text = field.value;
    field.value = "";

    if (disabled || text.length === 0) {
      return;
    }

    ontext(text);
  }
</script>

<input
  bind:this={field}
  class="tc-keystream"
  type="text"
  {placeholder}
  aria-label="Type into the pane"
  autocapitalize="off"
  autocorrect="off"
  autocomplete="off"
  spellcheck="false"
  enterkeyhint="send"
  {disabled}
  onkeydown={press}
  oninput={typed}
/>

<style lang="scss">
  @use "./KeyStream.scss";
</style>
