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

  onMount(() => {
    void decide();

    // Returning from the background is the case that matters. Launching happens
    // once; a phone put down and picked up again happens all day, and it is the
    // one where somebody else is holding it.
    const onVisible = () => {
      if (document.visibilityState === "hidden") {
        void invoke("lock").catch(() => {});

        return;
      }

      // A download interrupted by the switch away is asleep on a timer, and
      // coming back is the moment worth acting on rather than whenever that
      // timer expires. Told here rather than from a screen, because the
      // transfer is not a screen's and the person may return to a different
      // one from the one they left.
      void invoke("resume_downloads").catch(() => {});
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
