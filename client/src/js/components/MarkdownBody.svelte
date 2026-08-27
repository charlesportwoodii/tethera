<script lang="ts">
  import { Markdown, TableView } from "$console";
  import { MarkdownTables } from "$managers/tables";

  interface Props {
    source: string;
    onlink?: ((href: string) => void) | null;
  }

  let { source, onlink = null }: Props = $props();

  const segments = $derived(MarkdownTables.split(source));
</script>

<!--
  Agent prose, with its tables drawn as tables.

  The console renderer has no table block, so a table arrives as a paragraph of
  pipes with its newlines collapsed. This splits the source and hands each piece
  to the component that can already draw it - prose to their markdown renderer,
  tables to their table - rather than parsing markdown a second time beside
  theirs.
-->
{#each segments as segment, index (index)}
  {#if segment.kind === "table"}
    <TableView columns={segment.columns} rows={segment.rows} />
  {:else}
    <Markdown source={segment.text} {onlink} />
  {/if}
{/each}
