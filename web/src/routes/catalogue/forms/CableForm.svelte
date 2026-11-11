<script lang="ts">
  import NumField from '../../../components/NumField.svelte';
  import ChipPicker from '../../../components/ChipPicker.svelte';
  import FieldError from '../../../components/FieldError.svelte';
  import ConnEnd from './ConnEnd.svelte';
  import { api } from '../../../lib/api';
  import { refList, refName, type CatalogItem } from '../../../lib/catalog.svelte';

  let { data = $bindable(), err }: { data: any; err: (f: string) => string | undefined } = $props();

  let models = $state<CatalogItem[]>([]);
  api.get<{ items: CatalogItem[] }>('/catalog/items?kind=cable-models&limit=1000').then((r) => (models = r.items)).catch(() => {});

  const proposed = $derived(`${refName('connectors', data.left.connector_id)} ${data.left.gender} ↔ ${refName('connectors', data.right.connector_id)} ${data.right.gender}`);
  const sigOptions = $derived(refList('signals', true).filter((s) => [data.left.connector_id, data.right.connector_id].some((c) => (s.data.connectors ?? []).includes(c))).map((s) => ({ id: s.id, name: s.name, color: s.data.color })));

  let lengths = $state((data.lengths_m ?? []).map((l: number) => String(l).replace('.', ',')).join(' ; '));
  $effect(() => { data.lengths_m = lengths.split(/[;\s]+/).map((s: string) => Number(s.replace(',', '.'))).filter((n: number) => !Number.isNaN(n) && n > 0); });
  const dims = $derived(refList('dimensions', true).filter((d) => d.data.kind === 'cable_length_m').map((d) => d.data.value as number));

  // Compatibilité du modèle choisi avec chaque usage (impédance puis bande passante).
  let compat = $state<Record<string, { result: string; message: string }>>({});
  $effect(() => {
    compat = {};
    const m = data.model_id, sigs = [...data.usages];
    if (!m) return;
    for (const s of sigs) api.get<{ result: string; message: string }>(`/catalog/compat?model_id=${m}&signal_id=${s}`).then((r) => (compat[s] = r)).catch(() => {});
  });
  const CMP: Record<string, string> = { compatible: 'ok', warning: 'warn', incompatible: 'err', unknown: '' };
</script>

<section class="card">
  <h2>Identité</h2>
  <div class="form-grid">
    <div class="field span"><label for="c-name">Nom</label><input class="input" id="c-name" bind:value={data.name} placeholder={`Vide : « ${proposed} »`}><span class="help">Laissez vide pour reprendre les connecteurs.</span><FieldError message={err('name')} /></div>
    <div class="field"><label for="c-ref">Référence</label><input class="input mono" id="c-ref" value={data.reference ?? ''} oninput={(e) => (data.reference = (e.target as HTMLInputElement).value || null)}></div>
    <div class="field"><label for="c-brand">Marque</label><select class="input" id="c-brand" value={data.brand_id ?? ''} onchange={(e) => (data.brand_id = (e.target as HTMLSelectElement).value || null)}><option value="">—</option>{#each refList('brands', true) as b}<option value={b.id}>{b.name}</option>{/each}</select></div>
    <div class="field"><label for="c-fn">Fonction</label><select class="input" id="c-fn" value={data.function ?? ''} onchange={(e) => (data.function = (e.target as HTMLSelectElement).value || null)}><option value="">—</option>{#each refList('functions', true) as f}<option value={f.id}>{f.name}</option>{/each}</select></div>
    <div class="field"><label for="c-model">Modèle de câble</label><select class="input" class:err={!!err('model_id')} id="c-model" value={data.model_id ?? ''} onchange={(e) => (data.model_id = (e.target as HTMLSelectElement).value || null)}><option value="">—</option>{#each models as m}<option value={m.id}>{m.name}</option>{/each}</select><FieldError message={err('model_id')} /></div>
    <div class="field" style="justify-content:flex-end"><label class="toggle-row"><input type="checkbox" checked={data.active !== false} onchange={(e) => (data.active = (e.target as HTMLInputElement).checked)}>Fiche active</label></div>
  </div>
</section>

<section class="card">
  <h2>Extrémités</h2>
  <div class="form-grid">
    <ConnEnd id="c-left" label="Côté gauche" bind:value={data.left} error={err('left')} />
    <ConnEnd id="c-right" label="Côté droit" bind:value={data.right} error={err('right')} />
    <div class="field span"><span class="flabel">Signaux transportés</span>
      {#if sigOptions.length}<ChipPicker options={sigOptions} bind:value={data.usages} label="Signaux" />{:else}<span class="muted small">Aucun signal n’utilise ces connecteurs.</span>{/if}
      {#if data.model_id && data.usages.length}
        <div style="display:flex;gap:8px;flex-wrap:wrap;margin-top:6px">{#each data.usages as s}{#if compat[s]}<span class="chip {CMP[compat[s].result]}" title={compat[s].message}>{refName('signals', s)} : {compat[s].message}</span>{/if}{/each}</div>
      {/if}
    </div>
  </div>
</section>

<section class="card">
  <h2>Longueurs</h2>
  <div class="form-grid">
    <div class="field span"><label for="c-len">Longueurs disponibles (m)</label><input class="input mono" id="c-len" bind:value={lengths} placeholder="ex. 1 ; 3 ; 5 ; 10" list="c-len-list"><datalist id="c-len-list">{#each dims as d}<option value={String(d).replace('.', ',')}></option>{/each}</datalist><FieldError message={err('lengths_m')} /></div>
    <NumField id="c-min" label="Sur mesure : minimum" unit="m" bind:value={data.custom_min_m} error={err('custom_min_m')} />
    <NumField id="c-max" label="Sur mesure : maximum" unit="m" bind:value={data.custom_max_m} error={err('custom_max_m')} />
    <div class="field span"><label for="c-notes">Notes</label><textarea class="input" id="c-notes" rows="3" style="height:auto;padding:12px 14px" value={data.notes ?? ''} oninput={(e) => (data.notes = (e.target as HTMLTextAreaElement).value || null)}></textarea></div>
  </div>
</section>
