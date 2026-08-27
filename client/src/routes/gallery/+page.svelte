<script lang="ts">
  import { TerminalGrid } from "$console/lib/terminal";
  import {
    Button,
    Chip,
    CodeSlots,
    Composer,
    ConnDot,
    Drawer,
    EmptyState,
    FileCard,
    Icon,
    KeyBar,
    PaneMap,
    Label,
    NavBar,
    PartView,
    QuestionCard,
    StatusGlyph,
    TabStrip,
    TerminalPane,
    TerminalView,
    ViewToggle,
    Timeline,
    ToolFold,
    Tree,
    TreeNode,
    TreeTwig,
    Turn,
    Toggle,
    type DrawerHeight,
    type GlyphState,
    type PaneBox,
    type WorkspaceView,
  } from "$console";
  import type { Part } from "$bindings/Part";
  import type { Question } from "$bindings/Question";
  import type { Tab } from "$bindings/Tab";

  // The gallery is the visual contract: every component, in isolation, with the
  // states that are hard to reach by clicking around a real screen.
  const GLYPHS: GlyphState[] = [
    "working",
    "stalled",
    "idle",
    "done",
    "blocked",
    "offline",
    "set",
    "unset",
  ];

  const ROUTE_QUESTION: Question = {
    id: "q1",
    fingerprint: "fp-demo",
    asks: [
      {
        header: "Route",
        prompt: "Which route should own tethera://pair?",
        options: [
          { label: "Rewrite before the router sees it.", description: "One place to fix." },
          { label: "Register pair as a real route.", description: "Fights the framework." },
          { label: "Handle it outside the router.", description: "More code, fewer surprises." },
        ],
        multi_select: false,
        allows_free_text: true,
      },
    ],
  };

  let harness = $state("claude");
  let notify = $state(true);
  let code = $state("7329");
  let draft = $state("");
  let drawer = $state<DrawerHeight>("half");
  let tab = $state("b");
  let pane = $state("p1");
  let workspaceView = $state<WorkspaceView>("terminal");

  // Tabs come from the wire now: the index is the backend's own ordinal, which is
  // what a person means by "2:build", and agent state is keyed separately because
  // a plain shell has none.
  const TABS: Tab[] = [
    {
      id: "a",
      workspace_id: "w1",
      index: 1,
      title: "claude",
      conversation: "c1",
      foreground_command: "claude",
    },
    {
      id: "b",
      workspace_id: "w1",
      index: 2,
      title: "build",
      conversation: null,
      foreground_command: "cargo",
    },
    {
      id: "c",
      workspace_id: "w1",
      index: 3,
      title: "git",
      conversation: null,
      foreground_command: null,
    },
  ];

  const TAB_STATES: Record<string, GlyphState> = { a: "blocked" };

  const PANES: PaneBox[] = [
    { id: "p1", label: "claude", x: 0, y: 0, w: 0.5, h: 1 },
    { id: "p2", label: "cargo watch", x: 0.5, y: 0, w: 0.5, h: 0.5 },
    { id: "p3", label: "git log", x: 0.5, y: 0.5, w: 0.5, h: 0.5 },
  ];

  // The component owns a mutable grid rather than a list of lines, because a
  // damage frame only means anything against what came before. A showcase has
  // no live pane, so this is an empty one at a readable size.
  const TERM_GRID = new TerminalGrid(60, 12);

  // The assembled terminal side wants something on screen: an empty grid shows
  // the chrome but not what the chrome is wrapped around.
  const PANE_GRID = new TerminalGrid();
  PANE_GRID.apply({
    snapshot: {
      cols: 46,
      rows: 9,
      styles: [
        { fg: "default", bg: "default", attrs: 0 },
        { fg: { indexed: 2 }, bg: "default", attrs: 0 },
        { fg: { indexed: 1 }, bg: "default", attrs: 1 },
        { fg: "default", bg: "default", attrs: 2 },
      ],
      rows_data: [
        { y: 0, from_x: 0, spans: [{ style: 3, text: "charl@atlas ~/projects/tethera" }] },
        { y: 1, from_x: 0, spans: [{ style: 1, text: "❯ " }, { style: 0, text: "cargo test -p tethera-common" }] },
        { y: 2, from_x: 0, spans: [{ style: 3, text: "    Finished test profile in 4.31s" }] },
        { y: 4, from_x: 0, spans: [{ style: 1, text: "ok" }, { style: 0, text: "   wire::unknown_part_keeps_text" }] },
        { y: 5, from_x: 0, spans: [{ style: 2, text: "FAILED" }, { style: 0, text: " pair::uri::host_not_path" }] },
        { y: 7, from_x: 0, spans: [{ style: 1, text: "❯ " }] },
      ],
      cursor: { x: 2, y: 7, visible: true, shape: "block" },
      alt_screen: false,
      scrollback_len: 240,
    },
  });

  const PARTS: Part[] = [
    { text: { text: "The document is explicit that pair is the URI host, not a path." } },
    {
      tool_use: {
        name: "Bash · grep -rn",
        input: 'grep -rn "tethera://" old/app/src',
        result: "2 hits",
        status: "ok",
        fallback_text: "",
      },
    },
    {
      diff: {
        path: "src/lib/deeplink.ts",
        unified: "@@ -36,3 +36,5 @@\n   const u = new URL(raw);\n-  return u;\n+  if (u.host) return route(u.host);\n+  return u;",
        added: 2,
        removed: 1,
        fallback_text: "",
      },
    },
    {
      todo: {
        items: [
          { text: "Read the pairing contract", status: "done" },
          { text: "Decide who owns the rewrite", status: "in_progress" },
          { text: "Write the test first", status: "pending" },
        ],
        fallback_text: "",
      },
    },
    {
      table: {
        columns: ["test", "result"],
        rows: [
          ["pair::uri::host_not_path", "FAILED"],
          ["agent::catalog::claude_default", "ok"],
        ],
        fallback_text: "",
      },
    },
    { status: { label: "Compacted", detail: "62k tokens reclaimed", fallback_text: "" } },
    {
      file: {
        asset: "as_9f21c",
        name: "pairing-routes.md",
        mime: "text/markdown",
        size: 8396,
        fallback_text: "",
      },
    },
    { question: { question: ROUTE_QUESTION, answered: null, fallback_text: "" } },
    { unknown: { kind: "chart", fallback_text: "--- a/deeplink.ts\n+++ b/deeplink.ts" } },
  ];

