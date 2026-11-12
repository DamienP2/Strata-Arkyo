<script lang="ts">
  // Ajouter ou modifier un élément du projet : d'abord le type (galerie par famille), puis
  // les champs communs et ceux propres au type (mur d'écrans, pupitre, caméra, énergie…).
  import { untrack } from 'svelte';
  import Dialog from '../../components/Dialog.svelte';
  import NumField from '../../components/NumField.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import KindIll from '../../components/KindIll.svelte';
  import Icon from '../../components/Icon.svelte';
  import { api, ApiError } from '../../lib/api';
  import { toast } from '../../lib/toast.svelte';
  import { CONTAINER_KINDS, ELEMENT_FAMILIES, ELEMENT_HINTS, U_PRESETS, blankContainer, type Container, type ContainerData, type ContainerKind, type DeskPosition, type ProjectView } from '../../lib/projects';

  let { view, container = null, locationId, onclose, ondone }: { view: ProjectView; container?: Container | null; locationId: string; onclose: () => void; ondone: (id: string) => void } = $props();
  // Le dialogue part d'une copie : les changements ne s'appliquent qu'à l'enregistrement.
  let step = $state<'type' | 'form'>(untrack(() => (container ? 'form' : 'type')));
  let name = $state(untrack(() => container?.name ?? ''));
  let named = $state(untrack(() => !!container));
  let loc = $state(untrack(() => container?.location_id ?? locationId));
  let c = $state<ContainerData & Partial<Container>>(untrack(() => (container ? JSON.parse(JSON.stringify({ ...container, details: { ...blankContainer(container.kind).details, ...(container.details ?? {}) } })) : blankContainer('rack'))));
  let errors = $state<Record<string, string>>({});
  let general = $state<string | null>(null);
  const K = $derived(CONTAINER_KINDS[c.kind as ContainerKind]);
  const d = $derived(c.details);

  function pick(k: ContainerKind) {
    if (container) { c.kind = k; if (!CONTAINER_KINDS[k].mobile) c.wheels = false; if (!CONTAINER_KINDS[k].canRack) c.rackable = false; c.details = { ...blankContainer(k).details, ...c.details }; }
    else c = { ...blankContainer(k), empty_weight_kg: c.empty_weight_kg, max_weight_kg: c.max_weight_kg, max_power_w: c.max_power_w };
    if (!named) name = `${CONTAINER_KINDS[k].label} ${view.containers.filter((x) => x.kind === k).length + 1}`;
    step = 'form';
  }
  async function save(e: SubmitEvent) {
    e.preventDefault(); errors = {}; general = null;
    const { id: _i, project_id: _p, position: _o, rev: _r, totals: _t, ...data } = $state.snapshot(c) as Container;
    const body = { ...data, name, location_id: loc };
    try {
      const r = container ? await api.patch<Container>(`/containers/${container.id}`, { ...body, rev: container.rev }) : await api.post<Container>(`/projects/${view.project.id}/containers`, body);
      toast(container ? 'Élément enregistré.' : r.name !== name.trim() ? `Élément ajouté sous le nom « ${r.name} ».` : 'Élément ajouté.');
      ondone(r.id);
    } catch (x) {
      if (x instanceof ApiError) { errors = Object.fromEntries(x.fields.map((f) => [f.field.replace(/^details\./, ''), f.message])); general = x.fields.length ? null : x.message; } else general = 'Enregistrement impossible.';
    }
  }
  const txt = (k: string) => (e: Event) => { (c.details as Record<string, unknown>)[k] = (e.target as HTMLInputElement).value || null; };
  function addPost() { c.details.positions = [...(c.details.positions ?? []), { id: `p${Date.now().toString(36)}`, role: '', name: '', instance_ids: [] }]; }
</script>

