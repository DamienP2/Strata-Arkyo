<script lang="ts">
  import NumField from '../../../components/NumField.svelte';
  import FieldError from '../../../components/FieldError.svelte';
  import { refList } from '../../../lib/catalog.svelte';
  let { data = $bindable(), err }: { data: any; err: (f: string) => string | undefined } = $props();
  const txt = (k: string) => (e: Event) => (data[k] = (e.target as HTMLInputElement).value || null);
</script>

<section class="card">
  <h2>Identité</h2>
  <div class="form-grid">
    <div class="field span"><label for="m-name">Nom</label><input class="input" class:err={!!err('name')} id="m-name" bind:value={data.name} required placeholder="Ex. Coax 75 Ω 12G-SDI"><FieldError message={err('name')} /></div>
    <div class="field"><label for="m-ref">Référence</label><input class="input mono" id="m-ref" value={data.reference ?? ''} oninput={txt('reference')}></div>
    <div class="field"><label for="m-brand">Marque</label><select class="input" id="m-brand" value={data.brand_id ?? ''} onchange={(e) => (data.brand_id = (e.target as HTMLSelectElement).value || null)}><option value="">—</option>{#each refList('brands', true) as b}<option value={b.id}>{b.name}</option>{/each}</select></div>
    <div class="field"><label for="m-fn">Fonction</label><select class="input" id="m-fn" value={data.function ?? ''} onchange={(e) => (data.function = (e.target as HTMLSelectElement).value || null)}><option value="">—</option>{#each refList('functions', true) as f}<option value={f.id}>{f.name}</option>{/each}</select></div>
    <div class="field" style="justify-content:flex-end"><label class="toggle-row"><input type="checkbox" checked={data.active !== false} onchange={(e) => (data.active = (e.target as HTMLInputElement).checked)}>Fiche active</label></div>
  </div>
</section>
<section class="card">
  <h2>Caractéristiques</h2>
  <div class="fg3" style="margin-top:16px">
    <NumField id="m-imp" label="Impédance" unit="Ω" bind:value={data.impedance_ohm} error={err('impedance_ohm')} suggestions={[50, 75, 100, 110]} help="Comparée à celle des signaux." />
    <NumField id="m-bw" label="Bande passante" unit="MHz" bind:value={data.bandwidth_mhz} error={err('bandwidth_mhz')} />
    <NumField id="m-dia" label="Diamètre" unit="mm" bind:value={data.diameter_mm} error={err('diameter_mm')} />
    <div class="field"><label for="m-cond">Âme</label><input class="input" id="m-cond" value={data.conductor ?? ''} oninput={txt('conductor')} placeholder="ex. cuivre plein 1,0 mm"></div>
    <div class="field"><label for="m-shield">Blindage</label><input class="input" id="m-shield" value={data.shielding ?? ''} oninput={txt('shielding')} placeholder="ex. double tresse"></div>
    <div class="field"><label for="m-jacket">Gaine</label><input class="input" id="m-jacket" value={data.jacket ?? ''} oninput={txt('jacket')} placeholder="ex. PVC, LSZH"></div>
    <div class="field"><label for="m-fire">Classement feu</label><input class="input" id="m-fire" value={data.fire_rating ?? ''} oninput={txt('fire_rating')} placeholder="ex. Cca-s1,d1,a1"></div>
    <div class="field" style="grid-column:span 2"><label for="m-ds">Fiche technique</label><input class="input mono" class:err={!!err('datasheet_url')} id="m-ds" value={data.datasheet_url ?? ''} oninput={txt('datasheet_url')} placeholder="https://…"><FieldError message={err('datasheet_url')} /></div>
  </div>
</section>