</script>

<div class="gallery">
  <header class="gallery__mast">
    <span class="gallery__eyebrow">Tethera \u00b7 Console design system</span>
    <h1>Building blocks</h1>
    <p>
      Every component in isolation. Nothing here fetches, routes, or knows what a screen is —
      composing them into screens is the integrator's job.
    </p>
  </header>

  <section class="gallery__row">
    <h2>StatusGlyph</h2>
    <div class="gallery__panel gallery__inline">
      {#each GLYPHS as state (state)}
        <span class="gallery__glyph"><StatusGlyph {state} /> <code>{state}</code></span>
      {/each}
    </div>
  </section>

  <section class="gallery__row">
    <h2>ConnDot</h2>
    <div class="gallery__panel">
      <ConnDot link="direct" rttMs={38} />
      <ConnDot link="relayed" rttMs={112} />
      <ConnDot link="offline" lastSeen="2d" />
      <ConnDot link="unknown" />
      <ConnDot link="direct" rttMs={38} note="native" />
    </div>
  </section>

  <section class="gallery__row">
    <h2>Button, Chip, Toggle</h2>
    <div class="gallery__panel">
      <Button icon="plus">New session</Button>
      <Button variant="quiet">Forget this server</Button>
      <Button disabled>Start session</Button>
      <div class="gallery__inline">
        <Chip
          label="Claude Code"
          detail="2.1.4"
          selected={harness === "claude"}
          onclick={() => (harness = "claude")}
        />
        <Chip
          label="Codex"
          detail="0.8"
          selected={harness === "codex"}
          onclick={() => (harness = "codex")}
        />
      </div>
      <div class="gallery__inline">
        <Toggle label="Push notifications" checked={notify} onchange={(v) => (notify = v)} />
        <code>checked = {notify}</code>
      </div>
    </div>
  </section>

  <section class="gallery__row">
    <h2>NavBar</h2>
    <div class="gallery__panel gallery__phone">
      <NavBar title="Servers" subtitle="3 paired \u00b7 1 not answering">
        {#snippet actions()}
          <Icon name="scan" label="Add a server" />
          <Icon name="settings" label="Settings" />
        {/snippet}
      </NavBar>
      <NavBar title="atlas" subtitle="5 workspaces" onback={() => {}} />
    </div>
  </section>

  <section class="gallery__row">
    <h2>Tree, TreeNode, TreeTwig</h2>
    <div class="gallery__panel gallery__phone">
      <Tree label="Servers">
        <TreeNode state="blocked" branches>
          <strong class="gallery__name">atlas</strong>
          <ConnDot link="direct" rttMs={38} />
          <TreeTwig state="blocked">
            <div class="gallery__twig">Pairing deep link</div>
            <div class="gallery__meta">tethera-3 \u00b7 claude</div>
          </TreeTwig>
          <TreeTwig state="working">
            <div class="gallery__twig">Flaky NAT punch test</div>
            <div class="gallery__meta">bvc-relay \u00b7 claude</div>
          </TreeTwig>
        </TreeNode>
        <TreeNode state="offline" spaced dim>
          <strong class="gallery__name">keel</strong>
          <ConnDot link="offline" lastSeen="2d" />
      <ConnDot link="unknown" />
        </TreeNode>
      </Tree>
    </div>
  </section>

  <section class="gallery__row">
    <h2>Label, CodeSlots</h2>
    <div class="gallery__panel gallery__phone">
      <Label flush>now type the code atlas is showing</Label>
      <CodeSlots value={code} />
      <div class="gallery__inline" style="padding: 12px 18px">
        <Button onclick={() => (code = code.length < 6 ? code + "4" : "")}>
          {code.length < 6 ? "Type a digit" : "Clear"}
        </Button>
      </div>
    </div>
  </section>

  <section class="gallery__row">
    <h2>Timeline, Turn, ToolFold, QuestionCard, FileCard</h2>
    <div class="gallery__panel gallery__phone gallery__tall">
      <Timeline>
        <Turn role="operator" time="14:18">
          <p>Read the pairing contract and tell me how the deep link should be routed.</p>
        </Turn>
        <Turn role="agent" time="14:19">
          <p>The document is explicit that pair is the URI host, not a path.</p>
        </Turn>
        <Turn role="agent" time="14:19">
          <ToolFold name="Bash · grep -rn" detail="2 hits" status="ok" />
        </Turn>
        <Turn role="agent" time="14:22">
          <ToolFold name="deeplink.ts" detail="+3 −1" status="ok" />
        </Turn>
        <Turn role="agent" time="14:31">
          <FileCard name="pairing-routes.md" size={8396} at="14:31" />
        </Turn>
        <Turn role="agent" time="14:29" marked>
          <QuestionCard question={ROUTE_QUESTION} waiting="3m" onopen={() => {}} />
        </Turn>
      </Timeline>
      <Composer value={draft} oninput={(v) => (draft = v)} onattach={() => {}} />
    </div>
  </section>

  <section class="gallery__row">
    <h2>PartView \u2014 every Part the wire can send</h2>
    <div class="gallery__panel gallery__phone">
      <Timeline>
        {#each PARTS as part, i (i)}
          <Turn role="agent" time="14:2{i}">
            <PartView {part} at="14:31" />
          </Turn>
        {/each}
      </Timeline>
    </div>
  </section>

  <section class="gallery__row">
    <h2>ViewToggle, TabStrip, PaneMap, TerminalView, KeyBar</h2>
    <div class="gallery__panel gallery__phone gallery__tall">
      <Drawer
        label="tethera-3"
        summary="11 passed, 1 failed"
        height={drawer}
        onheight={(h) => (drawer = h)}
      >
        <ViewToggle
          view={workspaceView}
          chatBadge="waiting"
          onchange={(v) => (workspaceView = v)}
        />
        <TabStrip
          tabs={TABS}
          activeId={tab}
          states={TAB_STATES}
          onselect={(id) => (tab = id)}
          onadd={() => {}}
        />
        <PaneMap
          panes={PANES}
          activeId={pane}
          onselect={(id) => (pane = id)}
          onsplit={() => {}}
        />
        <TerminalView grid={TERM_GRID} label="60x12" />
        <KeyBar />
      </Drawer>
    </div>
    <p class="gallery__note">The head cycles peek \u2192 half \u2192 full. Current: <code>{drawer}</code></p>
  </section>
</div>

<style lang="scss">
  .gallery {
    max-width: 900px;
    margin: 0 auto;
    padding: 48px 20px 120px;

    &__eyebrow {
      font-family: var(--tc-mono);
      font-size: 11px;
      letter-spacing: 0.18em;
      text-transform: uppercase;
      color: var(--tc-ink-3);
    }

    &__mast {
      margin-bottom: 40px;

      h1 {
        margin: 12px 0 10px;
        font-size: 2.4rem;
        letter-spacing: -0.04em;
      }

      p {
        margin: 0;
        max-width: 64ch;
        color: var(--tc-ink-2);
      }
    }

    &__row {
      margin-bottom: 36px;

      h2 {
        font-size: 0.95rem;
        font-weight: 600;
        letter-spacing: -0.01em;
        color: var(--tc-ink-2);
        margin: 0 0 10px;
      }
    }

    &__panel {
      background: var(--tc-surface);
      border-radius: 12px;
      padding: 18px;
      display: flex;
      flex-direction: column;
      gap: 14px;
    }

    &__phone {
      max-width: 340px;
      padding: 0 0 12px;
    }

    &__tall {
      height: 560px;
      overflow: hidden;
    }

    &__inline {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 12px;
    }

    &__glyph {
      display: inline-flex;
      align-items: center;
      gap: 7px;
    }

    &__name {
      font-family: var(--tc-mono);
      font-size: 16px;
      letter-spacing: -0.02em;
    }

    &__twig {
      font-size: 13.5px;
      font-weight: 600;
      letter-spacing: -0.015em;
    }

    &__meta {
      font-family: var(--tc-mono);
      font-size: 9.5px;
      color: var(--tc-ink-3);
      margin-top: 2px;
    }

    &__note {
      margin: 10px 0 0;
      font-size: 13px;
      color: var(--tc-ink-3);
    }

    code {
      font-family: var(--tc-mono);
      font-size: 11.5px;
      color: var(--tc-ink-3);
    }
  }
</style>