{#snippet hint(id: string, list: string[])}<datalist {id}>{#each list as v}<option value={v}></option>{/each}</datalist>{/snippet}

<Dialog title={container ? `Modifier ${container.name}` : step === 'type' ? 'Ajouter un élément' : `Nouveau : ${K.label}`} eyebrow={view.project.name} {onclose} wide>
  {#if step === 'type'}
    <div class="gal">
      {#each ELEMENT_FAMILIES as f (f.key)}
        <section>
          <div class="gal-h"><span class="lbl-t">{f.label}</span><span class="muted small">{f.help}</span></div>
          <div class="gal-g">
            {#each Object.entries(CONTAINER_KINDS).filter(([, v]) => v.family === f.key && !v.hidden) as [k, v] (k)}
              <button type="button" class="kc" onclick={() => pick(k as ContainerKind)}>
                <KindIll kind={k as ContainerKind} size={84} />
                <b>{v.label}</b><span class="muted small">{v.help}</span>
              </button>
            {/each}
          </div>
        </section>
      {/each}
    </div>
  {:else}
    <form id="contf" class="stack" onsubmit={save}>
      <div class="kind-bar">
        <KindIll kind={c.kind} size={60} />
        <div><b>{K.label}</b><span class="muted small">{K.help}</span></div>
        {#if !container}<button type="button" class="btn sm ghost" onclick={() => (step = 'type')}><Icon name="undo" size="sm" />Changer de type</button>{/if}
      </div>
      <div class="fg3">
        <div class="field"><label for="c-name">Nom</label><input class="input" class:err={!!errors.name} id="c-name" bind:value={name} oninput={() => (named = true)} required maxlength="120"><FieldError message={errors.name} /></div>
        <div class="field"><label for="c-loc">Emplacement</label><select class="input" id="c-loc" bind:value={loc}>{#each view.locations as l}<option value={l.id}>{l.name}</option>{/each}</select><FieldError message={errors.location_id} /></div>
        <div class="field"><label for="c-code">Code câble</label><input class="input mono" id="c-code" bind:value={c.cable_code} maxlength="5" placeholder="Tiré du nom"><span class="help">5 caractères : majuscules et chiffres.</span></div>
      </div>

      {#if c.kind === 'monitor_wall'}
        {@const w = d.wall!}
        <div class="spec">
          <span class="lbl-t">Mur d’écrans</span>
          <div class="fg4">
            <NumField id="w-r" label="Rangées" bind:value={w.rows} min={1} max={12} step={1} error={errors['wall.rows']} />
            <NumField id="w-c" label="Colonnes" bind:value={w.cols} min={1} max={16} step={1} error={errors['wall.cols']} />
            <div class="field" style="grid-column:span 2"><label for="w-s">Support</label><input class="input" id="w-s" list="h-sup" value={w.support ?? ''} oninput={(e) => (w.support = (e.target as HTMLInputElement).value || null)} placeholder="Structure alu, pieds…">{@render hint('h-sup', ELEMENT_HINTS.support!)}</div>
          </div>
          <div class="wall-prev" style="--r:{w.rows || 1};--c:{w.cols || 1}">{#each Array((w.rows || 1) * (w.cols || 1)) as _}<i></i>{/each}</div>
          <span class="muted small">Ajoutez les écrans à l’élément, puis placez-les sur la grille depuis l’inventaire.</span>
          {#if errors['wall.tiles'] || Object.keys(errors).some((k) => k.startsWith('wall.tiles'))}<div class="alert err">Réduire le mur ferait sortir des écrans de la grille : retirez-les d’abord.</div>{/if}
        </div>
      {:else if c.kind === 'desk' || c.kind === 'operator_position'}
        <div class="spec">
          <span class="lbl-t">{c.kind === 'desk' ? 'Postes du pupitre' : 'Poste'}</span>
          {#each d.positions ?? [] as p, i (p.id)}
            <div class="post">
              <span class="post-n mono">{i + 1}</span>
              <input class="input" list="h-roles" bind:value={p.role} placeholder="Rôle (réalisateur, scripte…)" aria-label="Rôle du poste {i + 1}" required>
              <input class="input" bind:value={p.name} placeholder="Personne prévue (facultatif)" aria-label="Personne du poste {i + 1}">
              {#if c.kind === 'desk'}<button type="button" class="icon-btn sm" aria-label="Retirer le poste {i + 1}" onclick={() => (c.details.positions = d.positions!.filter((x: DeskPosition) => x !== p))}><Icon name="x" size="sm" /></button>{/if}
            </div>
          {/each}
          {@render hint('h-roles', ELEMENT_HINTS.roles!)}
          {#if c.kind === 'desk' && (d.positions?.length ?? 0) < 24}<button type="button" class="btn sm ghost" style="justify-self:start" onclick={addPost}><Icon name="plus" size="sm" />Ajouter un poste</button>{/if}
          <FieldError message={errors.positions} />
        </div>
      {:else if c.kind === 'zone'}
        <div class="spec"><span class="lbl-t">Emplacement libre</span><div class="fg3">
          <div class="field"><label for="z-s">Surface</label><input class="input" id="z-s" list="h-surf" value={d.surface ?? ''} oninput={txt('surface')} placeholder="Table, sol, loge…">{@render hint('h-surf', ELEMENT_HINTS.surface!)}</div>
          <NumField id="z-a" label="Superficie" unit="m²" bind:value={c.details.area_m2!} error={errors.area_m2} />
        </div></div>
      {:else if c.kind === 'camera_position'}
        <div class="spec"><span class="lbl-t">Position caméra</span><div class="fg3">
          <NumField id="k-n" label="Numéro de caméra" bind:value={c.details.camera_number!} min={1} max={999} step={1} error={errors.camera_number} />
          <div class="field"><label for="k-m">Support</label><input class="input" id="k-m" list="h-mount" value={d.mount ?? ''} oninput={txt('mount')}>{@render hint('h-mount', ELEMENT_HINTS.mount!)}</div>
          <div class="field"><label for="k-l">Liaison</label><input class="input" id="k-l" list="h-link" value={d.link ?? ''} oninput={txt('link')}>{@render hint('h-link', ELEMENT_HINTS.link!)}</div>
        </div></div>
      {:else if c.kind === 'rf_mast'}
        <div class="spec"><span class="lbl-t">Mât HF</span><div class="fg3">
          <NumField id="m-h" label="Hauteur" unit="m" bind:value={c.details.mast_height_m!} error={errors.mast_height_m} />
          <div class="field" style="grid-column:span 2"><label for="m-f">Fréquences</label><input class="input mono" id="m-f" value={d.frequencies ?? ''} oninput={txt('frequencies')} placeholder="606,100 · 614,350 MHz…"></div>
        </div></div>
      {:else if c.kind === 'power'}
        <div class="spec"><span class="lbl-t">Énergie</span><div class="fg3">
          <NumField id="p-k" label="Puissance" unit="kVA" bind:value={c.details.power_kva!} error={errors.power_kva} />
          <div class="field"><span class="flabel">Phases</span><div class="seg">{#each [[1, 'Mono'], [3, 'Tri']] as [v, l]}<button type="button" class:on={d.phases === v} onclick={() => (c.details.phases = v as number)}>{l}</button>{/each}</div></div>
          <div class="field"><label for="p-o">Sortie</label><input class="input" id="p-o" list="h-out" value={d.outlet ?? ''} oninput={txt('outlet')}>{@render hint('h-out', ELEMENT_HINTS.outlet!)}</div>
        </div></div>
      {:else if c.kind === 'external_feed'}
        <div class="spec"><span class="lbl-t">Arrivée externe</span><div class="fg3">
          <div class="field"><label for="x-f">Liaison</label><input class="input" id="x-f" list="h-feed" value={d.feed ?? ''} oninput={txt('feed')}>{@render hint('h-feed', ELEMENT_HINTS.feed!)}</div>
          <div class="field"><label for="x-p">Opérateur</label><input class="input" id="x-p" value={d.provider ?? ''} oninput={txt('provider')}></div>
          <div class="field"><label for="x-r">Référence du circuit</label><input class="input mono" id="x-r" value={d.circuit_ref ?? ''} oninput={txt('circuit_ref')}></div>
        </div></div>
      {:else if c.kind === 'site_point' || c.kind === 'field_box'}
        <div class="spec"><span class="lbl-t">{K.label}</span><div class="fg3">
          <div class="field"><label for="s-k">Nature</label><input class="input" id="s-k" list="h-pt" value={d.point_kind ?? ''} oninput={txt('point_kind')}>{@render hint('h-pt', ELEMENT_HINTS.point_kind!)}</div>
        </div></div>
      {:else if c.kind === 'display'}
        <div class="spec"><span class="lbl-t">Diffusion</span><div class="fg3">
          <div class="field"><label for="d-k">Nature</label><input class="input" id="d-k" list="h-disp" value={d.display_kind ?? ''} oninput={txt('display_kind')}>{@render hint('h-disp', ELEMENT_HINTS.display_kind!)}</div>
          <NumField id="d-d" label="Diagonale" unit="″" bind:value={c.details.diagonal_in!} error={errors.diagonal_in} />
          <div class="field"><label for="d-r">Résolution</label><input class="input mono" id="d-r" value={d.resolution ?? ''} oninput={txt('resolution')} placeholder="3840 × 2160"></div>
        </div></div>
      {:else if c.kind === 'cable_drum'}
        <div class="spec"><span class="lbl-t">Touret</span><div class="fg3">
          <NumField id="t-l" label="Longueur" unit="m" bind:value={c.details.cable_length_m!} error={errors.cable_length_m} />
          <NumField id="t-n" label="Nombre" bind:value={c.details.drum_count!} min={1} step={1} error={errors.drum_count} />
        </div></div>
      {/if}

      <div style="display:flex;gap:18px;flex-wrap:wrap">
        {#if K.canRack}<label class="toggle-row"><input type="checkbox" bind:checked={c.rackable}>Rackable (19″)</label>{/if}
        {#if K.mobile}<label class="toggle-row"><input type="checkbox" bind:checked={c.wheels}>Roulettes</label>{/if}
      </div>
      <div class="fg4">
        {#if c.rackable}
          <NumField id="c-u" label="Hauteur" unit="U" bind:value={c.height_u} suggestions={U_PRESETS} error={errors.height_u} />
          <NumField id="c-d" label="Profondeur" unit="mm" bind:value={c.depth_mm} error={errors.depth_mm} />
          <NumField id="c-r" label="Rails arrière" unit="mm" bind:value={c.rear_rails_mm} error={errors.rear_rails_mm} help="Distance au fond ; vide sans rails." />
        {:else}
          <NumField id="c-l" label="Longueur" unit="mm" bind:value={c.length_mm} error={errors.length_mm} />
          <NumField id="c-w" label="Largeur" unit="mm" bind:value={c.width_mm} error={errors.width_mm} />
          <NumField id="c-d" label="Profondeur" unit="mm" bind:value={c.depth_mm} error={errors.depth_mm} />
        {/if}
        <NumField id="c-kg" label="Poids à vide" unit="kg" bind:value={c.empty_weight_kg} error={errors.empty_weight_kg} />
        <NumField id="c-mkg" label="Poids maximum" unit="kg" bind:value={c.max_weight_kg} error={errors.max_weight_kg} />
        <NumField id="c-mw" label="Puissance maximum" unit="W" bind:value={c.max_power_w} error={errors.max_power_w} />
      </div>
      <div class="field"><label for="c-notes">Notes</label><textarea class="input" id="c-notes" rows="2" style="height:auto;padding:12px 14px" value={c.notes ?? ''} oninput={(e) => (c.notes = (e.target as HTMLTextAreaElement).value || null)}></textarea></div>
      {#if general}<div class="alert err">{general}</div>{/if}
    </form>
  {/if}
  {#snippet footer()}<button class="btn ghost" onclick={onclose}>Annuler</button>{#if step === 'form'}<button class="btn primary" type="submit" form="contf">{container ? 'Enregistrer' : 'Ajouter l’élément'}</button>{/if}{/snippet}
</Dialog>

<style>
  .gal { display: grid; gap: 18px; }
  .gal-h { display: flex; gap: 10px; align-items: baseline; flex-wrap: wrap; margin-bottom: 8px; }
  .gal-g { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px; }
  .kc { display: grid; justify-items: center; gap: 4px; padding: 12px 10px; border-radius: 16px; border: 1.5px solid var(--line); background: var(--surface); cursor: pointer; font: inherit; color: inherit; text-align: center; transition: border-color .15s, transform .15s, box-shadow .15s; }
  .kc:hover { border-color: var(--fg); transform: translateY(-1px); box-shadow: 0 8px 18px -10px rgba(0, 0, 0, .3); }
  .kc b { font-size: 14.5px; }
  .kc .small { font-size: 12px; line-height: 1.3; }
  .kind-bar { display: flex; gap: 14px; align-items: center; padding: 10px 14px; border-radius: 16px; background: var(--surface-2); }
  .kind-bar > div { display: flex; flex-direction: column; flex: 1; }
  .spec { display: grid; gap: 10px; padding: 14px; border-radius: 16px; border: 1.5px dashed var(--line); }
  .post { display: grid; grid-template-columns: auto 1fr 1fr auto; gap: 8px; align-items: center; }
  .post-n { width: 24px; height: 24px; border-radius: 50%; display: grid; place-items: center; font-size: 11px; background: var(--fg); color: var(--bg); }
  .wall-prev { display: grid; grid-template-columns: repeat(var(--c), 1fr); gap: 4px; max-width: 360px; padding: 8px; border-radius: 10px; background: #15181C; }
  .wall-prev i { aspect-ratio: 16 / 9; border-radius: 3px; background: linear-gradient(180deg, #2C3138, #1E2227); box-shadow: inset 0 0 0 1px #3A4048; }
  @media (max-width: 560px) { .post { grid-template-columns: auto 1fr auto; } .post input:nth-of-type(2) { grid-column: 2 / -1; } }
</style>
