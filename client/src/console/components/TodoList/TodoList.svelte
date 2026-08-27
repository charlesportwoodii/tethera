<script lang="ts">
  import type { TodoListProps } from "./TodoList.types";

  let { items, label = "Plan" }: TodoListProps = $props();

  const done = $derived(items.filter((i) => i.status === "done").length);
</script>

<div class="tc-todo">
  <div class="tc-todo__head">
    <span>{label}</span>
    <span class="tc-todo__count">{done} of {items.length}</span>
  </div>
  <ul style="margin:0;padding:0;list-style:none">
    {#each items as item, i (i)}
      <li class="tc-todo__item is-{item.status}" data-status={item.status}>
        <span class="tc-todo__mark" aria-hidden="true"></span>
        <span>{item.text}</span>
      </li>
    {/each}
  </ul>
</div>

<style lang="scss">
  @use "./TodoList.scss";
</style>
