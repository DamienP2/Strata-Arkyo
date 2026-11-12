<script lang="ts">
  // Assistant de création (section 7.3) : une question par étape, récapitulatif, puis
  // création du projet, de l'emplacement et de l’élément.
  import Icon from '../../components/Icon.svelte';
  import Ill from '../../components/Ill.svelte';
  import NumField from '../../components/NumField.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import { api, ApiError } from '../../lib/api';
  import { navigate } from '../../lib/router.svelte';
  import { toast } from '../../lib/toast.svelte';
  import { REGIE, CONTAINER_KINDS, U_PRESETS, blankContainer, fdate, type ContainerKind, type Regie } from '../../lib/projects';

  let { onclose, clients = [] }: { onclose: () => void; clients?: string[] } = $props();
  const today = new Date().toISOString().slice(0, 10);

  let d = $state({
    regie: 'car' as Regie, name: '', external: true, client: '',
    planning_start: today, planning_end: '', shoot_start: '', shoot_end: '', venue: '', location: '', description: '', service_type: '',
  });
  let cname = $state('Baie 1');
  let c = $state(blankContainer('rack'));
  let step = $state(0);
  let busy = $state(false);
  let errors = $state<Record<string, string>>({});
  let taken = $state(false);
  let timer: ReturnType<typeof setTimeout>;

  const STEPS = ['regie', 'name', 'client', 'planning', 'dates', 'venue', 'location', 'container', 'description', 'recap'] as const;
  type Step = (typeof STEPS)[number];
  const cur = $derived(STEPS[step]!);
  const FIELD_STEP: Record<string, Step> = { regie: 'regie', name: 'name', client: 'client', planning_start: 'planning', planning_end: 'planning', shoot_start: 'dates', shoot_end: 'dates', venue: 'venue', service_type: 'venue', description: 'description' };

  function checkName() {
    clearTimeout(timer);
    const n = d.name.trim();
    if (!n) { taken = false; return; }
    timer = setTimeout(async () => { taken = (await api.get<{ taken: boolean }>(`/projects/check-name?name=${encodeURIComponent(n)}`)).taken; }, 250);
  }
  function setKind(k: ContainerKind) { c = { ...blankContainer(k), empty_weight_kg: c.empty_weight_kg }; }

  /** Erreur bloquante de l'étape courante, avant d'aller plus loin. */
  function stepError(s: Step): Record<string, string> {
    const e: Record<string, string> = {};
    if (s === 'name') { if (!d.name.trim()) e.name = 'Le nom est obligatoire.'; else if (taken) e.name = 'Un projet porte déjà ce nom.'; }
    if (s === 'client' && d.external && !d.client.trim()) e.client = 'Indiquez le nom du client.';
    if (s === 'planning') {
      if (!d.planning_start || !d.planning_end) e.planning_end = 'Le début et la fin sont obligatoires.';
      else if (d.planning_start < today) e.planning_start = 'La date est passée.';
      else if (d.planning_end < d.planning_start) e.planning_end = 'La fin précède le début.';
    }
    if (s === 'dates') {
      if (!d.shoot_start || !d.shoot_end) e.shoot_end = 'Le début et la fin de l’exploitation sont obligatoires.';
      else if (d.shoot_start < today) e.shoot_start = 'La date est passée.';
      else if (d.shoot_end < d.shoot_start) e.shoot_end = 'La fin précède le début.';
    }
    if (s === 'location' && !d.location.trim()) e.location = 'Le nom de l’emplacement est obligatoire.';
    if (s === 'container' && !cname.trim()) e.cname = 'Le nom de l’élément est obligatoire.';
    return e;
  }
  function next() {
    errors = stepError(cur);
    if (Object.keys(errors).length) return;
    step = Math.min(step + 1, STEPS.length - 1);
  }
  async function create() {
    busy = true; errors = {};
    const body = {
      name: d.name, client: d.external ? d.client : null, venue: d.venue || null, description: d.description || null, service_type: d.service_type || null,
      regie: d.regie, planning_start: d.planning_start, planning_end: d.planning_end,
      shoot_start: d.shoot_start || null, shoot_end: d.shoot_end || null,
      location: { name: d.location }, container: { name: cname, ...c },
    };
    try {
      const r = await api.post<{ project: { id: string }; container_error: { message: string } | null }>('/projects', body);
      if (r.container_error) toast(r.container_error.message, 'warn'); else toast('Projet créé.');
      onclose();
      navigate(`/projets/${r.project.id}/inventaire`);
    } catch (e) {
      if (e instanceof ApiError && e.fields.length) {
        errors = Object.fromEntries(e.fields.map((f) => [f.field, f.message]));
        const f = e.fields[0]!.field;
        const target: Step = f.startsWith('location') ? 'location' : (FIELD_STEP[f] ?? 'recap');
        step = STEPS.indexOf(target);
      } else errors = { general: e instanceof Error ? e.message : 'Création impossible.' };
    } finally { busy = false; }
  }
  const recap = $derived([
    ['regie', 'Régie', REGIE[d.regie].label],
    ['name', 'Nom', d.name],
    ['client', 'Client', d.external ? d.client : 'Projet interne'],
    ['planning', 'Préparation', `${fdate(d.planning_start)} → ${fdate(d.planning_end)}`],
    ['dates', 'Exploitation', `${fdate(d.shoot_start)} → ${fdate(d.shoot_end)}`],
    ['venue', 'Lieu', [d.venue, d.service_type].filter(Boolean).join(' · ') || '—'],
    ['location', 'Emplacement', d.location],
    ['container', 'Élément', `${cname} · ${CONTAINER_KINDS[c.kind].label}${c.rackable ? ` · ${c.height_u} U` : ''}`],
    ['description', 'Description', d.description || '—'],
  ] as [Step, string, string][]);
  function key(e: KeyboardEvent) { if (e.key === 'Escape') onclose(); }
