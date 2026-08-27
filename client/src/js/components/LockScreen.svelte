<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Button } from "$console";

  interface Props {
    onopen: () => void;
  }

  let { onopen }: Props = $props();

  let asking = $state(false);
  let refused = $state(false);

  onMount(() => {
    // Asked once on arrival, so the ordinary case is a thumb already on the
    // sensor and no button ever seen. The button below is what a second attempt
    // uses.
    void ask();
  });

  async function ask(): Promise<void> {
    if (asking) {
      return;
    }

    asking = true;
    refused = false;

    try {
      const biometric = await import("@tauri-apps/plugin-biometric");

      await biometric.authenticate("Unlock Tethera", {
        // The way back in when the sensor will not read. A person locked out of
        // this app is locked out of the machine they would fix it from, so the
        // device PIN is always accepted.
        allowDeviceCredential: true,
        title: "Tethera",
        confirmationRequired: false,
      });

      await invoke("unlock");
      onopen();
    } catch {
      // Cancelled, or a thumb in the wrong place. Neither is a fault worth
      // naming, and both are answered the same way: the door stays shut and the
      // button is there to try again.
      refused = true;
    } finally {
      asking = false;
    }
  }
</script>

<div class="lock">
  <div class="middle">
    <p class="mark">TETHERA</p>
    <p class="say">
      {#if asking}
        Waiting for you…
      {:else if refused}
        That did not open it.
      {:else}
        Locked.
      {/if}
    </p>

    {#if !asking}
      <Button onclick={ask}>Unlock</Button>
    {/if}
  </div>

  <p class="foot">Your machines are unreachable until this opens.</p>
</div>

<style lang="scss">
  .lock {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 18px;
    height: 100dvh;
    padding: 24px;
    background: var(--tc-bg);
  }

  .middle {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }

  .mark {
    margin: 0;
    font-family: var(--tc-mono);
    font-size: 12px;
    letter-spacing: 0.34em;
    color: var(--tc-ink-3);
  }

  .say {
    margin: 0;
    font-size: 15px;
    color: var(--tc-ink-1);
  }

  .foot {
    position: absolute;
    bottom: 28px;
    margin: 0;
    font-size: 11px;
    color: var(--tc-ink-3);
  }
</style>
