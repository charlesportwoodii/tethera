<script lang="ts">
  import type { TableViewProps } from "./TableView.types";

  let { columns, rows, caption }: TableViewProps = $props();
</script>

<div class="tc-table">
  <table>
    {#if caption}<caption>{caption}</caption>{/if}
    <thead>
      <tr>
        {#each columns as column, i (i)}
          <th scope="col">{column}</th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each rows as row, r (r)}
        <tr>
          {#each row as cell, c (c)}
            <td>{cell}</td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style lang="scss">
  @use "./TableView.scss";
</style>
