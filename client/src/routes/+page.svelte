<script lang="ts">
  import {
    AskBlock,
    Button,
    Chip,
    CodeSlots,
    Composer,
    ConnDot,
    Drawer,
    FileCard,
    Icon,
    KeyBar,
    Label,
    NavBar,
    PartView,
    StatusGlyph,
    TabStrip,
    TerminalView,
    Timeline,
    ToolFold,
    Tree,
    TreeNode,
    TreeTwig,
    Turn,
    Toggle,
    type DrawerHeight,
    type GlyphState,
  } from "$console";
  import type { Part } from "$bindings/Part";

  // The gallery is the visual contract: every component, in isolation, with the
  // states that are hard to reach by clicking around a real screen.
  const GLYPHS: GlyphState[] = ["working", "idle", "done", "blocked", "offline", "set", "unset"];

  let harness = $state("claude");
  let notify = $state(true);
  let code = $state("7329");
  let draft = $state("");
  let drawer = $state<DrawerHeight>("half");
  let tab = $state("b");

  const TABS = [
    { id: "a", label: "1:claude", state: "blocked" as const },
    { id: "b", label: "2:build" },
    { id: "c", label: "3:git" },
  ];

  const TERM = [
    { text: "charl@atlas ~/projects/tethera", tone: "dim" as const },
    { text: "\u276F cargo test -p tethera-common" },
    { text: "running 12 tests", tone: "accent" as const },
    { text: "ok   agent::catalog::claude_default", tone: "ok" as const },
    { text: "FAIL pair::uri::host_not_path", tone: "attn" as const },
    { text: '  left:  "/pair"', tone: "warn" as const },
    { text: "11 passed; 1 failed", tone: "dim" as const },
  ];

  const PARTS: Part[] = [
    { text: { text: "The document is explicit that pair is the URI host, not a path." } },
    { tool_use: { name: "Bash \u00b7 grep -rn", input: "", fallback_text: "" } },
    { file: { name: "pairing-routes.md", size: 8396n, fallback_text: "" } },
    { unknown: { kind: "diff", fallback_text: "--- a/deeplink.ts\n+++ b/deeplink.ts" } },
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
    <h2>Timeline, Turn, ToolFold, AskBlock, FileCard</h2>
    <div class="gallery__panel gallery__phone gallery__tall">
      <Timeline>
        <Turn role="you" time="14:18">
          <p>Read the pairing contract and tell me how the deep link should be routed.</p>
        </Turn>
        <Turn role="agent" time="14:19">
          <p>The document is explicit that pair is the URI host, not a path.</p>
        </Turn>
        <Turn role="agent" time="14:19">
          <ToolFold name="Bash \u00b7 grep -rn" detail="2 hits" />
        </Turn>
        <Turn role="agent" time="14:22">
          <ToolFold name="deeplink.ts" detail="+3 -1" tone="ok" />
        </Turn>
        <Turn role="agent" time="14:31">
          <FileCard name="pairing-routes.md" size={8396n} at="14:31" />
        </Turn>
        <Turn role="agent" time="14:29" marked>
          <AskBlock
            prompt="Which route should own tethera://pair?"
            waiting="3m"
            options={[
              { label: "Rewrite before the router sees it.", detail: "One place to fix." },
              { label: "Register pair as a real route.", detail: "Fights the framework." },
              { label: "Handle it outside the router.", detail: "More code, fewer surprises." },
            ]}
          />
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
    <h2>Drawer, TabStrip, TerminalView, KeyBar</h2>
    <div class="gallery__panel gallery__phone gallery__tall">
      <Drawer
        label="tethera-3"
        summary="11 passed, 1 failed"
        height={drawer}
        onheight={(h) => (drawer = h)}
      >
        <TabStrip tabs={TABS} activeId={tab} onselect={(id) => (tab = id)} onadd={() => {}} />
        <TerminalView lines={TERM} cursor />
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
