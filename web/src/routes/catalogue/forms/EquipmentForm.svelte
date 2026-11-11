<script lang="ts">
  import Icon from '../../../components/Icon.svelte';
  import NumField from '../../../components/NumField.svelte';
  import ChipPicker from '../../../components/ChipPicker.svelte';
  import FieldError from '../../../components/FieldError.svelte';
  import { refList, refById, FACES, DIRECTIONS, WIDTHS } from '../../../lib/catalog.svelte';
  import { nf } from '../../../lib/format';

  let { data = $bindable(), err }: { data: any; err: (f: string) => string | undefined } = $props();

  const dims = (k: string) => refList('dimensions', true).filter((d) => d.data.kind === k).map((d) => d.data.value as number);
  const fnOptions = $derived(refList('functions', true).map((f) => ({ id: f.id, name: f.name, color: f.data.color })));
  const phaseOptions = $derived(refList('phases', true).map((p) => ({ id: p.id, name: p.name })));
  const types = $derived(refList('types', true));
  const topTypes = $derived(types.filter((t) => !t.parent_id));
  const categories = $derived(refList('categories', true));
  const connectors = $derived(refList('connectors', true));
  const signals = $derived(refList('signals', true));

  let tags = $state((data.tags ?? []).join(', '));
  $effect(() => { data.tags = tags.split(/[,;]/).map((t: string) => t.trim()).filter(Boolean); });

  // Le type impose sa fonction (règle du moteur) : on l'ajoute pour la personne.
  $effect(() => {
    const t = refById('types', data.type_id);
    if (!t) return;
    const f = t.parent_id ? refById('types', t.parent_id)?.data.function : t.data.function;
    if (f && !data.functions.includes(f)) data.functions = [...data.functions, f];
  });

  function setFormat(kind: 'rack' | 'free') {
    if (data.format.kind === kind) return;
    data.format = kind === 'rack' ? { kind, height_u: 1, width: 'full', depth_mm: 0, rear_reserve_mm: null } : { kind, length_mm: 0, width_mm: 0, height_mm: 0 };
  }
  const suggestedBtu = $derived(data.power_w ? Math.round(data.power_w * 3.412) : null);

  function addPort() {
    const last = data.ports.at(-1);
    data.ports = [...data.ports, last ? { ...last, label: null } : { face: 'rear', connector_id: 'bnc', variant: null, signal_id: null, direction: 'in', gender: 'F', quantity: 1, label: null, network_role: null, redundancy: true }];
  }
  const removePort = (i: number) => (data.ports = data.ports.filter((_: unknown, j: number) => j !== i));
  const sigsFor = (c: string) => signals.filter((s) => (s.data.connectors ?? []).includes(c));
  const variantsOf = (c: string): string[] => refById('connectors', c)?.data.variants ?? [];
  const totalPorts = $derived(data.ports.reduce((s: number, p: any) => s + (Number(p.quantity) || 0), 0));

  let dmxOn = $state(!!data.dmx);
  $effect(() => { if (dmxOn && !data.dmx) data.dmx = { default_channels: 1, modes: [] }; if (!dmxOn && data.dmx) data.dmx = null; });
  const addDoc = () => (data.documents = [...data.documents, { title: '', url: '' }]);
</script>

