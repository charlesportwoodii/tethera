<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { beforeNavigate, goto } from "$app/navigation";
  import { page } from "$app/state";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { Button, Chip, EmptyState, KeyStream, NavBar, TabStrip, TerminalView } from "$console";
  import ComposerRail from "$components/ComposerRail.svelte";
  import LayoutSheet from "$components/LayoutSheet.svelte";
  import { MachineManager } from "$managers/machine_manager";
  import { TerminalManager, type Listen } from "$managers/terminal_manager";
  import type { Key } from "$bindings/Key";
  import type { Mods } from "$bindings/Mods";
  import type { Pane } from "$bindings/Pane";
  import type { SplitDirection } from "$bindings/SplitDirection";

  /**
   * The geometry this screen claims for a pane it attaches to.
   *
   * A claim on the pty, not a hint and not the grid's size. The grid sizes
   * itself from whatever a snapshot says, so these two numbers only ever leave
   * here in an `AttachSpec` — which is why they have to describe what a phone
   * can actually draw. Claiming more rows than that resizes the pty to a height
   * the desk's pane cannot show either: measured on a 91x39 pane, a 200-row
   * claim put vim's status line 160 rows below the last visible one.
   *
   * A character of the console's mono face at 11px is close to 6.6px wide and
   * 17px tall, so a 393px phone fits about 58 columns, and the space between the
   * tab strip and the composer fits about 34 rows.
   *
   * Fixed rather than measured because a geometry that changed with the keyboard
   * appearing would reflow the pane underneath somebody mid-read — and it would
   * reflow it at the desk too, because there is one pty and it has one size.
   */
  const COLS = 58;
  const ROWS = 34;

  const server = page.url.searchParams.get("server") ?? "";
  const workspace = page.url.searchParams.get("workspace") ?? "";

  /**
   * The session this screen was opened from, when it was opened from one.
   *
   * Carried in the URL because the terminal is a side of a session rather than
   * a place of its own. Empty when the machine screen opened it directly, and
   * the tab's own panes are then the only way to find a chat to return to.
   */
  const opener = page.url.searchParams.get("conversation") ?? "";

  const tree = new MachineManager(invoke, listen as unknown as Listen, server);
  const manager = new TerminalManager(
    invoke,
    listen as unknown as Listen,
    server,
    COLS,
    ROWS,
  );

  const revision = manager.revision;
  const live = manager.live;
  const failure = manager.error;
  const controls = manager.controls;
  const treeError = tree.error;

  let activeTab = $state<string | null>(null);
  let selectedPane = $state<string | null>(null);
  let loading = $state(true);

  /**
   * Whether a pane is on screen, rather than the floorplan.
   *
   * False on arrival, deliberately. Opening the terminal is somebody asking to
   * see their terminals, not asking to be dropped into whatever raw TUI the
   * focused pane happens to be running.
   */
  let entered = $state(false);

  const tabList = tree.tabs;
  const paneList = tree.panes;
  const layoutList = tree.layouts;
  const conversationList = tree.conversations;

  const tabs = $derived(
    $tabList.filter((tab) => (tab.workspace_id as unknown as string) === workspace),
  );

  const panes = $derived(
    activeTab === null
      ? []
      : $paneList.filter((pane) => (pane.tab_id as unknown as string) === activeTab),
  );

  // Read from the store rather than through `layoutOf`, so a `LayoutChanged`
  // arriving while this screen is open redraws the map. The accessor answers
  // from the manager's own copy, which Svelte has nothing to compare.
  const layout = $derived(
    $layoutList.find((held) => (held.tab as unknown as string) === activeTab) ?? null,
  );

  const held = $derived(
    panes.find((pane) => (pane.id as unknown as string) === selectedPane) ?? null,
  );

  const paneLabel = $derived(held?.foreground_command ?? held?.label ?? "pane");

  /**
   * Leaves a pane that has stopped existing.
   *
   * `exit` in a pane destroys it, and the screen has no way to know from the
   * grid alone: the last frame drawn stays drawn. The tree says so within a
   * poll interval, and the honest answer is the floorplan rather than a frozen
   * terminal somebody can still type into.
   *
   * Guarded the same way the tab fallback is, and for the same reason: a pane
   * this screen just entered is absent from the tree until the watch delivers
   * it, and treating that as a death would bounce straight back out.
   */
  let enteredPane: string | null = null;

  $effect(() => {
    if (!entered || selectedPane === null || panes.length === 0) {
      return;
    }

    if (panes.some((pane) => (pane.id as unknown as string) === selectedPane)) {
      enteredPane = selectedPane;

      return;
    }

    if (enteredPane === selectedPane) {
      enteredPane = null;
      entered = false;
      manager.close();
    }
  });

  /**
   * The last tab this screen was on that the tree actually confirmed.
   *
   * Not `$state`: it is a record of what has been seen, and making it reactive
   * would re-run the effect below on the way back out.
   */
  let confirmed: string | null = null;

  /**
   * Follows the tab strip when the tab this screen is on stops existing.
   *
   * A tab closed at the desk leaves the strip within a poll interval, and
   * without this the screen stays pointed at it: an empty map, no actions, and
   * a hint telling somebody to tap a rectangle that is not there.
   *
   * Only for a tab that was *confirmed and then vanished*. A tab this screen
   * has just created is absent from the tree for as long as the watch takes to
   * deliver it, and treating that as a disappearance yanks the screen back to
   * the first tab the moment somebody presses `+`.
   */
  $effect(() => {
    if (loading || tabs.length === 0 || activeTab === null) {
      return;
    }

    if (tabs.some((tab) => (tab.id as unknown as string) === activeTab)) {
      confirmed = activeTab;

      return;
    }

    if (confirmed === activeTab) {
      confirmed = null;
      void selectTab(tabs[0].id as unknown as string);
    }
  });

  /**
   * The system back gesture leads to chat too, not to wherever this screen was
   * pushed from.
   *
   * Opened from chat, the history already answers correctly. Opened from the
   * machine screen it does not, and the two would disagree with each other and
   * with the chevron. Back out of a terminal means "the other side of this
   * session" wherever the screen was reached from.
   *
   * Only when there is a session to return to: a workspace with no readable
   * one has no chat, and popping to the machine list is then the honest answer
   * rather than a redirect to nothing.
   */
  beforeNavigate((nav) => {
    if (nav.type !== "popstate" || !chatTarget) {
      return;
    }

    if (nav.to?.url.pathname === "/conversation") {
      return;
    }

    nav.cancel();
    toChat();
  });

  onMount(() => {
    void start();
  });

  onDestroy(() => {
    manager.close();
    tree.close();
  });

  async function start(): Promise<void> {
    await manager.loadControls();
    await tree.open();

    const first = tabs[0];

    if (first !== undefined) {
      await selectTab(first.id as unknown as string);
    }

    loading = false;
  }

  /**
   * Shows this tab here, and moves the desk to it.
   *
   * The local switch does not wait on the machine. The phone is a second window
   * onto one session rather than a separate session, so the desk follows — but a
   * slow or refused focus must not leave somebody staring at the tab they were
   * already on.
   */
  async function selectTab(id: string): Promise<void> {
    activeTab = id;
    entered = false;

    const focused = tree.panesOf(id).find((pane) => pane.focused) ?? tree.panesOf(id)[0];

    selectedPane = focused === undefined ? null : (focused.id as unknown as string);

    if ($controls.focus_tab) {
      try {
        await invoke("focus_tab", { server, tab: id });
      } catch (error) {
        console.error("the desk did not move to this tab", error);
      }
    }
  }

  async function enterPane(id: string): Promise<void> {
    selectedPane = id;
    entered = true;

    await manager.open(id);
  }

  /**
   * Makes a tab and shows it, without waiting for the watch to say so.
   *
   * The watch will confirm within its poll interval. A strip that did not move
   * when somebody pressed `+` reads as a button that did nothing, and two
   * seconds is long enough for that to be the conclusion they reach.
   */
  async function addTab(): Promise<void> {
    try {
      const made = (await invoke("open_terminal", {
        server,
        workspace,
        cwd: null,
      })) as Pane;

      await selectTab(made.tab_id as unknown as string);

      // Named from the create's own answer, not from the tree. `selectTab` picks
      // the focused pane out of the watch's snapshot, and the watch has not
      // delivered this tab yet - so without this the new tab lands with nothing
      // selected and no actions, and the first thing somebody has to do after
      // making a terminal is tap the one rectangle on the screen.
      selectedPane = made.id as unknown as string;
    } catch (error) {
      console.error("could not open a terminal", error);
    }
  }

  async function split(pane: string, direction: SplitDirection): Promise<void> {
    try {
      await invoke("split_pane", { server, pane, direction });
    } catch (error) {
      console.error("could not split this pane", error);
    }
  }

  async function closePane(pane: string): Promise<void> {
    try {
      await invoke("close_pane", { server, pane });
    } catch (error) {
      console.error("could not close this pane", error);
    }
  }

  function send(key: Key, mods: Mods): void {
    void manager.key(key, mods);
  }

  /**
   * The chat this terminal belongs to.
   *
   * The opener first, because it is what somebody actually came from — the
   * tab's agent is a guess at the same answer and is wrong whenever a tab holds
   * more than one. Null for a workspace with no readable session at all, which
   * is a real state and the only case where back cannot mean chat.
   */
  /**
   * A chat among the panes of the tab on screen, if one is readable.
   *
   * The machine screen opens a terminal *only* for a session with no readable
   * transcript — deliberately, so that somebody is not dropped on an empty chat
   * that reads as the machine failing. So an unreadable session is not a chat
   * this screen can offer, and a guess has to be checked.
   */
  const chatOfTab = $derived.by(() => {
    const talking = panes.find((pane) => {
      const held = pane.conversation as unknown as string | null;

      return (
        held !== null &&
        $conversationList.some(
          (seen) => (seen.id as unknown as string) === held && seen.has_transcript,
        )
      );
    });

    return talking === undefined ? null : (talking.conversation as unknown as string);
  });

  /**
   * Where leaving this screen goes: the session it was opened from.
   *
   * The opener is proven — whoever set it was looking at that chat a moment ago
   * — so it is trusted without the readability check the guess needs.
   */
  const chatTarget = $derived(opener || chatOfTab);

  /**
   * The chat side of the tab on screen.
   *
   * Not the same question as back. Back means "leave this screen", and the
   * honest answer is where somebody came from. The mode key means "show me the
   * other side of *this*", and answering it with the opener sends somebody who
   * has switched tabs to a conversation that has nothing to do with what they
   * are looking at — which is a prompt typed at the wrong agent.
   */
  function toTabChat(): void {
    const target = chatOfTab ?? opener;

    if (!target) {
      void goto(`/server?id=${encodeURIComponent(server)}`);

      return;
    }

    void goto(
      `/conversation?server=${encodeURIComponent(server)}` +
        `&id=${encodeURIComponent(target)}`,
    );
  }

  function toChat(): void {
    if (!chatTarget) {
      void goto(`/server?id=${encodeURIComponent(server)}`);

      return;
    }

    void goto(
      `/conversation?server=${encodeURIComponent(server)}` +
        `&id=${encodeURIComponent(chatTarget)}`,
    );
  }

  function openChat(conversation: string): void {
    void goto(
      `/conversation?server=${encodeURIComponent(server)}` +
        `&id=${encodeURIComponent(conversation)}`,
    );
  }
