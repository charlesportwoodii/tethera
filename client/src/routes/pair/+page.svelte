<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { invoke } from "@tauri-apps/api/core";
  import { Button, CodeSlots, Label, NavBar } from "$console";
  import { PairingManager } from "$managers/pairing_manager";
  import { Refusal } from "$managers/refusal";

  const manager = new PairingManager(invoke);
  const pairing = manager.state;

  let typed = $state("");
  let pasted = $state("");
  let scanning = $state(false);
  let scanError = $state<string | null>(null);
  let slot: HTMLInputElement | null = $state(null);

  onMount(() => {
    const uri = page.url.searchParams.get("uri");

    if (uri) {
      void manager.begin(uri);
    }
  });

  // Leaving mid-scan would otherwise leave the camera running and the page
  // transparent, which reads as the app having disappeared.
  onDestroy(() => {
    void stopScan();
  });

  async function scan(): Promise<void> {
    scanError = null;

    const scanner = await import("@tauri-apps/plugin-barcode-scanner");

    try {
      // Widened deliberately. The plugin declares a narrower union than Android
      // actually returns: a first run answers `prompt-with-rationale`, which the
      // declared type says cannot happen, and comparing against it is a type
      // error rather than dead code.
      const permission: string = await scanner.checkPermissions();

      if (permission === "prompt" || permission === "prompt-with-rationale") {
        await scanner.requestPermissions();
      }

      // The preview is drawn behind the webview rather than into it, so the
      // page has to stop painting for it to be visible at all. The finder's
      // own backdrop puts the opacity back everywhere except its own bounds -
      // see the box-shadow below - so the camera appears in the box the design
      // draws and nowhere else.
      scanning = true;
      document.documentElement.classList.add("scanning");

      const result = await scanner.scan({ windowed: true, formats: [scanner.Format.QRCode] });

      await stopScan();
      await manager.begin(result.content);
    } catch (error) {
      await stopScan();

      // No camera at all on the desktop, or access refused. The paste field is
      // the whole desktop path and the recovery from a refusal.
      scanError = String(error);
    }
  }

  async function stopScan(): Promise<void> {
    if (!scanning) {
      return;
    }

    scanning = false;
    document.documentElement.classList.remove("scanning");

    try {
      const scanner = await import("@tauri-apps/plugin-barcode-scanner");
      await scanner.cancel();
    } catch {
      // Already finished. Cancelling a scan that resolved is not a failure.
    }
  }

  async function submit(): Promise<void> {
    await manager.submit(typed);
    typed = "";
    await focusSlot();
  }

  async function focusSlot(): Promise<void> {
    await tick();
    slot?.focus();
  }

  $effect(() => {
    if ($pairing.step === "found") {
      void focusSlot();
    }

    // Declared rather than done inline after `submit`: reading the store's value
    // straight after an await raced the update and left the screen on a dead end.
    if ($pairing.step === "paired") {
      void goto("/");
    }
  });
</script>

<div class="bar">
  <NavBar title="Add a server" subtitle="run tethera pair on the machine" onback={() => goto("/")} />
</div>

