<script lang="ts">
  // Choix multiple par puces : chaque option est une touche qui s'allume.
  let { options, value = $bindable([]), label, disabled = false }:
    { options: { id: string; name: string; color?: string }[]; value: string[]; label: string; disabled?: boolean } = $props();
  function toggle(id: string) { value = value.includes(id) ? value.filter((v) => v !== id) : [...value, id]; }
</script>

<div class="chip-pick" role="group" aria-label={label}>
  {#each options as o (o.id)}
    <button type="button" class="fchip" class:on={value.includes(o.id)} style={o.color ? `--c:${o.color}` : undefined} aria-pressed={value.includes(o.id)} {disabled} onclick={() => toggle(o.id)}>{o.name}</button>
  {/each}
</div>

<style>
  .chip-pick { display: flex; flex-wrap: wrap; gap: 8px; }
</style>
