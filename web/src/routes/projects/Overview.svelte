<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Vu from '../../components/Vu.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { navigate } from '../../lib/router.svelte';
  import { nf } from '../../lib/format';
  import { REGIE, type Project, type ProjectView } from '../../lib/projects';

  let { view = $bindable(), reload }: { view: ProjectView; reload: () => Promise<void> } = $props();
  const writable = $derived(can('projets', 'ecriture'));
  let form = $state<Project>(structuredClone($state.snapshot(view.project)));
  let saved = $state(JSON.stringify(view.project));
  let errors = $state<Record<string, string>>({});
  let busy = $state(false);
  let deleting = $state(false);
  // Une mise à jour venue d'un autre membre remplace le formulaire s'il n'a pas été touché.
  $effect(() => {
    const incoming = JSON.stringify(view.project);
    if (incoming !== saved && JSON.stringify(form) === saved) { form = structuredClone($state.snapshot(view.project)); saved = incoming; }
  });
  const dirty = $derived(JSON.stringify(form) !== saved);
  const txt = (k: keyof Project) => (e: Event) => ((form as any)[k] = (e.target as HTMLInputElement).value || null);

  async function save(e: SubmitEvent) {
    e.preventDefault(); busy = true; errors = {};
    try {
      const p = await api.patch<Project>(`/projects/${form.id}`, form);
      view.project = p; form = structuredClone(p); saved = JSON.stringify(p); toast('Fiche enregistrée.');
    } catch (x) {
      if (x instanceof ApiError && x.code === 'stale') { toast('Un autre membre a modifié la fiche : elle est rechargée.', 'warn'); await reload(); form = structuredClone($state.snapshot(view.project)); saved = JSON.stringify(view.project); }
      else if (x instanceof ApiError && x.fields.length) errors = Object.fromEntries(x.fields.map((f) => [f.field, f.message]));
      else fail(x);
    } finally { busy = false; }
  }
  async function remove() {
    try { await api.del(`/projects/${view.project.id}`); toast('Projet supprimé.'); navigate('/projets'); } catch (e) { fail(e); }
  }
  const p = $derived(view.project);
  const units = $derived(view.instances.length);
  const missing = $derived([
    !p.client && 'le client',
    !p.venue && 'le lieu',
    !p.description && 'une description',
    !p.service_type && 'le type de prestation',
    !(p.planning_start && p.planning_end) && 'la période de planification',
    !(p.intervention === 'prestation' ? p.shoot_start && p.shoot_end : p.commissioning) && (p.intervention === 'prestation' ? 'les dates de tournage' : 'la date de mise en service'),
    !view.locations.length && 'un emplacement',
    !view.containers.length && 'un contenant',
    !view.articles.length && 'du matériel',
  ].filter(Boolean) as string[]);
  const openTasks = $derived(view.tasks.filter((t) => t.status === 'todo' || t.status === 'in_progress').length);
</script>