</script>

<!--
  Fixed rather than in the flow. The rail and the pane both sit at the bottom of
  a full-height screen, so a message in the flow would render underneath
  whichever of them is covering it - which is how a refusal went unread for an
  evening on the conversation screen.
-->
{#if $failure || $treeError}
  <p class="over" role="alert">{$failure ?? $treeError}</p>
{/if}

<div class="screen">
  <NavBar
    title="Terminal"
    subtitle={entered ? paneLabel : "your panes"}
    onback={toChat}
  />

  {#if loading}
    <p class="note">Reading this workspace…</p>
  {:else if tabs.length === 0}
    <div class="pad">
      <EmptyState
        icon="terminal"
        title="Nothing open here"
        body="This workspace has no terminal tab yet. Making one starts a shell in the workspace's own directory, and it appears at your desk as well."
      >
        {#snippet actions()}
          {#if $controls.open}
            <Button icon="plus" onclick={addTab}>New tab</Button>
          {/if}
        {/snippet}
      </EmptyState>
    </div>
  {:else}
    <TabStrip
      {tabs}
      activeId={activeTab as never}
      onselect={(id) => void selectTab(id as unknown as string)}
      onadd={$controls.open ? () => void addTab() : null}
    />

    {#if !entered && layout === null}
      <!--
        A machine that will not place its panes gets a list rather than a map.
        `Floorplan.place` draws nothing without a layout, and inventing an
        arrangement here would put a picture on screen that somebody would
        trust. Choosing a pane still has to work.
      -->
      <div class="pad">
        <p class="note">
          This machine does not report where its panes sit, so there is no map to draw. These
          are the panes in this tab.
        </p>

        <div class="row">
          {#each panes as pane (pane.id)}
            <!--
              A pane with no shim cannot be shown at all, so it is drawn as
              unavailable rather than left tappable. Tapping one answers with a
              refusal, and a control that refuses on press teaches somebody that
              the app is unreliable.
            -->
            <Chip
              label={pane.foreground_command ?? pane.label}
              selected={(pane.id as unknown as string) === selectedPane}
              onclick={pane.streamed
                ? () => void enterPane(pane.id as unknown as string)
                : undefined}
            />
          {/each}
        </div>

        {#if panes.some((pane) => !pane.streamed)}
          <p class="note">
            A pane that is not offered here started before the terminal shim, so nothing on this
            machine can read it. Close it and open a new one, and it will stream.
          </p>
        {/if}
      </div>
    {:else if !entered}
      <!--
        The landing screen, not a detour. Somebody opening the terminal wants to
        see their terminals or make a new one; a pane fills the screen only once
        they have chosen it.
      -->
      <LayoutSheet
        {panes}
        {layout}
        selected={selectedPane}
        controls={$controls}
        onselect={(pane) => (selectedPane = pane)}
        onenter={(pane) => void enterPane(pane)}
        onchat={openChat}
        onsplit={(pane, direction) => void split(pane, direction)}
        onclosepane={(pane) => void closePane(pane)}
      />
    {:else if !$controls.attach}
      <p class="note">
        This machine will not stream a pane. It can list what is open, and open and close tabs,
        but nothing here can show you what a pane is doing.
      </p>
    {:else}
      {#if !$live}
        <p class="note">Not following. Anything typed will not arrive.</p>
      {/if}

      <TerminalView grid={manager.grid} revision={$revision} label={paneLabel} />
    {/if}

    <ComposerRail
      mode="terminal"
      onmode={toTabChat}
      onmap={entered ? () => (entered = false) : null}
      mapBadge={panes.length}
      onkey={entered && $controls.input ? send : null}
    />

    {#if entered && $controls.input}
      <div class="line">
        <KeyStream ontext={(text) => void manager.text(text)} onkey={send} />
      </div>
    {/if}
  {/if}
</div>

<style lang="scss">
  // Bounded here rather than inherited. Nothing above this establishes a
  // height, so a 200-row grid lays the page out to its own length and the rail
  // ends up thousands of pixels below the fold - which reads as a terminal you
  // cannot type into rather than as a layout fault.
  .screen {
    display: flex;
    flex-direction: column;
    height: 100dvh;
    overflow: hidden;
  }

  .row {
    display: flex;
    gap: 6px;
    padding: 8px 16px;
    overflow-x: auto;
  }

  .pad {
    padding: 16px 18px;
  }

  .note {
    margin: 0;
    padding: 10px 18px;
    font-size: 13px;
    line-height: 1.5;
    color: var(--tc-ink-3);
  }

  // One row holding the keystream, which brings its own field styling from the
  // console. Nothing here overrides it: a terminal's input is the console's to
  // look after, and a screen restyling it is how two of them drift apart.
  .line {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
  }

  .over {
    position: fixed;
    z-index: 7;
    top: 12px;
    left: 12px;
    right: 12px;
    margin: 0;
    padding: 10px 12px;
    border: 1px solid var(--tc-attn);
    border-radius: var(--tc-r-chip);
    background: var(--tc-surface);
    color: var(--tc-ink);
    font-size: 12.5px;
    line-height: 1.45;
  }
</style>