<section class="card">
  <h2>Identité</h2>
  <div class="form-grid">
    <div class="field span"><label for="e-name">Nom</label><input class="input" class:err={!!err('name')} id="e-name" bind:value={data.name} required placeholder="Ex. Mélangeur 2 M/E Constellation HD"><FieldError message={err('name')} /></div>
    <div class="field"><label for="e-ref">Référence fabricant</label><input class="input mono" id="e-ref" value={data.reference ?? ''} oninput={(e) => (data.reference = (e.target as HTMLInputElement).value || null)}><FieldError message={err('reference')} /></div>
    <div class="field"><label for="e-brand">Marque</label><select class="input" id="e-brand" value={data.brand_id ?? ''} onchange={(e) => (data.brand_id = (e.target as HTMLSelectElement).value || null)}><option value="">—</option>{#each refList('brands', true) as b}<option value={b.id}>{b.name}</option>{/each}</select><FieldError message={err('brand_id')} /></div>
    <div class="field"><label for="e-cat">Catégorie</label><select class="input" id="e-cat" value={data.category_id ?? ''} onchange={(e) => (data.category_id = (e.target as HTMLSelectElement).value || null)}><option value="">—</option>{#each categories as c}<option value={c.id}>{c.parent_id ? '— ' : ''}{c.name}</option>{/each}</select><FieldError message={err('category_id')} /></div>
    <div class="field"><label for="e-type">Type</label><select class="input" id="e-type" value={data.type_id ?? ''} onchange={(e) => (data.type_id = (e.target as HTMLSelectElement).value || null)}><option value="">—</option>{#each topTypes as t}<option value={t.id}>{t.name}</option>{#each types.filter((s) => s.parent_id === t.id) as s}<option value={s.id}>— {s.name}</option>{/each}{/each}</select><FieldError message={err('type_id')} /></div>
    <div class="field span"><span class="flabel">Fonctions</span><ChipPicker options={fnOptions} bind:value={data.functions} label="Fonctions" /><FieldError message={err('functions')} /></div>
    <div class="field span"><span class="flabel">Phases</span><ChipPicker options={phaseOptions} bind:value={data.phases} label="Phases" /></div>
    <div class="field"><label for="e-tags">Étiquettes</label><input class="input" id="e-tags" bind:value={tags} placeholder="séparées par des virgules"></div>
    <div class="field" style="justify-content:flex-end"><label class="toggle-row"><input type="checkbox" checked={data.active !== false} onchange={(e) => (data.active = (e.target as HTMLInputElement).checked)}>Fiche active (proposée dans les projets)</label></div>
    <div class="field span"><label for="e-notes">Notes</label><textarea class="input" id="e-notes" rows="3" style="height:auto;padding:12px 14px" value={data.notes ?? ''} oninput={(e) => (data.notes = (e.target as HTMLTextAreaElement).value || null)}></textarea></div>
  </div>
</section>

<section class="card">
  <div class="card-head"><h2>Format</h2><div class="seg"><button type="button" class:on={data.format.kind === 'rack'} onclick={() => setFormat('rack')}>Rack 19″</button><button type="button" class:on={data.format.kind === 'free'} onclick={() => setFormat('free')}>Hors rack</button></div></div>
  {#if data.format.kind === 'rack'}
    <div class="fg4" style="margin-top:16px">
      <NumField id="e-u" label="Hauteur" unit="U" bind:value={data.format.height_u} suggestions={dims('height_u')} error={err('format.height_u')} required />
      <div class="field"><span class="flabel">Largeur</span><div class="seg">{#each WIDTHS as [k, l]}<button type="button" class:on={data.format.width === k} onclick={() => (data.format.width = k)}>{l}</button>{/each}</div></div>
      <NumField id="e-depth" label="Profondeur" unit="mm" bind:value={data.format.depth_mm} suggestions={dims('depth_mm')} error={err('format.depth_mm')} />
      <NumField id="e-rear" label="Réserve arrière" unit="mm" bind:value={data.format.rear_reserve_mm} suggestions={dims('rear_reserve_mm')} error={err('format.rear_reserve_mm')} help="Place pour les câbles derrière l’appareil." />
    </div>
  {:else}
    <div class="fg3" style="margin-top:16px">
      <NumField id="e-l" label="Longueur" unit="mm" bind:value={data.format.length_mm} error={err('format.length_mm')} />
      <NumField id="e-w" label="Largeur" unit="mm" bind:value={data.format.width_mm} error={err('format.width_mm')} />
      <NumField id="e-h" label="Hauteur" unit="mm" bind:value={data.format.height_mm} error={err('format.height_mm')} />
    </div>
  {/if}
</section>

<section class="card">
  <h2>Poids et énergie</h2>
  <div class="fg4" style="margin-top:16px">
    <NumField id="e-kg" label="Poids" unit="kg" bind:value={data.weight_kg} error={err('weight_kg')} />
    <NumField id="e-pw" label="Consommation" unit="W" bind:value={data.power_w} error={err('power_w')} />
    <NumField id="e-btu" label="Dissipation" unit="BTU/h" bind:value={data.btu_h} error={err('btu_h')} help={suggestedBtu && data.btu_h == null ? `Estimée : ${nf(suggestedBtu)} BTU/h` : undefined} />
    <div class="field" style="justify-content:flex-end;gap:10px">
      {#if suggestedBtu && data.btu_h == null}<button type="button" class="btn sm" onclick={() => (data.btu_h = suggestedBtu)}>Reprendre l’estimation</button>{/if}
      <label class="toggle-row"><input type="checkbox" bind:checked={data.dual_psu}>Double alimentation</label>
    </div>
  </div>
</section>

<section class="card">
  <div class="card-head"><div><h2>Ports</h2><span class="muted small">{totalPorts} port{totalPorts > 1 ? 's' : ''} en {data.ports.length} groupe{data.ports.length > 1 ? 's' : ''}</span></div><button type="button" class="btn sm" onclick={addPort}><Icon name="plus" size="sm" />Ajouter un groupe</button></div>
  {#if data.ports.length}
    <div class="tbl-wrap">
      <table class="tbl ports-tbl">
        <thead><tr><th>Face</th><th>Connecteur</th><th>Variante</th><th>Signal</th><th>Sens</th><th>Genre</th><th>Nombre</th><th>Libellé</th><th></th></tr></thead>
        <tbody>
          {#each data.ports as p, i}
            <tr>
              <td><select class="input" bind:value={p.face} aria-label="Face">{#each FACES as [k, l]}<option value={k}>{l}</option>{/each}</select></td>
              <td><select class="input" class:err={!!err(`ports[${i}].connector_id`)} bind:value={p.connector_id} aria-label="Connecteur" onchange={() => { p.variant = null; if (p.signal_id && !sigsFor(p.connector_id).some((s) => s.id === p.signal_id)) p.signal_id = null; }}>{#each connectors as c}<option value={c.id}>{c.name}</option>{/each}</select></td>
              <td><select class="input" value={p.variant ?? ''} aria-label="Variante" disabled={!variantsOf(p.connector_id).length} onchange={(e) => (p.variant = (e.target as HTMLSelectElement).value || null)}><option value="">—</option>{#each variantsOf(p.connector_id) as v}<option value={v}>{v}</option>{/each}</select></td>
              <td><select class="input" class:err={!!err(`ports[${i}].signal_id`)} value={p.signal_id ?? ''} aria-label="Signal" onchange={(e) => (p.signal_id = (e.target as HTMLSelectElement).value || null)}><option value="">—</option>{#each sigsFor(p.connector_id) as s}<option value={s.id}>{s.name}</option>{/each}</select></td>
              <td><select class="input" bind:value={p.direction} aria-label="Sens">{#each DIRECTIONS as [k, l]}<option value={k}>{l}</option>{/each}</select></td>
              <td><select class="input" bind:value={p.gender} aria-label="Genre"><option value="F">F</option><option value="M">M</option></select></td>
              <td><input class="input mono qty" type="number" min="1" max="1024" bind:value={p.quantity} aria-label="Nombre"></td>
              <td><input class="input" value={p.label ?? ''} aria-label="Libellé" placeholder="ex. SDI IN" oninput={(e) => (p.label = (e.target as HTMLInputElement).value || null)}></td>
              <td><button type="button" class="btn sm ghost danger" title="Retirer" onclick={() => removePort(i)}><Icon name="x" size="sm" /></button></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <FieldError message={err('ports')} />
  {:else}<p class="empty">Aucun port. Ajoutez un groupe par type de connecteur et de signal (ex. 12 × BNC, SDI 3G, entrée).</p>{/if}
</section>

<section class="card">
  <div class="card-head"><h2>Lumières (DMX)</h2><label class="toggle-row"><input type="checkbox" bind:checked={dmxOn}>Pilotable en DMX</label></div>
  {#if dmxOn && data.dmx}
    <div class="fg3" style="margin-top:16px"><NumField id="e-dmx" label="Canaux par défaut" bind:value={data.dmx.default_channels} error={err('dmx')} /></div>
  {/if}
</section>

<section class="card">
  <div class="card-head"><h2>Documents</h2><button type="button" class="btn sm" onclick={addDoc}><Icon name="plus" size="sm" />Ajouter un lien</button></div>
  <div class="stack" style="margin-top:12px">
    {#each data.documents as d, i}
      <div style="display:grid;grid-template-columns:1fr 2fr auto;gap:10px">
        <input class="input" bind:value={d.title} placeholder="Titre, ex. Manuel" aria-label="Titre du document">
        <input class="input mono" class:err={!!err(`documents[${i}]`)} bind:value={d.url} placeholder="https://…" aria-label="Lien du document">
        <button type="button" class="btn sm ghost danger" title="Retirer" onclick={() => (data.documents = data.documents.filter((_: unknown, j: number) => j !== i))}><Icon name="x" size="sm" /></button>
      </div>
    {:else}<p class="muted small">Manuel, fiche technique, page du fabricant…</p>{/each}
    <FieldError message={err('documents')} />
  </div>
</section>