{#if $pairing.step === "idle" || $pairing.step === "reaching"}
  <div class="finder" class:live={scanning}>
    <i class="tick tl"></i>
    <i class="tick tr"></i>
    <i class="tick bl"></i>
    <i class="tick br"></i>

    {#if scanning}
      <i class="sweep"></i>
    {:else}
      <span class="idle">the code the machine is showing</span>
    {/if}
  </div>
{/if}

{#if $pairing.step === "idle"}
  <div class="pane">
    {#if scanning}
      <Button variant="quiet" onclick={stopScan}>Cancel</Button>
    {:else}
      <Button icon="scan" onclick={scan}>Scan the code</Button>

      {#if scanError}
        <p class="note">No camera available. Paste the link the machine printed instead.</p>
      {/if}

      <Label flush>or paste the link the machine printed</Label>
      <input class="uri" bind:value={pasted} placeholder="tethera://pair?..." spellcheck="false" />
      <Button
        variant="quiet"
        disabled={pasted.trim() === ""}
        onclick={() => manager.begin(pasted.trim())}
      >
        Use this link
      </Button>
    {/if}
  </div>
{:else if $pairing.step === "reaching"}
  <div class="pane">
    <Label flush>reaching the machine…</Label>
  </div>
{:else if $pairing.step === "found"}
  <div class="pane">
    <div class="found">
      <strong class="name">{$pairing.found.server.label}</strong>
      <span class="meta">
        {$pairing.found.endpoint_id.slice(0, 8)}…{$pairing.found.endpoint_id.slice(-4)}
        {#if $pairing.found.relay}· relay{/if}
        · {$pairing.found.direct_addr_count} direct addresses
      </span>
    </div>

    <Label flush>now type the code {$pairing.found.server.label} is showing</Label>

    <!-- CodeSlots renders the value; it does not collect it. The input is the
         real control and sits invisibly over the slots. -->
    <div class="slots">
      <CodeSlots value={typed} length={$pairing.found.code_length} />
      <input
        bind:this={slot}
        class="capture"
        bind:value={typed}
        inputmode="numeric"
        autocomplete="one-time-code"
        maxlength={$pairing.found.code_length}
        aria-label="Pairing code"
      />
    </div>

    {#if $pairing.attemptsLeft !== null}
      <p class="note warn">
        That code was not right. {$pairing.attemptsLeft}
        {$pairing.attemptsLeft === 1 ? "attempt" : "attempts"} left.
      </p>
    {/if}

    <p class="hint">
      <b>A link alone can never pair a device.</b> The code proves you were standing at the machine
      when you left it.
    </p>

    <Button disabled={typed.length !== $pairing.found.code_length} onclick={submit}>
      Pair with {$pairing.found.server.label}
    </Button>
  </div>
{:else if $pairing.step === "paired"}
  <div class="pane">
    <Label flush>paired with {$pairing.entry.server.label}</Label>
  </div>
{:else if $pairing.step === "refused"}
  <div class="pane">
    <p class="note warn">{Refusal.text($pairing.outcome, $pairing.label)}</p>
    <Button variant="quiet" onclick={() => manager.cancel()}>Start over</Button>
  </div>
{/if}

<style lang="scss">
  // Above the finder's backdrop, which paints over everything outside its own
  // bounds and would otherwise cover the title and the back control.
  .bar {
    position: relative;
    z-index: 1;
  }

  .finder {
    margin: 12px 18px 6px;
    aspect-ratio: 1;
    border-radius: 14px;
    background: var(--tc-surface);
    position: relative;
    display: grid;
    place-items: center;
    overflow: hidden;
  }

  // The hole. A spread large enough to cover any viewport paints the page's own
  // background everywhere outside this box and clips to its radius, so the
  // camera behind the webview shows through here and nowhere else. Without it a
  // transparent page makes the camera the background of the whole app.
  .finder.live {
    background: transparent;
    overflow: visible;
    box-shadow: 0 0 0 100vmax var(--tc-bg);
  }

  .tick {
    position: absolute;
    width: 36px;
    height: 36px;
    border: 2px solid var(--tc-accent, #4a90ff);
  }

  .tl {
    top: 16px;
    left: 16px;
    border-right: 0;
    border-bottom: 0;
    border-top-left-radius: 8px;
  }

  .tr {
    top: 16px;
    right: 16px;
    border-left: 0;
    border-bottom: 0;
    border-top-right-radius: 8px;
  }

  .bl {
    bottom: 16px;
    left: 16px;
    border-right: 0;
    border-top: 0;
    border-bottom-left-radius: 8px;
  }

  .br {
    bottom: 16px;
    right: 16px;
    border-left: 0;
    border-top: 0;
    border-bottom-right-radius: 8px;
  }

  .sweep {
    position: absolute;
    left: 20px;
    right: 20px;
    height: 1px;
    background: var(--tc-accent, #4a90ff);
    box-shadow: 0 0 14px 3px color-mix(in srgb, var(--tc-accent, #4a90ff) 55%, transparent);
    animation: sweep 3.6s ease-in-out infinite;
  }

  @keyframes sweep {
    0%,
    100% {
      top: 22%;
    }

    50% {
      top: 78%;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .sweep {
      animation: none;
      top: 50%;
    }
  }

  .idle {
    font-family: var(--tc-mono);
    font-size: 11px;
    color: var(--tc-ink-3);
    max-width: 18ch;
    text-align: center;
    line-height: 1.5;
  }

  .pane {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 8px 18px 32px;
  }

  .found {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .name {
    font-family: var(--tc-mono);
    font-size: 16px;
    letter-spacing: -0.02em;
  }

  .meta {
    font-family: var(--tc-mono);
    font-size: 10px;
    color: var(--tc-ink-3);
  }

  .slots {
    position: relative;
  }

  .capture {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    border: 0;
    background: none;
    font-size: 16px;
  }

  .uri {
    width: 100%;
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid var(--tc-line, rgba(255, 255, 255, 0.14));
    background: var(--tc-surface);
    color: inherit;
    font-family: var(--tc-mono);
    font-size: 13px;
  }

  .note,
  .hint {
    margin: 0;
    font-size: 13px;
    line-height: 1.5;
    color: var(--tc-ink-3);
  }

  .hint b {
    color: var(--tc-ink-2);
    font-weight: 600;
  }

  .warn {
    color: var(--tc-ink-2);
  }
</style>
