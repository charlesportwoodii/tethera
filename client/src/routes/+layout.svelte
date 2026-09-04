<script lang="ts">
  import type { Snippet } from "svelte";
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { page } from "$app/state";
  import "../app.scss";
  import LockScreen from "$components/LockScreen.svelte";

  let { children }: { children: Snippet } = $props();

  // Starts true so no screen paints before the answer arrives. Painting first
  // and covering afterwards shows the transcript of whatever was last open, in
  // the moment the lock exists to prevent.
  let deciding = $state(true);
  let locked = $state(false);

  let release: (() => void) | null = null;

  // When this app was last put away. Null on a cold launch, which is not a
  // resume at all.
  let hiddenAt: number | null = null;

  onMount(() => {
    void decide();

    // Returning from the background is the case that matters. Launching happens
    // once; a phone put down and picked up again happens all day, and it is the
    // one where somebody else is holding it.
    const onVisible = () => {
      if (document.visibilityState === "hidden") {
        hiddenAt = Date.now();

        void invoke("lock").catch(() => {});

        return;
      }

      void woken();

      // Not behind the resume: this decides whether anything paints at all, and
      // a lock screen that arrives two seconds late is two seconds of blank.
      void decide();
    };

    document.addEventListener("visibilitychange", onVisible);
    release = () => document.removeEventListener("visibilitychange", onVisible);
  });

  onDestroy(() => {
    if (release) {
      release();
    }
  });

  // Everything that has to happen, in order, when this app comes back.
  //
  // The order is the point. The Rust side was frozen for as long as this app
  // was away: its NAT mappings have expired, its relay socket is gone, and the
  // connections it holds still answer that they are open, so anything that
  // reaches a machine before the transport has been told waits out its whole
  // deadline and fails for a reason that reads like the machine being off.
  //
  // How long we were away is passed rather than measured over there, because
  // only this side has a clock that kept running.
  async function woken(): Promise<void> {
    const hidden = hiddenAt === null ? 0 : Date.now() - hiddenAt;
    hiddenAt = null;

    await invoke("resumed", { hidden }).catch(() => {});

    // A download interrupted by the switch away is asleep on a timer, and
    // coming back is the moment worth acting on rather than whenever that timer
    // expires. Told here rather than from a screen, because the transfer is not
    // a screen's and the person may return to a different one from the one they
    // left.
    await invoke("resume_downloads").catch(() => {});
  }

  async function decide(): Promise<void> {
    try {
      const unlocked = (await invoke("is_unlocked")) as boolean;
      locked = !unlocked;
    } catch {
      // A state that cannot be read is treated as locked. The other direction
      // opens the app on a failure, which is the wrong way for a lock to fail.
      locked = true;
    } finally {
      deciding = false;
    }
  }
</script>

{#if deciding}
  <div class="hold"></div>
{:else if locked}
  <LockScreen onopen={() => (locked = false)} />
{:else}
  <!--
    Keyed on the whole URL, so a screen is rebuilt when its parameters change.
    Every route here reads its parameters once, at construction — `const id =
    page.url.searchParams.get("id")` — and hands them to a manager that keeps
    them for life. SvelteKit reuses a component when navigating between two
    URLs of the *same* route, so without this, opening one conversation from
    inside another leaves the screen bound to the first: the transcript on
    screen and the id every send carries stop agreeing, and a prompt reaches
    the agent somebody was reading a moment ago rather than the one they are
    looking at.

    Keyed here rather than fixed in each route because it is the routes'
    shared assumption that is wrong, and a route added later would inherit the
    same bug without inheriting the remedy.
  -->
  {#key page.url.href}
    {@render children()}
  {/key}
{/if}

<style lang="scss">
  .hold {
    height: 100dvh;
    background: var(--tc-bg);
  }
</style>
