
<script>
  import { invoke } from '@tauri-apps/api/core';
  export let node;
  export let parentArray;
  export let index;
  export let toggleNode;
  export let openItem;
</script>

<div class="folder-row" on:click={() => toggleNode(node, parentArray, index)}>
  <span>{node.expanded ? '📂' : '📁'} {node.name}</span>
</div>

{#if node.expanded}
  <ul>
    {#each node.children as child, i (child.path)}
      {#if child.is_dir}
        <svelte:self node={child} parentArray={node.children} index={i} {toggleNode} {openItem} />
      {:else}
        <li class="file-row" on:click|stopPropagation={() => openItem(child.path)}>
          📄 {child.name}
        </li>
      {/if}
    {/each}
  </ul>
{/if}