<div class="grid-2" style="grid-template-columns:minmax(0,1.6fr) minmax(0,1fr);align-items:start">
  <section class="card">
    <div class="card-head"><h2>Fiche du projet</h2><span class="chip">{p.intervention === 'prestation' ? 'Prestation' : 'Intégration'} · {REGIE[p.regie].label}</span></div>
    <form class="form-grid" style="margin-top:16px" onsubmit={save}>
      <fieldset disabled={!writable} style="display:contents">
        <div class="field span"><label for="o-name">Nom</label><input class="input" id="o-name" bind:value={form.name} required maxlength="120"><FieldError message={errors.name} /></div>
        <div class="field"><label for="o-client">Client</label><input class="input" id="o-client" value={form.client ?? ''} oninput={txt('client')} placeholder="Vide : projet interne"></div>
        <div class="field"><label for="o-tp">Type de prestation</label><input class="input" id="o-tp" value={form.service_type ?? ''} oninput={txt('service_type')} placeholder="Sport, concert, TV…"></div>
        <div class="field span"><label for="o-venue">Lieu</label><input class="input" id="o-venue" value={form.venue ?? ''} oninput={txt('venue')}></div>
        <div class="field"><label for="o-ps">Planification : début</label><input class="input" type="date" id="o-ps" value={form.planning_start ?? ''} oninput={txt('planning_start')}><FieldError message={errors.planning_start} /></div>
        <div class="field"><label for="o-pe">Planification : fin</label><input class="input" type="date" id="o-pe" value={form.planning_end ?? ''} oninput={txt('planning_end')}><FieldError message={errors.planning_end} /></div>
        {#if form.intervention === 'prestation'}
          <div class="field"><label for="o-ss">Tournage : début</label><input class="input" type="date" id="o-ss" value={form.shoot_start ?? ''} oninput={txt('shoot_start')}><FieldError message={errors.shoot_start} /></div>
          <div class="field"><label for="o-se">Tournage : fin</label><input class="input" type="date" id="o-se" value={form.shoot_end ?? ''} oninput={txt('shoot_end')}><FieldError message={errors.shoot_end} /></div>
        {:else}
          <div class="field"><label for="o-cm">Mise en service</label><input class="input" type="date" id="o-cm" value={form.commissioning ?? ''} oninput={txt('commissioning')}><FieldError message={errors.commissioning} /></div>
          <div class="field"><label for="o-regie">Type de régie</label><select class="input" id="o-regie" bind:value={form.regie}>{#each Object.entries(REGIE) as [k, r]}<option value={k}>{r.label}</option>{/each}</select></div>
        {/if}
        {#if form.intervention === 'prestation'}<div class="field span"><span class="flabel">Type de régie</span><div class="seg">{#each Object.entries(REGIE).filter(([k]) => k !== 'fixed') as [k, r]}<button type="button" class:on={form.regie === k} onclick={() => (form.regie = k as Project['regie'])}>{r.label}</button>{/each}</div></div>{/if}
        <div class="field span"><label for="o-desc">Description</label><textarea class="input" id="o-desc" rows="4" style="height:auto;padding:12px 14px" value={form.description ?? ''} oninput={txt('description')}></textarea></div>
        {#if writable}<div class="span row-actions"><button class="btn ghost" type="button" disabled={!dirty} onclick={() => { form = structuredClone($state.snapshot(view.project)); errors = {}; }}>Annuler</button><button class="btn primary" disabled={!dirty || busy}>{#if busy}<span class="spin"></span>{/if}Enregistrer</button></div>{/if}
      </fieldset>
    </form>
  </section>

  <div class="stack" style="gap:20px">
    <section class="card">
      <div class="card-head"><div><p class="eyebrow">Complétion de la fiche</p><h2 style="font:760 48px/1 var(--display);letter-spacing:-.04em">{view.completion}<small style="font-size:.45em;color:var(--fg-3)"> %</small></h2></div></div>
      <div style="margin-top:12px"><Vu value={view.completion} n={24} cls={view.completion >= 80 ? 'ok' : ''} /></div>
      {#if missing.length}<p class="muted small" style="margin-top:12px">À compléter : {missing.join(', ')}.</p>{/if}
    </section>
    <section class="card">
      <div class="card-head"><h2>Inventaire</h2><a class="btn sm" href="/projets/{p.id}/inventaire">Ouvrir</a></div>
      <dl class="kv" style="margin-top:12px">
        <dt>Emplacements</dt><dd>{view.locations.length}</dd>
        <dt>Contenants</dt><dd>{view.containers.length}</dd>
        <dt>Articles</dt><dd>{nf(view.articles.reduce((s, a) => s + a.quantity, 0))}</dd>
        <dt>Exemplaires repérés</dt><dd>{units}</dd>
        <dt>Poids total</dt><dd>{nf(view.locations.reduce((s, l) => s + l.totals.weight_kg, 0), 1)} kg</dd>
        <dt>Consommation</dt><dd>{nf(view.locations.reduce((s, l) => s + l.totals.power_w, 0))} W</dd>
      </dl>
    </section>
    <section class="card">
      <div class="card-head"><h2>Tâches</h2><a class="btn sm" href="/projets/{p.id}/taches">{openTasks ? `${openTasks} à traiter` : 'Voir'}</a></div>
      <p class="muted small" style="margin-top:10px">{view.tasks.length ? `${view.tasks.length - openTasks} sur ${view.tasks.length} terminée${view.tasks.length > 1 ? 's' : ''}.` : 'Aucune tâche.'}</p>
    </section>
    {#if writable}<button class="btn ghost danger" style="align-self:flex-start" onclick={() => (deleting = true)}><Icon name="trash" />Supprimer le projet</button>{/if}
  </div>
</div>

{#if deleting}
  <Confirm title="Supprimer « {p.name} » ?" eyebrow="Projet" confirmLabel="Supprimer définitivement" danger onconfirm={remove} onclose={() => (deleting = false)}>
    <p>Les emplacements, contenants, articles, exemplaires et tâches partent avec lui. Seule une sauvegarde du site permet de le retrouver.</p>
  </Confirm>
{/if}
