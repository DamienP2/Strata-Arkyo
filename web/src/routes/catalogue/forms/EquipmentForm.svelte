<script lang="ts">
  import Icon from '../../../components/Icon.svelte';
  import NumField from '../../../components/NumField.svelte';
  import ChipPicker from '../../../components/ChipPicker.svelte';
  import FieldError from '../../../components/FieldError.svelte';
  import SynNode from '../../../components/SynNode.svelte';
  import { refList, refById, refName, FACES, WIDTHS } from '../../../lib/catalog.svelte';
  import { nf } from '../../../lib/format';
  import { VISUALS, visualOf, decor, ALU } from '../../../lib/faceplate';
  import { api } from '../../../lib/api';

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

  /** Nouveau groupe du côté voulu, sur le modèle du dernier groupe de ce côté. */
  function addPort(direction: 'in' | 'out' | 'bidir') {
    const same = data.ports.filter((p: any) => (direction === 'in' ? p.direction === 'in' : p.direction !== 'in')).at(-1) ?? data.ports.at(-1);
    const base = same ? { ...same, label: null } : { face: 'rear', connector_id: 'bnc', variant: null, signal_id: null, gender: 'F', quantity: 1, label: null, network_role: null, redundancy: true };
    data.ports = [...data.ports, { ...base, direction }];
    hl = data.ports.length - 1;
  }
  let hl = $state<number | null>(null);
  const side = (inSide: boolean) => data.ports.map((p: any, i: number) => ({ p, i })).filter(({ p }: any) => (p.direction === 'in') === inSide);
  const countOf = (inSide: boolean) => side(inSide).reduce((n: number, { p }: any) => n + (Number(p.quantity) || 0), 0);
  const removePort = (i: number) => (data.ports = data.ports.filter((_: unknown, j: number) => j !== i));
  const sigsFor = (c: string) => signals.filter((s) => (s.data.connectors ?? []).includes(c));
  const variantsOf = (c: string): string[] => refById('connectors', c)?.data.variants ?? [];
  const totalPorts = $derived(data.ports.reduce((s: number, p: any) => s + (Number(p.quantity) || 0), 0));

  let dmxOn = $state(!!data.dmx);
  $effect(() => { if (dmxOn && !data.dmx) data.dmx = { default_channels: 1, modes: [] }; if (!dmxOn && data.dmx) data.dmx = null; });
  let ipOn = $state(!!data.ip);
  $effect(() => { if (ipOn && !data.ip) data.ip = { control: 'out_of_band', flows: [] }; if (!ipOn && data.ip) data.ip = null; });
  const addFlow = () => (data.ip.flows = [...data.ip.flows, { signal_id: signals[0]?.id ?? '', inputs: 0, outputs: 1 }]);
  const addMode = () => (data.dmx.modes = [...data.dmx.modes, { name: '', channels: data.dmx.default_channels || 1 }]);
  const addDoc = () => (data.documents = [...data.documents, { title: '', url: '' }]);

  // ----- Onglets -----
  type Tab = 'general' | 'format' | 'ports' | 'modules' | 'options' | 'docs';
  const TABS: { id: Tab; label: string; fields: string[] }[] = [
    { id: 'general', label: 'Général', fields: ['name', 'reference', 'brand_id', 'category_id', 'type_id', 'functions', 'phases'] },
    { id: 'format', label: 'Format et énergie', fields: ['format', 'weight_kg', 'power_w', 'btu_h', 'visual'] },
    { id: 'ports', label: 'Ports', fields: ['ports'] },
    { id: 'modules', label: 'Modules', fields: ['modular'] },
    { id: 'options', label: 'DMX et IP', fields: ['dmx', 'ip'] },
    { id: 'docs', label: 'Documents', fields: ['documents', 'custom'] },
  ];
  let tab = $state<Tab>((() => { try { return (sessionStorage.getItem('fiche-tab') as Tab) || 'general'; } catch { return 'general'; } })());
  $effect(() => { try { sessionStorage.setItem('fiche-tab', tab); } catch { /* indisponible */ } });
  const tabErr = (t: (typeof TABS)[number]) => t.fields.some((f) => !!err(f));
  const tabCount = (t: Tab) => t === 'ports' ? totalPorts : t === 'modules' ? (data.modular?.role === 'host' ? data.modular.slots : data.modular?.role === 'component' ? 1 : 0) : t === 'docs' ? data.documents.length : t === 'options' ? Number(!!data.dmx) + Number(!!data.ip) : 0;

  // ----- Aperçu de la façade aux vraies proportions (19″ = 482,6 mm, 1 U = 44,45 mm) -----
  const vis = $derived(visualOf({ visual: data.visual, type_id: data.type_id, label: data.name ?? '' }));
  const hu = $derived(data.format.kind === 'rack' ? Math.max(0.5, Number(data.format.height_u) || 1) : 1);
  const fpW = $derived(Math.min(560, (230 * 482.6) / (44.45 * hu)));
  const fpH = $derived((fpW * 44.45 * hu) / 482.6);
  const frac = $derived(({ full: 1, half: 0.5, third: 1 / 3, quarter: 0.25 } as Record<string, number>)[data.format.width ?? 'full'] ?? 1);
  const fnColor0 = $derived(refById('functions', data.functions[0])?.data?.color ?? 'var(--fg-3)');
  const inCount = $derived(countOf(true)), outCount = $derived(countOf(false));

  // ----- Modulaire : châssis (reçoit des modules) ou module (occupe des emplacements) -----
  interface Comp { id: string; name: string; reference: string | null; slots: number }
  let comps = $state<Comp[] | null>(null);
  async function loadComps() {
    if (comps) return;
    try { comps = (await api.get<{ items: { id: string; name: string; reference: string | null; data: any }[] }>('/catalog/items?kind=equipment&modular=component&limit=1000')).items.map((i) => ({ id: i.id, name: i.name, reference: i.reference, slots: i.data.modular?.slots ?? 1 })); }
    catch { comps = []; }
  }
  $effect(() => { if (data.modular?.role === 'host') loadComps(); });
  function setRole(r: 'none' | 'host' | 'component') {
    if (r === 'none') data.modular = null;
    else if (r === 'host') { if (!data.visual) data.visual = 'cards'; data.modular = { role: 'host', slots: data.modular?.role === 'host' ? data.modular.slots : 10, default_composition: data.modular?.default_composition ?? [] }; }
    else data.modular = { role: 'component', slots: data.modular?.role === 'component' ? data.modular.slots : 1 };
  }
  const compOf = (id: string) => comps?.find((c) => c.id === id);
  /** Emplacement → module qui l'occupe (premier emplacement seulement porte le module). */
  const occupancy = $derived.by(() => {
    const m = new Map<number, { i: number; first: boolean; width: number }>();
    if (data.modular?.role !== 'host') return m;
    data.modular.default_composition.forEach((p: any, i: number) => { const w = compOf(p.equipment_id)?.slots ?? 1; for (let k = 0; k < w; k++) m.set(p.slot + k, { i, first: k === 0, width: w }); });
    return m;
  });
  let pickSlot = $state<number | null>(null);
  let compQ = $state('');
  const fits = (slot: number, w: number) => { if (slot + w - 1 > (data.modular?.slots ?? 0)) return false; for (let k = 0; k < w; k++) if (occupancy.has(slot + k)) return false; return true; };
  function placeComp(c: Comp) {
    if (pickSlot == null || !fits(pickSlot, c.slots)) return;
    data.modular.default_composition = [...data.modular.default_composition, { equipment_id: c.id, slot: pickSlot }].sort((a: any, b: any) => a.slot - b.slot);
    pickSlot = null; compQ = '';
  }
  const removeComp = (i: number) => (data.modular.default_composition = data.modular.default_composition.filter((_: unknown, j: number) => j !== i));
