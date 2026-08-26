<script lang="ts">
  import AskBlock from "$console/components/AskBlock/AskBlock.svelte";
  import FileCard from "$console/components/FileCard/FileCard.svelte";
  import ToolFold from "$console/components/ToolFold/ToolFold.svelte";
  import type { PartViewProps } from "./PartView.types";

  let {
    part,
    at = null,
    waiting = null,
    fingerprint = null,
    onanswer,
    ondownload,
    ontool,
  }: PartViewProps = $props();
</script>

<!--
  The part set is closed and externally tagged, so this is the whole mapping from
  wire to screen. A type this build has never heard of falls through to
  fallback_text rather than disappearing.
-->
{#if "text" in part}
  <p class="tc-part__text">{part.text.text}</p>
{:else if "tool_use" in part}
  <ToolFold name={part.tool_use.name} onclick={() => ontool?.(part.tool_use.name)} />
{:else if "question" in part}
  <AskBlock
    prompt={part.question.prompt}
    options={part.question.options}
    {waiting}
    {fingerprint}
    {onanswer}
  />
{:else if "file" in part}
  <FileCard
    name={part.file.name}
    size={part.file.size}
    {at}
    ondownload={() => ondownload?.(part.file.name)}
  />
{:else if "unknown" in part}
  <pre class="tc-part__fallback" data-kind={part.unknown.kind}>{part.unknown.fallback_text}</pre>
{/if}

<style lang="scss">
  @use "./PartView.scss";
</style>
