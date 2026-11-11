<script lang="ts">
  // Une extrémité : connecteur et genre (M, F, ou N pour neutre).
  import FieldError from '../../../components/FieldError.svelte';
  import { refList } from '../../../lib/catalog.svelte';
  let { value = $bindable(), label, id, error, genders = ['M', 'F', 'N'] }:
    { value: { connector_id: string; gender: string; label?: string | null }; label: string; id: string; error?: string; genders?: string[] } = $props();
</script>

<div class="field">
  <label for={id}>{label}</label>
  <div style="display:grid;grid-template-columns:1fr auto;gap:8px">
    <select class="input" class:err={!!error} {id} bind:value={value.connector_id}>{#each refList('connectors', true) as c}<option value={c.id}>{c.name}</option>{/each}</select>
    <div class="seg" role="group" aria-label="Genre">{#each genders as g}<button type="button" class:on={value.gender === g} onclick={() => (value.gender = g)} title={g === 'M' ? 'Mâle' : g === 'F' ? 'Femelle' : 'Neutre'}>{g}</button>{/each}</div>
  </div>
  <FieldError message={error} />
</div>