</script>

<svelte:window onkeydown={key} />
<div class="modal" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Nouveau projet">
    <div class="wz-head">
      <div class="row"><span class="eyebrow">Nouveau projet · étape {step + 1} sur {STEPS.length}</span><button class="icon-btn sm" onclick={onclose} aria-label="Fermer"><Icon name="x" /></button></div>
      <div class="bar"><i style="width:{(100 * (step + 1)) / STEPS.length}%"></i></div>
    </div>
    <form class="wz-body" onsubmit={(e) => { e.preventDefault(); if (cur === 'recap') create(); else next(); }}>
      {#if cur === 'regie'}
        <h2>Quel type de régie ?</h2>
        <div class="choices">
          {#each Object.entries(REGIE) as [k, r]}
            <button type="button" class="choice" class:on={d.regie === k} onclick={() => (d.regie = k as Regie)}>
              <span class="ch-vis"><Ill kind={r.ill} /></span><b>{r.label}</b>
            </button>
          {/each}
        </div>
        <p class="muted small">Modifiable ensuite dans la fiche du projet.</p>
      {:else if cur === 'name'}
        <h2>Comment s’appelle-t-il ?</h2>
        <div class="field"><label for="wz-name">Nom du projet</label><input class="input" id="wz-name" bind:value={d.name} oninput={checkName} maxlength="120" autocomplete="off" placeholder="Ex. Finale Coupe Atlantique">
          {#if taken}<span class="help err">Un projet porte déjà ce nom.</span>{:else if d.name.trim()}<span class="help ok">Nom disponible.</span>{/if}<FieldError message={errors.name} /></div>
      {:else if cur === 'client'}
        <h2>Pour quel client ?</h2>
        <div class="seg" style="align-self:flex-start"><button type="button" class:on={!d.external} onclick={() => (d.external = false)}>Projet interne</button><button type="button" class:on={d.external} onclick={() => (d.external = true)}>Client externe</button></div>
        {#if d.external}<div class="field"><label for="wz-client">Nom du client</label><input class="input" id="wz-client" bind:value={d.client} list="wz-clients" maxlength="120"><datalist id="wz-clients">{#each clients as cl}<option value={cl}></option>{/each}</datalist><FieldError message={errors.client} /></div>{/if}
      {:else if cur === 'planning'}
        <h2>Période de préparation</h2>
        <p class="muted">Conception, commandes, câblage en atelier.</p>
        <div class="form-grid">
          <div class="field"><label for="wz-ps">Début</label><input class="input" type="date" id="wz-ps" bind:value={d.planning_start} min={today}><FieldError message={errors.planning_start} /></div>
          <div class="field"><label for="wz-pe">Fin</label><input class="input" type="date" id="wz-pe" bind:value={d.planning_end} min={d.planning_start || today}><FieldError message={errors.planning_end} /></div>
        </div>
      {:else if cur === 'dates'}
        <h2>Période d’exploitation</h2>
        <p class="muted">Tournage, événement ou mise en service : un seul jour si début et fin sont identiques.</p>
        <div class="form-grid">
          <div class="field"><label for="wz-ss">Début</label><input class="input" type="date" id="wz-ss" bind:value={d.shoot_start} min={today}><FieldError message={errors.shoot_start} /></div>
          <div class="field"><label for="wz-se">Fin</label><input class="input" type="date" id="wz-se" bind:value={d.shoot_end} min={d.shoot_start || today}><FieldError message={errors.shoot_end} /></div>
        </div>
      {:else if cur === 'venue'}
        <h2>Où se passe-t-il ?</h2>
        <div class="form-grid">
          <div class="field"><label for="wz-venue">Lieu</label><input class="input" id="wz-venue" bind:value={d.venue} placeholder="Ex. Arena Loire, Nantes"></div>
          <div class="field"><label for="wz-tp">Type de projet</label><input class="input" id="wz-tp" bind:value={d.service_type} list="wz-tps" placeholder="Sport, concert, TV…"><datalist id="wz-tps">{#each ['Sport', 'Concert', 'Plateau TV', 'Conférence', 'Théâtre', 'Studio'] as t}<option value={t}></option>{/each}</datalist></div>
        </div>
      {:else if cur === 'location'}
        <h2>Premier emplacement</h2>
        <p class="muted">Un emplacement regroupe des éléments : la régie, le bord du terrain, la cabine commentateurs… Son code entre dans la numérotation des câbles.</p>
        <div class="field"><label for="wz-loc">Nom</label><input class="input" id="wz-loc" bind:value={d.location} maxlength="120" placeholder="Ex. Car régie CR-2"><FieldError message={errors.location ?? errors['location.name']} /></div>
      {:else if cur === 'container'}
        <h2>Premier élément</h2>
        <div class="choices" style="grid-template-columns:repeat(auto-fit,minmax(130px,1fr))">
          {#each Object.entries(CONTAINER_KINDS).filter(([, v]) => !v.hidden) as [k, v]}<button type="button" class="choice" class:on={c.kind === k} onclick={() => setKind(k as ContainerKind)}><b style="font-size:17px">{v.label}</b></button>{/each}
        </div>
        <div class="form-grid">
          <div class="field"><label for="wz-cn">Nom</label><input class="input" id="wz-cn" bind:value={cname} maxlength="120"><FieldError message={errors.cname} /></div>
          <div class="field" style="justify-content:flex-end;gap:8px">
            <label class="toggle-row"><input type="checkbox" bind:checked={c.rackable}>Rackable (19″)</label>
            {#if CONTAINER_KINDS[c.kind].mobile}<label class="toggle-row"><input type="checkbox" bind:checked={c.wheels}>Roulettes</label>{/if}
          </div>
          {#if c.rackable}
            <NumField id="wz-u" label="Hauteur" unit="U" bind:value={c.height_u} suggestions={U_PRESETS} />
            <NumField id="wz-d" label="Profondeur" unit="mm" bind:value={c.depth_mm} />
          {:else}
            <NumField id="wz-l" label="Longueur" unit="mm" bind:value={c.length_mm} />
            <NumField id="wz-w" label="Largeur" unit="mm" bind:value={c.width_mm} />
          {/if}
          <NumField id="wz-kg" label="Poids à vide" unit="kg" bind:value={c.empty_weight_kg} />
          {#if c.rackable}<NumField id="wz-rails" label="Rails arrière (distance au fond)" unit="mm" bind:value={c.rear_rails_mm} help="Laisser vide sans rails arrière." />{/if}
        </div>
      {:else if cur === 'description'}
        <h2>Une description ?</h2>
        <div class="field"><label for="wz-desc">Description (facultative)</label><textarea class="input" id="wz-desc" rows="4" style="height:auto;padding:12px 14px" bind:value={d.description}></textarea></div>
      {:else}
        <h2>Récapitulatif</h2>
        <div class="recap">
          {#each recap as [s, l, v]}<button type="button" onclick={() => (step = STEPS.indexOf(s))}><span>{l}</span><b>{v}</b><Icon name="pen" /></button>{/each}
        </div>
        {#if errors.general}<div class="alert err">{errors.general}</div>{/if}
      {/if}
      <button type="submit" hidden aria-hidden="true" tabindex="-1"></button>
    </form>
    <div class="wz-foot">
      <button class="btn ghost" onclick={() => (step = Math.max(0, step - 1))} disabled={step === 0}><Icon name="chevl" size="sm" />Retour</button>
      {#if cur === 'recap'}<button class="btn primary" onclick={create} disabled={busy}>{#if busy}<span class="spin"></span>{/if}Créer le projet</button>
      {:else}<button class="btn primary" onclick={next}>Continuer<Icon name="chevr" size="sm" /></button>{/if}
    </div>
  </div>
</div>
