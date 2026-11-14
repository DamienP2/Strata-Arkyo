<script lang="ts" module>
  export interface Sort { key: string; dir: 1 | -1 }
</script>

<script lang="ts">
  // En-tête de colonne triable : un clic trie, un second inverse le sens.
  let { label, key, sort = $bindable(), cls = '' }: { label: string; key: string; sort: Sort; cls?: string } = $props();
  const on = $derived(sort.key === key);
  function click() { sort = on ? { key, dir: (sort.dir === 1 ? -1 : 1) as 1 | -1 } : { key, dir: 1 }; }
</script>

<th class={cls} aria-sort={on ? (sort.dir === 1 ? 'ascending' : 'descending') : 'none'}>
  <button type="button" class="th-sort" class:on onclick={click} title="Trier par {label.toLowerCase()}">
    {label}<span class="th-ind" aria-hidden="true">{on ? (sort.dir === 1 ? '▲' : '▼') : '↕'}</span>
  </button>
</th>