</script>

<!-- En-tête : façade aux vraies proportions et chiffres clés -->
<section class="card fhead">
  <div class="fh-face">
    {#if data.format.kind === 'rack'}
      <div class="fp-big" style="width:{fpW}px;height:{fpH}px;--u:{fpH / hu}px">
        <span class="ritem {ALU.has(vis) ? 'alu' : ''} {hu <= 1 ? 'u1' : ''}" style="--c:{fnColor0};left:12px;width:calc(({fpW}px - 24px) * {frac});height:{fpH}px"><i class="led"></i><span class="rn">{data.name || 'Équipement'}</span><span class="dec">{@html decor(vis)}</span></span>
      </div>
      <span class="muted small">Façade {nf(hu, hu % 1 ? 1 : 0)} U{frac < 1 ? ` · ${data.format.width === 'half' ? '1/2' : data.format.width === 'third' ? '1/3' : '1/4'} largeur` : ''} · {VISUALS.find(([k]) => k === vis)?.[1] ?? 'Générique'}{data.visual ? '' : ' (automatique)'}</span>
    {:else}
      <div class="fp-free"><Icon name="box" /><span>Hors rack{data.format.length_mm ? ` · ${nf(data.format.length_mm)} × ${nf(data.format.width_mm)} × ${nf(data.format.height_mm)} mm` : ''}</span></div>
    {/if}
  </div>
  <dl class="fh-facts">
    <div><dt>Format</dt><dd>{data.format.kind === 'rack' ? `${nf(hu, hu % 1 ? 1 : 0)} U · ${data.format.depth_mm ? `${nf(data.format.depth_mm)} mm` : 'prof. ?'}` : 'Hors rack'}</dd></div>
    <div><dt>Poids</dt><dd>{data.weight_kg != null ? `${nf(data.weight_kg, 1)} kg` : '—'}</dd></div>
    <div><dt>Conso.</dt><dd>{data.power_w != null ? `${nf(data.power_w)} W` : '—'}{data.dual_psu ? ' · 2 alim.' : ''}</dd></div>
    <div><dt>Ports</dt><dd>{inCount} entrée{inCount > 1 ? 's' : ''} · {outCount} sortie{outCount > 1 ? 's' : ''}</dd></div>
    <div><dt>Modulaire</dt><dd>{data.modular?.role === 'host' ? `Châssis · ${data.modular.slots} empl.` : data.modular?.role === 'component' ? `Module · ${data.modular.slots} empl.` : 'Non'}</dd></div>
  </dl>
</section>

<div class="ftabs" role="tablist" aria-label="Sections de la fiche">
  {#each TABS as t}
    <button type="button" role="tab" aria-selected={tab === t.id} class:on={tab === t.id} class:bad={tabErr(t)} onclick={() => (tab = t.id)}>{t.label}{#if tabCount(t.id)}<span class="tc">{tabCount(t.id)}</span>{/if}{#if tabErr(t)}<i class="te" title="Champ à corriger"></i>{/if}</button>
  {/each}
</div>

{#if tab === 'general'}
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

{:else if tab === 'format'}
<section class="card">
  <div class="card-head"><h2>Format</h2><div class="seg"><button type="button" class:on={data.format.kind === 'rack'} onclick={() => setFormat('rack')}>Rack 19″</button><button type="button" class:on={data.format.kind === 'free'} onclick={() => setFormat('free')}>Hors rack</button></div></div>
  {#if data.format.kind === 'rack'}
    <div class="fg4" style="margin-top:16px">
      <NumField id="e-u" label="Hauteur" unit="U" bind:value={data.format.height_u} suggestions={dims('height_u')} error={err('format.height_u')} required />
      <div class="field"><span class="flabel">Largeur</span><div class="seg">{#each WIDTHS as [k, l]}<button type="button" class:on={data.format.width === k} onclick={() => (data.format.width = k)}>{l}</button>{/each}</div></div>
      <NumField id="e-depth" label="Profondeur" unit="mm" bind:value={data.format.depth_mm} suggestions={dims('depth_mm')} error={err('format.depth_mm')} />
      <NumField id="e-rear" label="Réserve arrière" unit="mm" bind:value={data.format.rear_reserve_mm} suggestions={dims('rear_reserve_mm')} error={err('format.rear_reserve_mm')} help="Place pour les câbles derrière l’appareil." />
    </div>
    <div class="vis-row">
      <div class="field"><label for="e-vis">Visuel en baie</label>
        <select class="input" id="e-vis" value={data.visual ?? ''} onchange={(e) => (data.visual = (e.target as HTMLSelectElement).value || null)}>
          <option value="">Automatique · {VISUALS.find(([k]) => k === visualOf({ type_id: data.type_id, label: data.name ?? '' }))?.[1] ?? 'Générique'}</option>
          {#each VISUALS as [k, l]}<option value={k}>{l}</option>{/each}
        </select>
        <span class="help">Façade dessinée dans les vues réelle et 3D (aperçu en haut de la fiche). Automatique : d’après le type, puis le nom ; pour un moniteur, le nombre d’écrans vient du nom (Dual, Triple, 4×…).</span></div>
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

{:else if tab === 'ports'}
{#snippet group(p: any, i: number, inSide: boolean)}
  <div class="pg" class:on={hl === i} style="--c:{refById('signals', p.signal_id)?.data?.color ?? refById('connectors', p.connector_id)?.data?.color ?? 'var(--fg-3)'}" role="group" aria-label="Groupe de ports {i + 1}"
    onmouseenter={() => (hl = i)} onfocusin={() => (hl = i)}>
    <div class="pg-row">
      <input class="input mono pg-q" type="number" min="1" max="1024" bind:value={p.quantity} aria-label="Nombre de ports" title="Nombre de ports">
      <span class="pg-x">×</span>
      <select class="input pg-conn" class:err={!!err(`ports[${i}].connector_id`)} bind:value={p.connector_id} aria-label="Connecteur" onchange={() => { p.variant = null; if (p.signal_id && !sigsFor(p.connector_id).some((s) => s.id === p.signal_id)) p.signal_id = null; }}>{#each connectors as c}<option value={c.id}>{c.name}</option>{/each}</select>
      <div class="seg pg-g" aria-label="Genre"><button type="button" class:on={p.gender === 'F'} onclick={() => (p.gender = 'F')} title="Femelle">F</button><button type="button" class:on={p.gender === 'M'} onclick={() => (p.gender = 'M')} title="Mâle">M</button></div>
      <button type="button" class="icon-btn sm pg-del" title="Retirer ce groupe" aria-label="Retirer ce groupe" onclick={() => { removePort(i); hl = null; }}><Icon name="x" size="sm" /></button>
    </div>
    <div class="pg-row">
      <i class="pg-dot"></i>
      <select class="input" class:err={!!err(`ports[${i}].signal_id`)} value={p.signal_id ?? ''} aria-label="Signal" onchange={(e) => (p.signal_id = (e.target as HTMLSelectElement).value || null)}><option value="">Signal —</option>{#each sigsFor(p.connector_id) as s}<option value={s.id}>{s.name}</option>{/each}</select>
      {#if variantsOf(p.connector_id).length}<select class="input pg-var" value={p.variant ?? ''} aria-label="Variante" onchange={(e) => (p.variant = (e.target as HTMLSelectElement).value || null)}><option value="">Variante —</option>{#each variantsOf(p.connector_id) as v}<option value={v}>{v}</option>{/each}</select>{/if}
    </div>
    <div class="pg-row">
      <input class="input" value={p.label ?? ''} aria-label="Libellé" placeholder={inSide ? 'Libellé, ex. SDI IN' : 'Libellé, ex. PGM OUT'} oninput={(e) => (p.label = (e.target as HTMLInputElement).value || null)}>
      <select class="input pg-face" bind:value={p.face} aria-label="Face" title="Face">{#each FACES as [k, l]}<option value={k}>{l}</option>{/each}</select>
      {#if inSide}
        <button type="button" class="btn sm ghost" title="Passer en sortie" onclick={() => (p.direction = 'out')}>Sortie →</button>
      {:else}
        <div class="seg pg-g" aria-label="Sens"><button type="button" class:on={p.direction === 'out'} onclick={() => (p.direction = 'out')} title="Sortie">Sortie</button><button type="button" class:on={p.direction === 'bidir'} onclick={() => (p.direction = 'bidir')} title="Bidirectionnel (entrée-sortie)">E/S</button><button type="button" onclick={() => (p.direction = 'in')} title="Passer en entrée">← Entrée</button></div>
      {/if}
    </div>
  </div>
{/snippet}

<section class="card ports-card">
  <div class="card-head"><div><h2>Ports</h2><span class="muted small">{totalPorts} port{totalPorts > 1 ? 's' : ''} en {data.ports.length} groupe{data.ports.length > 1 ? 's' : ''} · un groupe = des ports identiques (ex. 12 × BNC F, SDI 3G)</span></div></div>
  <div class="ports3" role="presentation" onmouseleave={() => (hl = null)}>
    <div class="pcol">
      <div class="pcol-h"><b>Entrées</b><span class="muted small tnum">{countOf(true)}</span><button type="button" class="btn sm" onclick={() => addPort('in')}><Icon name="plus" size="sm" />Entrée</button></div>
      {#each side(true) as { p, i } (i)}{@render group(p, i, true)}{:else}<p class="pcol-empty">Aucune entrée.</p>{/each}
    </div>
    <div class="pprev">
      <SynNode name={data.name} subtitle={[refName('brands', data.brand_id), data.reference].filter(Boolean).join(' · ')} fn={data.functions[0] ?? null} ports={data.ports} {hl} />
      <span class="muted small">Aperçu dans le synoptique</span>
    </div>
    <div class="pcol">
      <div class="pcol-h"><b>Sorties et E/S</b><span class="muted small tnum">{countOf(false)}</span><button type="button" class="btn sm" onclick={() => addPort('out')}><Icon name="plus" size="sm" />Sortie</button><button type="button" class="btn sm" onclick={() => addPort('bidir')}><Icon name="plus" size="sm" />E/S</button></div>
      {#each side(false) as { p, i } (i)}{@render group(p, i, false)}{:else}<p class="pcol-empty">Aucune sortie.</p>{/each}
    </div>
  </div>
  <FieldError message={err('ports')} />
</section>
{:else if tab === 'modules'}
<section class="card">
  <div class="card-head"><div><h2>Équipement modulaire</h2><span class="muted small">Un châssis reçoit des modules (cartes, convertisseurs) dans ses emplacements ; un module occupe un ou plusieurs emplacements d’un châssis.</span></div></div>
  <div class="seg role-seg">
    <button type="button" class:on={!data.modular} onclick={() => setRole('none')}>Équipement simple</button>
    <button type="button" class:on={data.modular?.role === 'host'} onclick={() => setRole('host')}>Châssis · reçoit des modules</button>
    <button type="button" class:on={data.modular?.role === 'component'} onclick={() => setRole('component')}>Module · se glisse dans un châssis</button>
  </div>
  {#if data.modular?.role === 'host'}
    <div class="fg4" style="margin-top:16px"><NumField id="e-slots" label="Emplacements" bind:value={data.modular.slots} min={1} max={40} step={1} error={err('modular.slots')} help="De 1 à 40." /></div>
    <div class="chassis-h"><b>Composition par défaut</b><span class="muted small">Proposée quand le châssis est ajouté à un projet. Cliquez un emplacement libre pour y glisser un module.</span></div>
    <div class="chassis" style="--n:{data.modular.slots}">
      {#each Array(Math.max(1, Math.min(40, Number(data.modular.slots) || 1))) as _, k}
        {@const slot = k + 1}
        {@const o = occupancy.get(slot)}
        {#if o?.first}
          {@const p = data.modular.default_composition[o.i]}
          <div class="card-slot full" style="grid-column:span {Math.min(o.width, data.modular.slots - k)}" title={compOf(p.equipment_id)?.name ?? p.equipment_id}>
            <span class="cs-n">{slot}{o.width > 1 ? `–${slot + o.width - 1}` : ''}</span>
            <span class="cs-name">{compOf(p.equipment_id)?.name ?? 'Module'}</span>
            <button type="button" class="cs-x" aria-label="Retirer ce module" onclick={() => removeComp(o.i)}><Icon name="x" size="sm" /></button>
          </div>
        {:else if !o}
          <button type="button" class="card-slot" class:pick={pickSlot === slot} onclick={() => { pickSlot = pickSlot === slot ? null : slot; loadComps(); }}><span class="cs-n">{slot}</span><span class="cs-plus">+</span></button>
        {/if}
      {/each}
    </div>
    <FieldError message={err('modular')} />
    {#if pickSlot != null}
      <div class="comp-pick">
        <div class="cp-h"><b>Module pour l’emplacement {pickSlot}</b><input class="input" placeholder="Rechercher un module…" bind:value={compQ} aria-label="Rechercher un module"><button type="button" class="btn sm ghost" onclick={() => (pickSlot = null)}>Fermer</button></div>
        {#if comps === null}<span class="spin"></span>
        {:else}
          {#each comps.filter((c) => !compQ.trim() || `${c.name} ${c.reference ?? ''}`.toLowerCase().includes(compQ.trim().toLowerCase())).slice(0, 60) as c (c.id)}
            {@const ok = fits(pickSlot, c.slots)}
            <button type="button" class="cp-row" disabled={!ok} onclick={() => placeComp(c)}><b>{c.name}</b><span class="muted small">{c.reference ?? ''}</span><span class="cp-w">{c.slots} empl.{ok ? '' : ' · ne tient pas ici'}</span></button>
          {:else}<p class="muted small">Aucun module au catalogue. Ouvrez la fiche d’une carte et choisissez « Module » dans cet onglet.</p>{/each}
        {/if}
      </div>
    {/if}
  {:else if data.modular?.role === 'component'}
    <div class="fg4" style="margin-top:16px"><NumField id="e-cslots" label="Emplacements occupés" bind:value={data.modular.slots} min={1} max={8} step={1} error={err('modular.slots')} help="Largeur du module dans le châssis, de 1 à 8." /></div>
    <p class="muted small" style="margin-top:8px">Ce module apparaît dans la liste des modules de chaque fiche de châssis.</p>
  {:else}
    <p class="muted small" style="margin-top:14px">Cet équipement ne reçoit pas de modules et ne se glisse pas dans un châssis.</p>
  {/if}
</section>

{:else if tab === 'options'}
<section class="card">
  <div class="card-head"><h2>Lumières (DMX)</h2><label class="toggle-row"><input type="checkbox" bind:checked={dmxOn}>Pilotable en DMX</label></div>
  {#if dmxOn && data.dmx}
    <div class="fg3" style="margin-top:16px"><NumField id="e-dmx" label="Canaux par défaut" bind:value={data.dmx.default_channels} error={err('dmx')} /></div>
    <div class="stack" style="gap:8px;margin-top:10px">
      <span class="flabel">Modes</span>
      {#each data.dmx.modes as mo, i}
        <div class="fg3"><input class="input" bind:value={mo.name} placeholder="Nom du mode (Standard, Étendu…)" aria-label="Nom du mode {i + 1}"><NumField id="e-dm{i}" label="Canaux" bind:value={mo.channels} /><button type="button" class="btn sm ghost danger" style="align-self:end" onclick={() => (data.dmx.modes = data.dmx.modes.filter((_: unknown, j: number) => j !== i))}><Icon name="trash" size="sm" />Retirer</button></div>
      {/each}
      <button type="button" class="btn sm" style="justify-self:start" onclick={addMode}><Icon name="plus" size="sm" />Ajouter un mode</button>
    </div>
  {/if}
</section>

<section class="card">
  <div class="card-head"><div><h2>Flux IP (SMPTE ST 2110…)</h2><span class="muted small">Chaque sortie de flux reçoit un groupe multicast dans le plan d’adressage du projet. Essence déduite de la famille du signal : vidéo, audio ou données annexes.</span></div><label class="toggle-row"><input type="checkbox" bind:checked={ipOn}>Émet ou reçoit des flux IP</label></div>
  {#if ipOn && data.ip}
    <div class="stack" style="gap:8px;margin-top:12px">
      <div class="seg" style="justify-self:start"><button type="button" class:on={data.ip.control === 'out_of_band'} onclick={() => (data.ip.control = 'out_of_band')}>Contrôle hors bande</button><button type="button" class:on={data.ip.control === 'in_band'} onclick={() => (data.ip.control = 'in_band')}>Contrôle dans la bande</button></div>
      {#each data.ip.flows as f, i}
        <div class="fg4">
          <div class="field"><label for="e-fs{i}">Signal</label><select class="input" id="e-fs{i}" bind:value={f.signal_id}>{#each signals as sg}<option value={sg.id}>{sg.name}</option>{/each}</select></div>
          <NumField id="e-fi{i}" label="Entrées" bind:value={f.inputs} min={0} step={1} />
          <NumField id="e-fo{i}" label="Sorties" bind:value={f.outputs} min={0} step={1} />
          <button type="button" class="btn sm ghost danger" style="align-self:end" onclick={() => (data.ip.flows = data.ip.flows.filter((_: unknown, j: number) => j !== i))}><Icon name="trash" size="sm" />Retirer</button>
        </div>
      {/each}
      <button type="button" class="btn sm" style="justify-self:start" onclick={addFlow}><Icon name="plus" size="sm" />Ajouter un flux</button>
    </div>
  {/if}
</section>

{:else}
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
{/if}

<style>
  .ports3 { display: grid; grid-template-columns: minmax(300px, 1fr) minmax(330px, auto) minmax(300px, 1fr); gap: 18px; align-items: start; margin-top: 14px; }
  .pcol { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .pcol-h { display: flex; align-items: center; gap: 8px; min-height: 30px; }
  .pcol-h b { font: 650 14px var(--display); }
  .pcol-h .muted { margin-right: auto; }
  .pcol-empty { color: var(--fg-3); font-size: 13px; padding: 14px; border-radius: 9px; border: 1px dashed var(--line-2); text-align: center; }
  .pprev { position: sticky; top: 70px; display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 8px 4px; }
  .pg { display: flex; flex-direction: column; gap: 6px; padding: 8px 8px 8px 12px; border-radius: 10px; background: var(--surface-2); box-shadow: inset 0 0 0 1px var(--line), inset 3px 0 0 var(--c); transition: box-shadow .12s, background .12s; }
  .pg.on { background: color-mix(in srgb, var(--c) 6%, var(--surface)); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--c) 45%, var(--line)), inset 3px 0 0 var(--c); }
  .pg-row { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .pg-row > .input { flex: 1 1 0; min-width: 0; height: 30px; font-size: 13px; }
  .pg .pg-q { flex: 0 0 58px; text-align: right; }
  .pg-x { color: var(--fg-3); font-size: 13px; }
  .pg .pg-face { flex: 0 0 104px; }
  .pg .pg-var { flex: 0 1 120px; }
  .pg-g { flex: none; }
  .pg-g button { height: 24px; padding: 0 8px; font-size: 12px; }
  .pg-del { flex: none; width: 26px; height: 26px; }
  .pg-dot { flex: none; width: 9px; height: 9px; margin: 0 3px 0 1px; border-radius: 50%; border: 2px solid var(--c); background: var(--node-bg, var(--surface)); }
  @media (max-width: 1280px) {
    .ports3 { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); }
    .pprev { grid-column: 1 / -1; grid-row: 1; position: static; }
  }
  @media (max-width: 760px) { .ports3 { grid-template-columns: minmax(0, 1fr); } }
  .vis-row { display: grid; grid-template-columns: minmax(220px, 420px); gap: 16px; margin-top: 14px; }
  /* En-tête */
  .fhead { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 28px; align-items: center; }
  .fh-face { display: flex; flex-direction: column; gap: 8px; align-items: flex-start; }
  .fp-big { position: relative; flex: none; border-radius: 6px; background: linear-gradient(90deg, #2A2E34, #3A3F46 6%, #30353B 50%, #3A3F46 94%, #2A2E34); box-shadow: 0 0 0 6px #1d2126, 0 12px 26px -14px rgba(0, 0, 0, .6); }
  .fp-big .ritem { position: absolute; top: 0; font-size: 13px; }
  .fp-free { display: flex; align-items: center; gap: 10px; padding: 18px 22px; border-radius: 12px; background: var(--surface-2); color: var(--fg-2); }
  .fh-facts { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px 18px; margin: 0; }
  .fh-facts div { display: flex; flex-direction: column; gap: 2px; }
  .fh-facts dt { font: 600 10.5px var(--mono); letter-spacing: .08em; text-transform: uppercase; color: var(--fg-3); }
  .fh-facts dd { margin: 0; font-weight: 600; font-size: 14.5px; }
  @media (max-width: 900px) { .fhead { grid-template-columns: minmax(0, 1fr); } .fp-big { max-width: 100%; } }
  /* Onglets */
  .ftabs { display: flex; gap: 4px; flex-wrap: wrap; padding: 4px; border-radius: 12px; background: var(--surface); box-shadow: inset 0 0 0 1px var(--line); position: sticky; top: 8px; z-index: 5; }
  .ftabs button { display: inline-flex; align-items: center; gap: 7px; padding: 8px 14px; border: 0; border-radius: 9px; background: none; font: inherit; font-weight: 600; font-size: 14px; color: var(--fg-3); cursor: pointer; }
  .ftabs button:hover { color: var(--fg); background: var(--surface-2); }
  .ftabs button.on { color: var(--fg); background: var(--surface-2); box-shadow: inset 0 0 0 1px var(--line-2); }
  .ftabs .tc { font: 600 11px var(--mono); padding: 1px 6px; border-radius: 99px; background: var(--sunken, var(--surface-2)); color: var(--fg-2); }
  .ftabs .te { width: 7px; height: 7px; border-radius: 50%; background: var(--err); }
  .ftabs button.bad { color: var(--err); }
  /* Modulaire */
  .role-seg { margin-top: 14px; flex-wrap: wrap; }
  .chassis-h { display: flex; flex-direction: column; gap: 2px; margin: 18px 0 8px; }
  .chassis { display: grid; grid-template-columns: repeat(var(--n), minmax(26px, 1fr)); gap: 3px; padding: 10px; border-radius: 8px; background: linear-gradient(180deg, #2b3036, #1c2025); box-shadow: inset 0 0 0 1px #000; }
  .card-slot { position: relative; height: 150px; display: flex; flex-direction: column; align-items: center; justify-content: space-between; padding: 6px 2px; border: 0; border-radius: 3px; background: repeating-linear-gradient(180deg, #121519 0 6px, #171b20 6px 12px); box-shadow: inset 0 0 0 1px #000; color: #8f98a3; font: inherit; cursor: pointer; }
  .card-slot:hover, .card-slot.pick { box-shadow: inset 0 0 0 1.5px var(--accent); }
  .card-slot.full { cursor: default; background: linear-gradient(90deg, #4a515a, #5d6570 40%, #444b54); color: #fff; box-shadow: inset 0 0 0 1px #000, inset 0 1px 0 rgba(255, 255, 255, .25); }
  .cs-n { font: 600 10px var(--mono); }
  .cs-plus { font-size: 18px; line-height: 1; opacity: .7; }
  .cs-name { flex: 1; writing-mode: vertical-rl; transform: rotate(180deg); font-size: 11.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin: 6px 0; }
  .cs-x { border: 0; background: rgba(0, 0, 0, .35); color: #fff; border-radius: 4px; width: 22px; height: 22px; display: grid; place-items: center; cursor: pointer; }
  .comp-pick { margin-top: 12px; display: grid; gap: 6px; padding: 12px; border-radius: 12px; background: var(--surface-2); max-height: 360px; overflow: auto; }
  .cp-h { display: flex; gap: 8px; align-items: center; }
  .cp-h .input { flex: 1; height: 34px; }
  .cp-row { display: grid; grid-template-columns: minmax(0, 1fr) auto auto; gap: 10px; align-items: center; padding: 8px 10px; border: 0; border-radius: 9px; background: var(--surface); text-align: left; font: inherit; color: inherit; cursor: pointer; }
  .cp-row:hover:not(:disabled) { box-shadow: inset 0 0 0 1.5px var(--accent); }
  .cp-row:disabled { opacity: .5; cursor: default; }
  .cp-w { font: 600 11.5px var(--mono); color: var(--fg-3); }
</style>
