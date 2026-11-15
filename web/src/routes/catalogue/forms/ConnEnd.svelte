<script lang="ts">
  // Une extrémité : connecteur et genre (M, F, ou N pour neutre).
  import FieldError from '../../../components/FieldError.svelte';
  import Seg from '../../../components/Seg.svelte';
  import { refList } from '../../../lib/catalog.svelte';
  let { value = $bindable(), label, id, error, genders = ['M', 'F', 'N'] }:
    { value: { connector_id: string; gender: string; label?: string | null }; label: string; id: string; error?: string; genders?: string[] } = $props();
</script>

<div class="field">
  <label for={id}>{label}</label>
  <div style="display:grid;grid-template-columns:1fr auto;gap:8px">
    <select class="input" class:err={!!error} {id} bind:value={value.connector_id}>{#each refList('connectors', true) as c}<option value={c.id}>{c.name}</option>{/each}</select>
    <!-- « Neutre » en toutes lettres : « N » se confondrait avec le connecteur RF de type N. -->
    <Seg label="Genre" bind:value={value.gender} options={genders.map((g) => ({ value: g, label: g === 'N' ? 'Neutre' : g, title: g === 'M' ? 'Mâle' : g === 'F' ? 'Femelle' : 'Neutre (ni mâle ni femelle)' }))} />
  </div>
  <FieldError message={error} />
</div>
