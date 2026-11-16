<script lang="ts">
  // Ajouter ou modifier un élément du projet, dans une seule fenêtre : le type en cartes (en tête
  // à la création, replié en modification), les seuls champs utiles à ce type avec des préréglages
  // cliquables, les champs rares repliés, et un aperçu vivant à droite (au-dessus sur mobile).
  import { untrack } from 'svelte';
  import Dialog from '../../components/Dialog.svelte';
  import NumField from '../../components/NumField.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import KindIll from '../../components/KindIll.svelte';
  import Icon from '../../components/Icon.svelte';
  import Seg from '../../components/Seg.svelte';
  import ContainerPreview from './ContainerPreview.svelte';
  import { api, ApiError } from '../../lib/api';
  import { toast } from '../../lib/toast.svelte';
  import { CONTAINER_KINDS, ELEMENT_HINTS, U_PRESETS, blankContainer, type Container, type ContainerData, type ContainerKind, type DeskPosition, type ProjectView } from '../../lib/projects';

  let { view, container = null, locationId, onclose, ondone }: { view: ProjectView; container?: Container | null; locationId: string; onclose: () => void; ondone: (id: string) => void } = $props();
  const editing = untrack(() => !!container);

  // Galerie : trois groupes, les types masqués (casquette) exclus sauf s'il s'agit du type de l'élément modifié.
  type Group = 'contenants' | 'regie' | 'autres';
  const GROUPS: { value: Group; label: string; help: string }[] = [
    { value: 'contenants', label: 'Racks et valises', help: 'Élévation en U, panels et circuits.' },
    { value: 'regie', label: 'Mobilier et régie', help: 'Pupitres, postes et murs d’écrans.' },
    { value: 'autres', label: 'Autres', help: 'Terrain, plateau, infrastructure du site et diffusion.' },
  ];
  const groupOf = (k: ContainerKind): Group => { const f = CONTAINER_KINDS[k].family; return f === 'contenants' || f === 'regie' ? f : 'autres'; };
  const kindsOf = (g: Group) => (Object.keys(CONTAINER_KINDS) as ContainerKind[]).filter((k) => groupOf(k) === g && (!CONTAINER_KINDS[k].hidden || k === container?.kind));
  /** Nom de base proposé pour un nouvel élément (« Rack 2 », « Pupitre 1 »). */
  const BASE: Record<ContainerKind, string> = {
    rack: 'Rack', flight_case: 'Flight-case', hard_case: 'Valise', waterproof_case: 'Valise étanche', furniture: 'Meuble',
    monitor_wall: 'Mur d’écrans', desk: 'Pupitre', operator_position: 'Poste', casquette: 'Casquette',
    zone: 'Zone', camera_position: 'Caméra', field_box: 'Boîtier', rf_mast: 'Mât HF',
    site_point: 'Point', power: 'Énergie', external_feed: 'Arrivée', display: 'Écran', cable_drum: 'Touret',
  };
  const U_CASE = [2, 3, 4, 6, 8, 10, 12, 16, 20, 24];
  const DEPTHS = { rack: [600, 800, 1000, 1200], case: [400, 450, 500, 600] };
  const DESK = {
    desk: { L: [1800, 2400, 3000], P: [800, 900, 1000], H: [720, 750, 1050] },
    furniture: { L: [600, 1200, 1800], P: [400, 600], H: [750, 1000, 2000] },
  };

  // La fenêtre part d'une copie : les changements ne s'appliquent qu'à l'enregistrement.
  let c = $state<ContainerData & Partial<Container>>(untrack(() => (container ? JSON.parse(JSON.stringify({ ...container, details: { ...blankContainer(container.kind).details, ...(container.details ?? {}) } })) : blankContainer('rack'))));
  let loc = $state(untrack(() => container?.location_id ?? locationId));
  let named = $state(editing);
  let name = $state(untrack(() => container?.name ?? suggestName('rack', locationId)));
  let group = $state<Group>(untrack(() => groupOf(c.kind)));
  let showTypes = $state(!editing);
  let advanced = $state(false);
  let touched = $state(false);
  let errors = $state<Record<string, string>>({});
  let general = $state<string | null>(null);
  const K = $derived(CONTAINER_KINDS[c.kind]);
  const d = $derived(c.details);
  /** Dimensions principales : en U (rackable), mobilier (L × P × H), boîte (L × l × P) ou repliées. */
  const dims = $derived(c.rackable ? 'u' : c.kind === 'desk' || c.kind === 'furniture' ? 'desk' : c.kind === 'waterproof_case' || K.canRack ? 'box' : 'none');
  const isContainer = $derived(dims !== 'none');
  const ADV_KEYS = $derived(['cable_code', 'max_weight_kg', 'max_power_w', 'notes', ...(isContainer ? [] : ['length_mm', 'width_mm', 'depth_mm', 'empty_weight_kg'])]);

  function suggestName(k: ContainerKind, l: string): string {
    const base = BASE[k];
    const re = new RegExp(`^${base.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s+(\\d+)$`, 'i');
    const taken = new Set(view.containers.filter((x) => x.id !== container?.id).map((x) => x.name.trim().toLowerCase()));
    let n = 1 + Math.max(0, ...view.containers.filter((x) => x.location_id === l && x.id !== container?.id).map((x) => Number(re.exec(x.name.trim())?.[1] ?? 0)));
    while (taken.has(`${base} ${n}`.toLowerCase())) n++;
    return `${base} ${n}`;
  }
  function pick(k: ContainerKind) {
    if (k === c.kind) return;
    const N = CONTAINER_KINDS[k];
    if (editing) {
      const was = CONTAINER_KINDS[c.kind];
      c.kind = k;
      if (!N.mobile) c.wheels = false;
      if (!N.canRack) c.rackable = false;
      else if (!was.canRack) { c.rackable = N.rackable; c.height_u ??= k === 'rack' ? 42 : 12; c.depth_mm ??= 600; }
      c.details = { ...blankContainer(k).details, ...c.details };
      touched = true;
    } else {
      const p = $state.snapshot(c);
      c = { ...blankContainer(k), empty_weight_kg: p.empty_weight_kg, max_weight_kg: p.max_weight_kg, max_power_w: p.max_power_w, cable_code: p.cable_code, notes: p.notes,
        ...(k === 'desk' ? { length_mm: 2400, width_mm: 900, outer_height_mm: 750 } : {}) };
    }
    if (!named) name = suggestName(k, loc);
  }
  function setNum(key: 'height_u' | 'depth_mm' | 'length_mm' | 'width_mm' | 'outer_height_mm', v: number) { c[key] = v; touched = true; }
  // La boîte suit elle-même l'envoi (bouton bloqué, indicateur) pendant que save() attend.
  async function save() {
    errors = {}; general = null;
    const { id: _i, project_id: _p, position: _o, rev: _r, totals: _t, ...data } = $state.snapshot(c) as Container;
    const body = { ...data, name: name.trim(), location_id: loc };
    try {
      const r = container ? await api.patch<Container>(`/containers/${container.id}`, { ...body, rev: container.rev }) : await api.post<Container>(`/projects/${view.project.id}/containers`, body);
      touched = false;
      toast(container ? 'Élément enregistré.' : r.name !== body.name ? `Élément créé sous le nom « ${r.name} ».` : `« ${r.name} » créé.`);
      ondone(r.id);
    } catch (x) {
      if (!(x instanceof ApiError)) { general = 'Enregistrement impossible.'; return; }
      errors = Object.fromEntries(x.fields.map((f) => [f.field.replace(/^details\./, ''), f.message]));
      if (ADV_KEYS.some((k) => errors[k])) advanced = true;
      // Une erreur sur un champ absent de la fenêtre reste lisible dans l'alerte générale.
      const lost = x.fields.filter((f) => !SHOWN.has(f.field.replace(/^details\./, '').replace(/\.\d+.*$/, ''))).map((f) => f.message);
      general = !x.fields.length ? x.message : lost.length ? lost.join(' ') : null;
    }
  }
  const SHOWN = new Set(['name', 'location_id', 'cable_code', 'height_u', 'depth_mm', 'rear_rails_mm', 'length_mm', 'width_mm', 'outer_height_mm', 'empty_weight_kg', 'max_weight_kg', 'max_power_w',
    'wall.rows', 'wall.cols', 'wall.tiles', 'positions', 'area_m2', 'camera_number', 'mast_height_m', 'power_kva', 'diagonal_in', 'cable_length_m', 'drum_count']);
  const txt = (k: string) => (e: Event) => { (c.details as Record<string, unknown>)[k] = (e.target as HTMLInputElement).value || null; };
  function addPost() { c.details.positions = [...(c.details.positions ?? []), { id: `p${Date.now().toString(36)}`, role: '', name: '', instance_ids: [] }]; touched = true; }

  // Aperçu : résumé texte et pastilles.
  const num = (n: number) => n.toLocaleString('fr-FR', { maximumFractionDigits: 2 });
  const summary = $derived.by(() => {
    const p: string[] = [K.label];
    if (c.rackable) { if (c.height_u) p.push(`${num(c.height_u)} U`); if (c.depth_mm) p.push(`${num(c.depth_mm)} mm`); }
    else if (dims === 'desk') { if (c.length_mm && c.width_mm) p.push(`${num(c.length_mm)} × ${num(c.width_mm)} mm`); }
    else if (dims === 'box') { const v = [c.length_mm, c.width_mm, c.depth_mm]; if (v.every((x) => x)) p.push(`${v.map((x) => num(x!)).join(' × ')} mm`); }
    if (c.kind === 'monitor_wall' && d.wall) p.push(`${d.wall.rows || 1} × ${d.wall.cols || 1} écrans`);
    if (c.kind === 'desk') { const n = d.positions?.length ?? 0; p.push(n ? `${n} poste${n > 1 ? 's' : ''}` : 'sans poste'); }
    if (c.kind === 'operator_position' && d.positions?.[0]?.role) p.push(d.positions[0].role);
    if (c.kind === 'zone') { if (d.surface) p.push(d.surface); if (d.area_m2) p.push(`${num(d.area_m2)} m²`); }
    if (c.kind === 'camera_position') { if (d.camera_number) p.push(`n° ${d.camera_number}`); if (d.mount) p.push(d.mount); }
    if (c.kind === 'rf_mast' && d.mast_height_m) p.push(`${num(d.mast_height_m)} m`);
    if (c.kind === 'power') { if (d.power_kva) p.push(`${num(d.power_kva)} kVA`); if (d.phases) p.push(d.phases === 3 ? 'Tri' : 'Mono'); }
    if (c.kind === 'external_feed' && d.feed) p.push(d.feed);
    if ((c.kind === 'site_point' || c.kind === 'field_box') && d.point_kind) p.push(d.point_kind);
    if (c.kind === 'display') { if (d.display_kind) p.push(d.display_kind); if (d.diagonal_in) p.push(`${num(d.diagonal_in)}″`); }
    if (c.kind === 'cable_drum' && d.cable_length_m) p.push(`${d.drum_count && d.drum_count > 1 ? `${d.drum_count} × ` : ''}${num(d.cable_length_m)} m`);
    return p.join(' · ');
  });
  const chips = $derived([
    ...(c.rackable && c.kind !== 'rack' ? ['Rackable 19″'] : []),
    ...(c.wheels ? ['Roulettes'] : []),
    ...(c.rackable && c.rear_rails_mm ? ['Rails arrière'] : []),
    ...(c.empty_weight_kg ? [`${num(c.empty_weight_kg)} kg à vide`] : []),
  ]);
  const place = $derived(view.locations.length > 1 ? (view.locations.find((l) => l.id === loc)?.name ?? '') : '');
