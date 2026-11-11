<script lang="ts">
  import Icon from '../../../components/Icon.svelte';
  import FieldError from '../../../components/FieldError.svelte';
  import ConnEnd from './ConnEnd.svelte';
  import { refList } from '../../../lib/catalog.svelte';
  let { data = $bindable(), err }: { data: any; err: (f: string) => string | undefined } = $props();
  const add = () => (data.outputs = [...data.outputs, { connector_id: data.outputs.at(-1)?.connector_id ?? 'bnc', gender: 'M', quantity: 1, label: null }]);
</script>

<section class="card">
  <h2>Identité</h2>
  <div class="form-grid">
    <div class="field span"><label for="a-name">Nom</label><input class="input" class:err={!!err('name')} id="a-name" bind:value={data.name} required placeholder="Ex. BNC F vers HD-BNC M"><FieldError message={err('name')} /></div>
    <div class="field"><label for="a-ref">Référence</label><input class="input mono" id="a-ref" value={data.reference ?? ''} oninput={(e) => (data.reference = (e.target as HTMLInputElement).value || null)}></div>
    <div class="field"><label for="a-fn">Fonction</label><select class="input" id="a-fn" value={data.function ?? ''} onchange={(e) => (data.function = (e.target as HTMLSelectElement).value || null)}><option value="">—</option>{#each refList('functions', true) as f}<option value={f.id}>{f.name}</option>{/each}</select></div>
    <div class="field span"><label class="toggle-row"><input type="checkbox" checked={data.active !== false} onchange={(e) => (data.active = (e.target as HTMLInputElement).checked)}>Fiche active</label></div>
  </div>
</section>
<section class="card">
  <div class="card-head"><h2>Entrée et sorties</h2><button type="button" class="btn sm" onclick={add}><Icon name="plus" size="sm" />Ajouter une sortie</button></div>
  <div class="stack" style="margin-top:16px">
    <ConnEnd id="a-in" label="Entrée" bind:value={data.input} error={err('input')} />
    {#each data.outputs as o, i}
      <div style="display:grid;grid-template-columns:1fr 90px auto;gap:10px;align-items:end">
        <ConnEnd id="a-out-{i}" label="Sortie {i + 1}" bind:value={data.outputs[i]} error={err(`outputs[${i}]`)} />
        <div class="field"><label for="a-q-{i}">Nombre</label><input class="input mono" id="a-q-{i}" type="number" min="1" bind:value={o.quantity}></div>
        <button type="button" class="btn sm ghost danger" title="Retirer" disabled={data.outputs.length < 2} onclick={() => (data.outputs = data.outputs.filter((_: unknown, j: number) => j !== i))} style="margin-bottom:4px"><Icon name="x" size="sm" /></button>
      </div>
    {/each}
    <FieldError message={err('outputs')} />
    <div class="field"><label for="a-notes">Notes</label><textarea class="input" id="a-notes" rows="3" style="height:auto;padding:12px 14px" value={data.notes ?? ''} oninput={(e) => (data.notes = (e.target as HTMLTextAreaElement).value || null)}></textarea></div>
  </div>
</section>
