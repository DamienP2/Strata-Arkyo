<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Vu from '../../components/Vu.svelte';
  import ProjectLinks from '../../components/ProjectLinks.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import AnomalyGroups from '../../components/AnomalyGroups.svelte';
  import type { Anomaly } from '../../lib/anomalies';
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
    !p.service_type && 'le type de projet',
    !(p.planning_start && p.planning_end) && 'la période de préparation',
    !((p.shoot_start && p.shoot_end) || p.commissioning) && 'la période d’exploitation',
    !view.locations.length && 'un emplacement',
    !view.containers.length && 'un élément',
    !view.articles.length && 'du matériel',
  ].filter(Boolean) as string[]);
  interface Item { kind: string; group?: string; severity: string; subject?: string; place?: string | null; detail?: string; message: string; href: string }
  interface Todo { domains: { key: string; label: string; done: number; total: number; pct: number; href: string; note: string; hint?: string }[]; items: Item[] }
  let todo = $state<Todo | null>(null);
  const KIND_GROUP: Record<string, string> = { network: 'Réseau', dmx: 'DMX', sheet: 'Fiche', rack: 'Non posé en baie' };
  // Une ligne par objet, regroupée par type d'anomalie ; les points sans sujet gardent leur phrase.
  const anomalies = $derived<Anomaly<Item>[]>((todo?.items ?? []).map((it) => ({
    group: it.group ?? KIND_GROUP[it.kind] ?? it.kind, severity: it.severity,
    subject: it.subject ?? it.message, place: it.place, detail: it.subject ? (it.detail ?? '') : '', href: it.href, src: it,
  })));
  // « Tout voir » d'un groupe de câbles : la liste des câbles filtrée sur eux.
  const groupHref = (g: { items: Anomaly<Item>[] }) => {
    const ids = g.items.flatMap((a) => new URLSearchParams(a.href?.split('?')[1] ?? '').get('cable')?.split(',') ?? []);
    return ids.length > 1 ? `/projets/${view.project.id}/cables?cable=${[...new Set(ids)].join(',')}` : undefined;
  };
  // Relue à chaque changement du projet (la vue se recharge sur les événements des autres).
  $effect(() => { view; api.get<Todo>(`/projects/${view.project.id}/todo`).then((t) => (todo = t)).catch(() => {}); });
  const openTasks = $derived(view.tasks.filter((t) => t.status === 'todo' || t.status === 'in_progress').length);
</script>

<svelte:window onbeforeunload={(e) => { if (dirty) e.preventDefault(); }} />

{#if todo}
  <section class="card td">
    <div class="td-doms">
      {#each todo.domains as d (d.key)}
        <a class="dom" href={d.href} title={d.hint}><span class="flabel">{d.label}{#if d.hint}<span class="hint" aria-hidden="true">?</span>{/if}</span><b class="mono">{d.pct} %</b><span class="bar"><i style="width:{d.pct}%" class:ok={d.pct === 100}></i></span><span class="muted small">{d.note}</span></a>
      {/each}
    </div>
    <div class="td-list">
      <h3>Ce qu’il reste à faire {#if todo.items.length}<span class="muted">· {todo.items.length} point{todo.items.length > 1 ? 's' : ''}</span>{/if}</h3>
      {#if !todo.items.length}<p class="muted small">Rien en attente : aucune anomalie, tout est posé, la fiche est complète.</p>
      {:else}<AnomalyGroups list={anomalies} {groupHref} groupLink="Voir ces câbles" />{/if}
    </div>
  </section>
{/if}

<div class="grid-2" style="grid-template-columns:minmax(0,1.6fr) minmax(0,1fr);align-items:start">
  <section class="card">
    <div class="card-head"><h2>Fiche du projet</h2><span class="chip">{REGIE[p.regie].label}</span></div>
    <form class="form-grid" style="margin-top:16px" onsubmit={save}>
      <fieldset disabled={!writable} style="display:contents">
        <div class="field span"><label for="o-name">Nom</label><input class="input" id="o-name" bind:value={form.name} required maxlength="120"><FieldError message={errors.name} /></div>
        <div class="field"><label for="o-client">Client</label><input class="input" id="o-client" value={form.client ?? ''} oninput={txt('client')} placeholder="Vide : projet interne"></div>
        <div class="field"><label for="o-tp">Type de projet</label><input class="input" id="o-tp" value={form.service_type ?? ''} oninput={txt('service_type')} placeholder="Sport, concert, TV…"></div>
        <div class="field span"><label for="o-venue">Lieu</label><input class="input" id="o-venue" value={form.venue ?? ''} oninput={txt('venue')}></div>
        <div class="field"><label for="o-ps">Préparation : début</label><input class="input" type="date" id="o-ps" value={form.planning_start ?? ''} oninput={txt('planning_start')}><FieldError message={errors.planning_start} /></div>
        <div class="field"><label for="o-pe">Préparation : fin</label><input class="input" type="date" id="o-pe" value={form.planning_end ?? ''} oninput={txt('planning_end')}><FieldError message={errors.planning_end} /></div>
        <div class="field"><label for="o-ss">Exploitation : début</label><input class="input" type="date" id="o-ss" value={form.shoot_start ?? form.commissioning ?? ''} oninput={txt('shoot_start')}><FieldError message={errors.shoot_start} /></div>
        <div class="field"><label for="o-se">Exploitation : fin</label><input class="input" type="date" id="o-se" value={form.shoot_end ?? form.commissioning ?? ''} oninput={txt('shoot_end')}><FieldError message={errors.shoot_end} /></div>
        <div class="field span"><span class="flabel">Type de régie</span><div class="seg">{#each Object.entries(REGIE) as [k, r]}<button type="button" class:on={form.regie === k} onclick={() => (form.regie = k as Project['regie'])}>{r.label}</button>{/each}</div></div>
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
        <dt>Éléments</dt><dd>{view.containers.length}</dd>
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
    <ProjectLinks pid={p.id} />
    {#if writable}<button class="btn ghost danger" style="align-self:flex-start" onclick={() => (deleting = true)}><Icon name="trash" />Supprimer le projet</button>{/if}
  </div>
</div>

{#if deleting}
  <Confirm title="Supprimer « {p.name} » ?" eyebrow="Projet" confirmLabel="Supprimer définitivement" danger onconfirm={remove} onclose={() => (deleting = false)}>
    <p>Les emplacements, éléments, matériel, câbles et tâches partent avec lui. Il reste récupérable pendant un temps depuis Administration › Sauvegardes (projets supprimés), puis seulement depuis une sauvegarde du site.</p>
  </Confirm>
{/if}

<style>
  .td { display: grid; gap: 16px; margin-bottom: 20px; }
  .td-doms { display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 10px; }
  .dom { display: grid; gap: 4px; padding: 10px 12px; border-radius: 12px; border: 1px solid var(--line); color: inherit; text-decoration: none; }
  .dom:hover { border-color: var(--fg-3); }
  .dom b { font-size: 22px; }
  .dom .bar { height: 5px; border-radius: 99px; background: var(--surface-2); overflow: hidden; }
  .dom .bar i { display: block; height: 100%; background: var(--fg); }
  .dom .bar i.ok { background: var(--ok, #0E8550); }
  .td-list h3 { margin: 0 0 8px; font-size: 16px; }
  .hint { display: inline-grid; place-items: center; width: 15px; height: 15px; margin-left: 5px; border-radius: 50%; border: 1px solid var(--fg-3); color: var(--fg-3); font-size: 10px; vertical-align: 1px; cursor: help; }
</style>