</script>

{#snippet hint(id: string, list: string[])}<datalist {id}>{#each list as v}<option value={v}></option>{/each}</datalist>{/snippet}

<!-- Préréglages cliquables sous un champ numérique (pastilles à état pressé). -->
{#snippet presets(label: string, values: number[], cur: number | null, set: (v: number) => void)}
  <div class="presets" role="group" aria-label={label}>
    {#each values as v (v)}<button type="button" class="fchip cxf" class:on={cur === v} aria-pressed={cur === v} onclick={() => set(v)}>{num(v)}</button>{/each}
  </div>
{/snippet}

<Dialog title={container ? `Modifier ${container.name}` : 'Nouvel élément'} eyebrow={view.project.name} {onclose} wide
  onsubmit={save} submitLabel={container ? 'Enregistrer' : 'Créer l’élément'} dirty={() => touched}>
  <div class="ce" oninput={() => (touched = true)} onchange={() => (touched = true)}>
    {#if showTypes}
      <section class="types" aria-label="Type d’élément">
        <div class="types-h">
          <Seg mode="tabs" label="Groupe de types" options={GROUPS} bind:value={group} controls="ce-types" />
          <span class="muted small">{GROUPS.find((x) => x.value === group)?.help}</span>
        </div>
        <div class="kcards" role="tabpanel" id="ce-types-{group}" aria-label={GROUPS.find((x) => x.value === group)?.label}>
          {#each kindsOf(group) as k (k)}
            {@const v = CONTAINER_KINDS[k]}
            <button type="button" class="card link compact kc" class:on={c.kind === k} aria-pressed={c.kind === k} onclick={() => pick(k)} data-autofocus={!editing && c.kind === k ? '' : undefined}>
              <KindIll kind={k} size={60} />
              <b>{v.label}</b><span class="kc-h">{v.help}</span>
            </button>
          {/each}
        </div>
      </section>
    {/if}

    <div class="ce-main">
      <aside class="ce-prev" aria-label="Aperçu">
        <ContainerPreview {c} {name} {place} {summary} {chips} />
      </aside>

      <div class="ce-form">
        {#if editing}
          <div class="typebar">
            <KindIll kind={c.kind} size={42} />
            <div><span class="lbl-t">Type</span><b>{K.label}</b></div>
            <button type="button" class="btn sm ghost" aria-expanded={showTypes} onclick={() => { showTypes = !showTypes; group = groupOf(c.kind); }}>{showTypes ? 'Masquer les types' : 'Changer le type'}</button>
          </div>
        {/if}

        <div class="form-grid">
          <div class="field" class:span={view.locations.length < 2}><label for="c-name">Nom</label><input class="input" class:err={!!errors.name} id="c-name" bind:value={name} oninput={() => (named = true)} required maxlength="120" autocomplete="off" aria-invalid={errors.name ? 'true' : undefined} aria-describedby={errors.name ? 'c-name-err' : undefined}><FieldError for="c-name" message={errors.name} /></div>
          {#if view.locations.length > 1}
            <div class="field"><label for="c-loc">Emplacement</label><select class="input" class:err={!!errors.location_id} id="c-loc" bind:value={loc} onchange={() => { if (!named) name = suggestName(c.kind, loc); }} aria-invalid={errors.location_id ? 'true' : undefined} aria-describedby={errors.location_id ? 'c-loc-err' : undefined}>{#each view.locations as l (l.id)}<option value={l.id}>{l.name}</option>{/each}</select><FieldError for="c-loc" message={errors.location_id} /></div>
          {/if}
        </div>

        {#if c.kind === 'monitor_wall'}
          {@const w = d.wall!}
          <section class="sec"><h3 class="lbl-t">Mur d’écrans</h3>
            <div class="fg3">
              <NumField id="w-r" label="Rangées" bind:value={w.rows} min={1} max={12} step={1} error={errors['wall.rows']} />
              <NumField id="w-c" label="Colonnes" bind:value={w.cols} min={1} max={16} step={1} error={errors['wall.cols']} />
              <div class="field"><label for="w-s">Support (facultatif)</label><input class="input" id="w-s" list="h-sup" value={w.support ?? ''} oninput={(e) => (w.support = (e.target as HTMLInputElement).value || null)} placeholder="Ex. structure alu">{@render hint('h-sup', ELEMENT_HINTS.support!)}</div>
            </div>
            <span class="help">Ajoutez ensuite les écrans à l’élément, puis placez-les sur la grille depuis l’inventaire.</span>
            {#if Object.keys(errors).some((k) => k.startsWith('wall.tiles'))}<div class="alert err">Réduire le mur ferait sortir des écrans de la grille : retirez-les d’abord.</div>{/if}
          </section>
        {:else if c.kind === 'desk' || c.kind === 'operator_position'}
          <section class="sec"><h3 class="lbl-t">{c.kind === 'desk' ? 'Postes du mobilier' : 'Poste'}</h3>
            {#each d.positions ?? [] as p, i (p.id)}
              <div class="post">
                <span class="post-n mono">{i + 1}</span>
                <input class="input" list="h-roles" bind:value={p.role} placeholder="Rôle (ex. réalisateur)" aria-label="Rôle du poste {i + 1}" required>
                <input class="input" bind:value={p.name} placeholder="Personne prévue" aria-label="Personne prévue au poste {i + 1} (facultatif)">
                {#if c.kind === 'desk'}<button type="button" class="icon-btn sm" aria-label="Retirer le poste {i + 1}" title="Retirer le poste {i + 1}" onclick={() => { c.details.positions = d.positions!.filter((x: DeskPosition) => x !== p); touched = true; }}><Icon name="x" size="sm" /></button>{/if}
              </div>
            {/each}
            {@render hint('h-roles', ELEMENT_HINTS.roles!)}
            {#if c.kind === 'desk' && (d.positions?.length ?? 0) < 24}<button type="button" class="btn sm ghost add-post" onclick={addPost}><Icon name="plus" size="sm" />Ajouter un poste</button>{/if}
            <FieldError message={errors.positions} />
          </section>
        {:else if c.kind === 'zone'}
          <section class="sec"><h3 class="lbl-t">Emplacement libre</h3><div class="form-grid">
            <div class="field"><label for="z-s">Surface</label><input class="input" id="z-s" list="h-surf" value={d.surface ?? ''} oninput={txt('surface')} placeholder="Ex. table, sol, loge">{@render hint('h-surf', ELEMENT_HINTS.surface!)}</div>
            <NumField id="z-a" label="Superficie" unit="m²" bind:value={c.details.area_m2!} error={errors.area_m2} />
          </div></section>
        {:else if c.kind === 'camera_position'}
          <section class="sec"><h3 class="lbl-t">Position caméra</h3><div class="fg3">
            <NumField id="k-n" label="Numéro de caméra" bind:value={c.details.camera_number!} min={1} max={999} step={1} error={errors.camera_number} />
            <div class="field"><label for="k-m">Support</label><input class="input" id="k-m" list="h-mount" value={d.mount ?? ''} oninput={txt('mount')}>{@render hint('h-mount', ELEMENT_HINTS.mount!)}</div>
            <div class="field"><label for="k-l">Liaison</label><input class="input" id="k-l" list="h-link" value={d.link ?? ''} oninput={txt('link')}>{@render hint('h-link', ELEMENT_HINTS.link!)}</div>
          </div></section>
        {:else if c.kind === 'rf_mast'}
          <section class="sec"><h3 class="lbl-t">Mât HF</h3><div class="form-grid">
            <NumField id="m-h" label="Hauteur" unit="m" bind:value={c.details.mast_height_m!} error={errors.mast_height_m} />
            <div class="field"><label for="m-f">Fréquences</label><input class="input mono" id="m-f" value={d.frequencies ?? ''} oninput={txt('frequencies')} placeholder="Ex. 606,100 · 614,350 MHz"></div>
          </div></section>
        {:else if c.kind === 'power'}
          <section class="sec"><h3 class="lbl-t">Énergie</h3><div class="fg3">
            <NumField id="p-k" label="Puissance" unit="kVA" bind:value={c.details.power_kva!} error={errors.power_kva} />
            <div class="field"><span class="flabel">Phases</span><Seg label="Phases" value={c.details.phases ?? null} onchange={(v) => { c.details.phases = v; touched = true; }} options={[{ value: 1, label: 'Mono' }, { value: 3, label: 'Tri' }]} /></div>
            <div class="field"><label for="p-o">Sortie</label><input class="input" id="p-o" list="h-out" value={d.outlet ?? ''} oninput={txt('outlet')}>{@render hint('h-out', ELEMENT_HINTS.outlet!)}</div>
          </div></section>
        {:else if c.kind === 'external_feed'}
          <section class="sec"><h3 class="lbl-t">Arrivée externe</h3><div class="fg3">
            <div class="field"><label for="x-f">Liaison</label><input class="input" id="x-f" list="h-feed" value={d.feed ?? ''} oninput={txt('feed')}>{@render hint('h-feed', ELEMENT_HINTS.feed!)}</div>
            <div class="field"><label for="x-p">Opérateur</label><input class="input" id="x-p" value={d.provider ?? ''} oninput={txt('provider')}></div>
            <div class="field"><label for="x-r">Référence du circuit</label><input class="input mono" id="x-r" value={d.circuit_ref ?? ''} oninput={txt('circuit_ref')}></div>
          </div></section>
        {:else if c.kind === 'site_point' || c.kind === 'field_box'}
          <section class="sec"><h3 class="lbl-t">{K.label}</h3><div class="form-grid">
            <div class="field"><label for="s-k">Nature</label><input class="input" id="s-k" list="h-pt" value={d.point_kind ?? ''} oninput={txt('point_kind')}>{@render hint('h-pt', ELEMENT_HINTS.point_kind!)}</div>
          </div></section>
        {:else if c.kind === 'display'}
          <section class="sec"><h3 class="lbl-t">Diffusion</h3><div class="fg3">
            <div class="field"><label for="d-k">Nature</label><input class="input" id="d-k" list="h-disp" value={d.display_kind ?? ''} oninput={txt('display_kind')}>{@render hint('h-disp', ELEMENT_HINTS.display_kind!)}</div>
            <NumField id="d-d" label="Diagonale" unit="″" bind:value={c.details.diagonal_in!} error={errors.diagonal_in} />
            <div class="field"><label for="d-r">Résolution</label><input class="input mono" id="d-r" value={d.resolution ?? ''} oninput={txt('resolution')} placeholder="3840 × 2160"></div>
          </div></section>
        {:else if c.kind === 'cable_drum'}
          <section class="sec"><h3 class="lbl-t">Touret</h3><div class="form-grid">
            <NumField id="t-l" label="Longueur" unit="m" bind:value={c.details.cable_length_m!} error={errors.cable_length_m} />
            <NumField id="t-n" label="Nombre" bind:value={c.details.drum_count!} min={1} step={1} error={errors.drum_count} />
          </div></section>
        {/if}

        {#if isContainer || K.mobile}
          <section class="sec"><h3 class="lbl-t">{isContainer ? 'Dimensions' : 'Transport'}</h3>
            {#if (K.canRack && (c.kind !== 'rack' || !c.rackable)) || K.mobile}
              <div class="toggles">
                {#if K.canRack && (c.kind !== 'rack' || !c.rackable)}<label class="toggle-row"><input type="checkbox" bind:checked={c.rackable} onchange={() => { if (c.rackable) { c.height_u ??= c.kind === 'rack' ? 42 : 12; c.depth_mm ??= 600; } }}>Rackable 19″ (hauteur en U)</label>{/if}
                {#if K.mobile}<label class="toggle-row"><input type="checkbox" bind:checked={c.wheels}>Sur roulettes</label>{/if}
              </div>
            {/if}
            {#if dims === 'u'}
              <div class="dim">
                <NumField id="c-u" label="Hauteur" unit="U" bind:value={c.height_u} min={1} max={60} step={1} error={errors.height_u} />
                {@render presets('Hauteurs courantes', c.kind === 'rack' ? U_PRESETS : U_CASE, c.height_u, (v) => setNum('height_u', v))}
              </div>
              <div class="dim">
                <NumField id="c-d" label="Profondeur" unit="mm" bind:value={c.depth_mm} error={errors.depth_mm} />
                {@render presets('Profondeurs courantes', c.kind === 'rack' ? DEPTHS.rack : DEPTHS.case, c.depth_mm, (v) => setNum('depth_mm', v))}
              </div>
              <div class="form-grid">
                {#if c.kind === 'rack' || c.rear_rails_mm != null}<NumField id="c-r" label="Rails arrière (facultatif)" unit="mm" bind:value={c.rear_rails_mm} error={errors.rear_rails_mm} help="Distance des rails arrière au fond de la baie." />{/if}
                <NumField id="c-kg" label="Poids à vide" unit="kg" bind:value={c.empty_weight_kg} error={errors.empty_weight_kg} />
              </div>
            {:else if dims === 'desk'}
              {@const P = DESK[c.kind as 'desk' | 'furniture']}
              <div class="dim"><NumField id="c-l" label="Longueur" unit="mm" bind:value={c.length_mm} error={errors.length_mm} />{@render presets('Longueurs courantes', P.L, c.length_mm, (v) => setNum('length_mm', v))}</div>
              <div class="dim"><NumField id="c-w" label="Profondeur" unit="mm" bind:value={c.width_mm} error={errors.width_mm} />{@render presets('Profondeurs courantes', P.P, c.width_mm, (v) => setNum('width_mm', v))}</div>
              <div class="dim"><NumField id="c-h" label="Hauteur" unit="mm" bind:value={c.outer_height_mm} error={errors.outer_height_mm} />{@render presets('Hauteurs courantes', P.H, c.outer_height_mm, (v) => setNum('outer_height_mm', v))}</div>
              <div class="form-grid"><NumField id="c-kg" label="Poids à vide" unit="kg" bind:value={c.empty_weight_kg} error={errors.empty_weight_kg} /></div>
            {:else if dims === 'box'}
              <div class="fg4">
                <NumField id="c-l" label="Longueur" unit="mm" bind:value={c.length_mm} error={errors.length_mm} />
                <NumField id="c-w" label="Largeur" unit="mm" bind:value={c.width_mm} error={errors.width_mm} />
                <NumField id="c-d" label="Profondeur" unit="mm" bind:value={c.depth_mm} error={errors.depth_mm} />
                <NumField id="c-kg" label="Poids à vide" unit="kg" bind:value={c.empty_weight_kg} error={errors.empty_weight_kg} />
              </div>
            {/if}
          </section>
        {/if}

        <section class="adv">
          <button type="button" class="adv-t" aria-expanded={advanced} aria-controls="ce-adv" onclick={() => (advanced = !advanced)}>
            <Icon name={advanced ? 'chevd' : 'chevr'} size="sm" /><span>Champs avancés</span>
            <span class="muted small">{isContainer ? 'Code câble, limites de charge, notes' : 'Code câble, dimensions, poids, notes'}</span>
          </button>
          {#if advanced}
            <div class="adv-b" id="ce-adv">
              <div class="fg3">
                <div class="field"><label for="c-code">Code câble</label><input class="input mono" class:err={!!errors.cable_code} id="c-code" bind:value={c.cable_code} maxlength="5" placeholder="Tiré du nom" autocomplete="off" aria-invalid={errors.cable_code ? 'true' : undefined} aria-describedby={errors.cable_code ? 'c-code-err' : 'c-code-help'}>{#if !errors.cable_code}<span class="help" id="c-code-help">5 caractères : majuscules et chiffres.</span>{/if}<FieldError for="c-code" message={errors.cable_code} /></div>
                <NumField id="c-mkg" label="Poids maximum" unit="kg" bind:value={c.max_weight_kg} error={errors.max_weight_kg} />
                <NumField id="c-mw" label="Puissance maximum" unit="W" bind:value={c.max_power_w} error={errors.max_power_w} />
                {#if !isContainer}
                  <NumField id="c-l" label="Longueur" unit="mm" bind:value={c.length_mm} error={errors.length_mm} />
                  <NumField id="c-w" label="Largeur" unit="mm" bind:value={c.width_mm} error={errors.width_mm} />
                  <NumField id="c-d" label="Profondeur" unit="mm" bind:value={c.depth_mm} error={errors.depth_mm} />
                  <NumField id="c-kg" label="Poids à vide" unit="kg" bind:value={c.empty_weight_kg} error={errors.empty_weight_kg} />
                {/if}
              </div>
              <div class="field"><label for="c-notes">Notes (facultatif)</label><textarea class="input" id="c-notes" rows="2" value={c.notes ?? ''} oninput={(e) => (c.notes = (e.target as HTMLTextAreaElement).value || null)}></textarea></div>
            </div>
          {/if}
        </section>

        {#if general}<div class="alert err" role="alert">{general}</div>{/if}
      </div>
    </div>
  </div>
</Dialog>

<style>
  .ce { display: grid; gap: 20px; }
  .types { display: grid; gap: 12px; }
  .types-h { display: flex; gap: 8px 14px; align-items: center; flex-wrap: wrap; }
  .kcards { display: grid; grid-template-columns: repeat(auto-fill, minmax(146px, 1fr)); gap: 10px; }
  .kc { display: grid; justify-items: center; align-content: start; gap: 4px; text-align: center; cursor: pointer; font: inherit; min-width: 0; }
  .kc :global(.ki) { margin-bottom: 2px; }
  .kc.on :global(.ki) { color: var(--accent-2); }
  .kc b { font: 600 var(--fs-md)/1.3 var(--font); }
  .kc-h { font-size: var(--fs-sm); line-height: 1.35; color: var(--fg-3); }
  .kc:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  .ce-main { display: grid; grid-template-columns: minmax(0, 1fr) 264px; gap: 24px; align-items: start; }
  .ce-prev { grid-column: 2; grid-row: 1; position: sticky; top: 0; }
  .ce-form { grid-column: 1; grid-row: 1; display: grid; gap: 20px; min-width: 0; }
  .typebar { display: flex; gap: 12px; align-items: center; padding: 8px 10px 8px 12px; border-radius: var(--r-sm); background: var(--surface-2); box-shadow: inset 0 0 0 1px var(--line); }
  .typebar > div { display: grid; gap: 2px; flex: 1; min-width: 0; }
  .typebar b { font: 600 var(--fs-md)/1.3 var(--font); }

  .sec { display: grid; gap: 12px; }
  .sec h3 { margin: 0; }
  .toggles { display: flex; gap: 10px 22px; flex-wrap: wrap; }
  .dim { display: grid; grid-template-columns: 128px minmax(0, 1fr); gap: 6px 14px; align-items: start; }
  .presets { display: flex; flex-wrap: wrap; gap: 6px; padding-top: 25px; }
  .presets .fchip { min-width: 34px; padding: 0 8px; justify-content: center; font-family: var(--mono); font-variant-numeric: tabular-nums; font-weight: 600; }
  .presets .fchip:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  .post { display: grid; grid-template-columns: auto 1fr 1fr auto; gap: 8px; align-items: center; }
  .post-n { width: 24px; height: 24px; border-radius: 50%; display: grid; place-items: center; font-size: var(--fs-2xs); font-weight: 700; background: var(--surface-3); color: var(--fg-2); }
  .add-post { justify-self: start; }

  .adv { display: grid; gap: 14px; border-top: 1px solid var(--line); padding-top: 14px; }
  .adv-t { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; justify-self: start; padding: 4px 6px 4px 2px; margin-left: -2px; border: 0; border-radius: var(--btn-r-sm, 7px); background: none; font: 600 var(--fs-md)/1.3 var(--font); color: var(--fg); cursor: pointer; text-align: left; }
  .adv-t:hover { color: var(--accent); }
  .adv-t:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .adv-t .muted { font-weight: 400; }
  .adv-b { display: grid; gap: 14px; }

  @media (max-width: 860px) {
    .kcards { grid-template-columns: minmax(0, 1fr); gap: 8px; }
    .kc { grid-template-columns: 48px minmax(0, 1fr); grid-template-rows: auto auto; justify-items: start; align-items: center; column-gap: 12px; row-gap: 1px; text-align: left; padding: 8px 12px; }
    .kc :global(.ki) { grid-row: 1 / 3; width: 48px; height: 32px; margin: 0; }
    .ce-main { grid-template-columns: minmax(0, 1fr); gap: 20px; }
    .ce-prev, .ce-form { grid-column: 1; grid-row: auto; position: static; }
    .dim { grid-template-columns: minmax(0, 1fr); }
    .presets { padding-top: 0; }
    .post { grid-template-columns: auto 1fr auto; }
    .post input:nth-of-type(2) { grid-column: 2 / -1; }
  }
</style>
