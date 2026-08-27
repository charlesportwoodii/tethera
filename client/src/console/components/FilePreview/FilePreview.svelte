<script lang="ts">
  import BrailleSpinner from "$console/components/BrailleSpinner/BrailleSpinner.svelte";
  import Icon from "$console/components/Icon/Icon.svelte";
  import Markdown from "$console/components/Markdown/Markdown.svelte";
  import { previewKind } from "$console/lib/preview";
  import type { FilePreviewProps } from "./FilePreview.types";

  let {
    name,
    mime = null,
    text = null,
    imageUrl = null,
    truncated = false,
    kind,
  }: FilePreviewProps = $props();

  const resolved = $derived(kind ?? previewKind(name, mime));

  // Waiting is a different state from having nothing to show, and conflating them
  // tells the reader a file is unreadable while its first chunk is still moving.
  const waiting = $derived(
    resolved !== "none" && text === null && imageUrl === null,
  );

  type Tone = "add" | "del" | "hunk" | "plain";

  function classify(line: string): Tone {
    if (line.startsWith("+++") || line.startsWith("---") || line.startsWith("@@")) return "hunk";
    if (line.startsWith("+")) return "add";
    if (line.startsWith("-")) return "del";
    return "plain";
  }

  const diffLines = $derived(
    resolved === "diff" && text !== null
      ? text.split("\n").map((line) => ({ line, tone: classify(line) }))
      : [],
  );
</script>

<div class="tc-fp" data-kind={resolved}>
  {#if resolved === "none"}
    <div class="tc-fp__none">
      <Icon name="terminal" size={30} />
      <p>No preview for this kind of file. It stays on the machine until you ask for it.</p>
    </div>
  {:else if waiting}
    <div class="tc-fp__waiting">
      <BrailleSpinner label="Loading" />
      <p>Reading the first part of the file.</p>
    </div>
  {:else if resolved === "image" && imageUrl}
    <div class="tc-fp__image">
      <img src={imageUrl} alt={name} />
    </div>
  {:else if resolved === "markdown" && text !== null}
    <!-- The same parser the transcript uses. A second renderer here is the
         mistake that TranscriptPart already taught us. -->
    <div class="tc-fp__body"><Markdown source={text} /></div>
  {:else if resolved === "diff" && text !== null}
    <pre class="tc-fp__code">{#each diffLines as row, i (i)}<span
          class="tc-fp__line is-{row.tone}">{row.line}
</span>{/each}</pre>
  {:else if text !== null}
    <pre class="tc-fp__code">{text}</pre>
  {/if}

  {#if truncated && !waiting && resolved !== "none"}
    <span class="tc-fp__truncated">first part only · save the file to read it all</span>
  {/if}
</div>

<style lang="scss">
  @use "./FilePreview.scss";
</style>
