<script lang="ts">
  import QuestionCard from "$console/components/QuestionCard/QuestionCard.svelte";
  import DiffView from "$console/components/DiffView/DiffView.svelte";
  import FileCard from "$console/components/FileCard/FileCard.svelte";
  import Markdown from "$console/components/Markdown/Markdown.svelte";
  import StatusLine from "$console/components/StatusLine/StatusLine.svelte";
  import TableView from "$console/components/TableView/TableView.svelte";
  import TodoList from "$console/components/TodoList/TodoList.svelte";
  import ToolFold from "$console/components/ToolFold/ToolFold.svelte";
  import { previewKind } from "$console/lib/preview";
  import type { PartViewProps } from "./PartView.types";

  let {
    part,
    at = null,
    waiting = null,
    expanded = false,
    onexpandquestion = null,
    onopenfile,
    imageUrl = null,
    onlink = null,
    ontool,
    ontoggle,
  }: PartViewProps = $props();
</script>

<!--
  The whole mapping from wire to screen. The part set is closed and externally
  tagged, so each branch is exactly one variant — and a variant this build has
  never heard of falls through to "unknown", which carries the source rows
  verbatim rather than disappearing.
-->
{#if "text" in part}
  <Markdown source={part.text.text} {onlink} />
{:else if "tool_use" in part}
  <!--
    The fold and its body together. A fold with nothing under it advertises
    detail it cannot show: the chevron moves, expanded flips, and visibly nothing
    happens. The result is what a reader wants; the input is the fallback for a
    call that is still running and has none yet.
  -->
  <ToolFold
    name={part.tool_use.name}
    detail={part.tool_use.result}
    status={part.tool_use.status}
    {expanded}
    onclick={() => ontool?.(part.tool_use.name)}
  />
  {#if expanded}
    <pre class="tc-part__body">{part.tool_use.result ?? part.tool_use.input}</pre>
  {/if}
{:else if "diff" in part}
  <DiffView
    path={part.diff.path}
    unified={part.diff.unified}
    added={part.diff.added}
    removed={part.diff.removed}
    open={expanded}
    {ontoggle}
  />
{:else if "todo" in part}
  <TodoList items={part.todo.items} />
{:else if "table" in part}
  <TableView columns={part.table.columns} rows={part.table.rows} />
{:else if "status" in part}
  <StatusLine label={part.status.label} detail={part.status.detail} />
{:else if "file" in part}
  <!--
    A picture reads as a picture. The decision of what counts as one is
    previewKind's, not this branch's: it is where the rule lives that
    image/svg+xml is a document that can carry script and renders as a card,
    never as an image. Duplicating the mime check here is how that rule would be
    forgotten in the one branch that is mostly about pictures.
  -->
  {#if previewKind(part.file.name, part.file.mime) === "image" && imageUrl}
    <button
      class="tc-part__thumb"
      type="button"
      onclick={() => onopenfile?.(part.file.asset, part.file.name)}
    >
      <img src={imageUrl} alt={part.file.name} loading="lazy" />
      <span class="tc-part__thumbname">{part.file.name}</span>
    </button>
  {:else}
    <FileCard
      name={part.file.name}
      size={part.file.size}
      {at}
      ondownload={() => onopenfile?.(part.file.asset, part.file.name)}
    />
  {/if}
{:else if "question" in part}
  {#if part.question.answered}
    <!-- Re-offering the options would invite a second answer to a settled question. -->
    <StatusLine
      label="answered"
      detail={part.question.question.asks[0]?.header ?? part.question.question.asks[0]?.prompt}
    />
  {:else}
    <QuestionCard
      question={part.question.question}
      {waiting}
      onopen={onexpandquestion}
    />
  {/if}
{:else if "unknown" in part}
  <pre class="tc-part__fallback" data-kind={part.unknown.kind}>{part.unknown.fallback_text}</pre>
{/if}

<style lang="scss">
  @use "./PartView.scss";
</style>
