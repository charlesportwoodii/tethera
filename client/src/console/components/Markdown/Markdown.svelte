<script lang="ts">
  import { parseMarkdown, type Inline } from "$console/lib/markdown";
  import type { MarkdownProps } from "./Markdown.types";

  let { source, onlink = null }: MarkdownProps = $props();

  const blocks = $derived(parseMarkdown(source));
</script>

<!--
  Every branch below emits an element this file names. There is no {@html} here
  and no HTML string upstream of it, so a tag in the agent's text is text: the
  pipeline cannot express an injected element, which is a stronger guarantee than
  sanitising one away. See lib/markdown.ts.
-->
{#snippet inline(nodes: Inline[])}
  {#each nodes as node, i (i)}
    {#if node.kind === "text"}{node.text}{:else if node.kind === "code"}<code
        class="tc-md__code">{node.text}</code
      >{:else if node.kind === "strong"}<strong>{@render inline(node.children)}</strong
      >{:else if node.kind === "em"}<em>{@render inline(node.children)}</em
      >{:else if node.kind === "strike"}<del>{@render inline(node.children)}</del
      >{:else if node.kind === "link"}<button
        class="tc-md__link"
        type="button"
        data-href={node.href}
        title={node.href}
        disabled={onlink === null}
        onclick={() => onlink?.(node.href)}>{@render inline(node.children)}</button
      >{/if}
  {/each}
{/snippet}

<div class="tc-md">
  {#each blocks as block, i (i)}
    {#if block.kind === "paragraph"}
      <p>{@render inline(block.inline)}</p>
    {:else if block.kind === "heading"}
      <p class="tc-md__h is-{block.level}" role="heading" aria-level={block.level}>
        {@render inline(block.inline)}
      </p>
    {:else if block.kind === "code"}
      <pre class="tc-md__pre">{#if block.lang}<span class="tc-md__lang"
          >{block.lang}</span
        >{/if}{block.text}</pre>
    {:else if block.kind === "list"}
      {#if block.ordered}
        <ol>
          {#each block.items as item, j (j)}
            <li>{@render inline(item)}</li>
          {/each}
        </ol>
      {:else}
        <ul>
          {#each block.items as item, j (j)}
            <li>{@render inline(item)}</li>
          {/each}
        </ul>
      {/if}
    {:else if block.kind === "quote"}
      <blockquote class="tc-md__quote">{@render inline(block.inline)}</blockquote>
    {:else if block.kind === "rule"}
      <hr class="tc-md__rule" />
    {/if}
  {/each}
</div>

<style lang="scss">
  @use "./Markdown.scss";
</style>
