<script lang="ts">
  // Page d'un projet : en-tête, sections, et temps réel (salle du projet).
  import { untrack } from 'svelte';
  import Workspace from '../../components/Workspace.svelte';
  import type { SecGroup } from '../../components/SecNav.svelte';
  import Avatar from '../../components/Avatar.svelte';
  import Icon from '../../components/Icon.svelte';
  import PageHead, { type Crumb } from '../../components/PageHead.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Loading from '../../components/Loading.svelte';
  import { pageSlot } from '../../lib/pagehead.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can, session } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { route, navigate } from '../../lib/router.svelte';
  import { join, live } from '../../lib/live.svelte';
  import { STATUS, STATUS_ORDER, PROJECT_TYPES, projectType, keyDate, daysBetween, period, urgency, type ProjectView, type Status } from '../../lib/projects';
  import Overview from './Overview.svelte';
  import Inventory from './Inventory.svelte';
  import Workshop from './Workshop.svelte';
  import RackSheet from './RackSheet.svelte';
  import Synoptic from './Synoptic.svelte';
  import CableTable from './CableTable.svelte';
  import Preparation from './Preparation.svelte';
  import Recette from './Recette.svelte';
  import Network from './Network.svelte';
  import Plans from './Plans.svelte';
  import Drawings from './Drawings.svelte';
  import Dossier from './Dossier.svelte';
  import Tasks from './Tasks.svelte';
  import History from './History.svelte';
  import { loadRefs } from '../../lib/catalog.svelte';
  import Racks from './Racks.svelte';
  import Furniture from './Furniture.svelte';

  let { id }: { id: string } = $props();
  let view = $state<ProjectView | null>(null);
  let gone = $state(false);
  const sub = $derived(route.path.split('/')[3] ?? '');

  async function load() {
    try { view = await api.get<ProjectView>(`/projects/${id}`); }
    catch (e) { if (e instanceof ApiError && e.status === 404) gone = true; else fail(e); }
  }
  // Une modification d'un autre membre recharge le projet (regroupé sur 300 ms).
  let t: ReturnType<typeof setTimeout>;
  $effect(() => {
    const pid = id;
    untrack(load);
    return join(`project:${pid}`, (c) => {
      if (c.kind === 'project' && c.action === 'delete') { toast('Ce projet vient d’être supprimé par un autre membre.', 'warn'); navigate('/projets'); return; }
      clearTimeout(t);
      t = setTimeout(load, 300);
    });
  });
  const presence = $derived(live.presence[`project:${id}`] ?? view?.presence ?? []);
  // Les autres membres présents (un même compte ouvert dans plusieurs onglets ne compte qu'une fois).
  const others = $derived([...new Map(presence.filter((m) => m.id !== session.me?.account.id).map((m) => [m.id, m])).values()]);

  const groups = $derived<SecGroup[]>([{ label: 'Projet', items: [
    { key: '', label: 'Vue d’ensemble', href: `/projets/${id}`, icon: 'grid' },
    { key: 'inventaire', label: 'Inventaire', href: `/projets/${id}/inventaire`, icon: 'box' },
    { key: 'taches', label: 'Tâches', href: `/projets/${id}/taches`, icon: 'check', badge: view?.tasks.filter((t) => t.status === 'todo' || t.status === 'in_progress').length || undefined },
  ] }, { label: 'Conception', items: [
    { key: 'racks', label: 'Racks', href: `/projets/${id}/racks`, icon: 'server' },
    { key: 'mobilier', label: 'Mobilier', href: `/projets/${id}/mobilier`, icon: 'layers' },
    { key: 'planche', label: 'Planches', href: `/projets/${id}/planche`, icon: 'sheet' },
    { key: 'synoptique', label: 'Synoptique', href: `/projets/${id}/synoptique`, icon: 'syn' },
    { key: 'cables', label: 'Liste des câbles', href: `/projets/${id}/cables`, icon: 'cable' },
    { key: 'reseau', label: 'Réseau', href: `/projets/${id}/reseau`, icon: 'network' },
    { key: 'plans', label: 'Plans', href: `/projets/${id}/plans`, icon: 'map' },
    { key: 'dessins', label: 'Dessins', href: `/projets/${id}/dessins`, icon: 'pen' },
  ] }, { label: 'Terrain', items: [
    { key: 'preparation', label: 'Préparation', href: `/projets/${id}/preparation`, icon: 'truck' },
    { key: 'atelier', label: 'Atelier', href: `/projets/${id}/atelier`, icon: 'tools' },
    { key: 'recette', label: 'Recette', href: `/projets/${id}/recette`, icon: 'ok' },
  ] }, { label: 'Documents', items: [
    { key: 'dossier', label: 'Dossier et exports', href: `/projets/${id}/dossier`, icon: 'file' },
    { key: 'historique', label: 'Historique', href: `/projets/${id}/historique`, icon: 'clock' },
  ] }]);
  // Une ligne sous le titre des pages dont le rôle n'est pas évident d'après leur nom.
  const DESC: Record<string, string> = {
    planche: 'Les élévations des racks et du mobilier, et les synoptiques, mis en page sur des feuilles pour l’impression ou l’export.',
    plans: 'Les éléments du projet placés sur le site, avec les liaisons tracées et leurs longueurs réelles.',
    dessins: 'Schémas de principe, implantations vidéo et audio, plans de feu avec adressage DMX.',
  };
  const section = $derived(groups.flatMap((g) => g.items.map((i) => ({ ...i, group: g.label }))).find((i) => i.key === sub));
  // Fil d'Ariane : Projets › nom du projet › groupe de la section (le titre porte la section).
  // Le groupe « Projet » (Vue d'ensemble, Inventaire, Tâches) répéterait le nom : il est omis.
  const crumbs = $derived<Crumb[]>(view ? [
    { label: 'Projets', href: '/projets' },
    sub ? { label: view.project.name, href: `/projets/${id}`, current: false } : { label: view.project.name },
    ...(sub && section?.group && section.group !== 'Projet' ? [{ label: section.group, current: false }] : []),
  ] : []);

  async function setStatus(s: Status) {
    if (!view) return;
    try { view.project = await api.patch(`/projects/${id}`, { ...view.project, status: s }); toast(`Statut : ${STATUS[s].label}.`); }
    catch (e) { fail(e); load(); }
  }
  const p = $derived(view?.project);
  const kd = $derived(p ? keyDate(p) : null);
  const days = $derived(view && kd ? daysBetween(view.today, kd) : null);
  // Référentiels (fonctions, types, connecteurs) : couleurs et catégories de toutes les pages du projet.
  loadRefs();
</script>

{#if gone}
  <div class="page">
    <PageHead crumbs={[{ label: 'Projets', href: '/projets' }, { label: 'Projet introuvable' }]} title="Projet introuvable" />
    <EmptyState icon="folder" message="Ce projet n’existe pas ou plus." hint="Il a peut-être été supprimé par un autre membre.">{#snippet action()}<a class="btn primary" href="/projets">Revenir aux projets</a>{/snippet}</EmptyState>
  </div>
{:else if view && p}
  {#snippet statusSelect()}
    <label class="st-select sm" style="--c:{STATUS[p.status].color}"><i class="dot"></i>
      <select value={p.status} aria-label="Statut du projet" disabled={!can('projets', 'ecriture')} onchange={(e) => setStatus((e.target as HTMLSelectElement).value as Status)}>{#each STATUS_ORDER as s}<option value={s}>{STATUS[s].label}</option>{/each}</select>
    </label>
  {/snippet}
  {#snippet meta()}
    <span class="hm">
      {#if days != null && days >= 0 && !['done', 'cancelled'].includes(p.status)}<span class="tally" style="--c:{urgency(days)}" title="Jours avant l’exploitation">J-{days}</span>{/if}
      <span class="hm-it"><span class="lbl-t">Exploitation</span>{period(p)}</span>
      {#if p.venue}<span class="hm-it hide-m"><span class="lbl-t">Lieu</span>{p.venue}</span>{/if}
      <span class="hm-it hide-m"><span class="lbl-t">Client</span>{p.client ?? 'Projet interne'}</span>
      {#if others.length}
        {@const label = `${others.length + 1} personnes connectées`}
        {@const help = `Aussi sur ce projet : ${others.map((m) => m.name).join(', ')}. Leurs modifications s’affichent sans recharger la page.`}
        <span class="hm-live" title={help}>
          <span class="presence" aria-hidden="true">{#each others as m (m.id)}<Avatar name={m.name} color={m.color} size="sm" />{/each}</span>
          <span class="chip">{label}<span class="sr-only"> : {help}</span></span>
        </span>
      {/if}
    </span>
  {/snippet}
  <Workspace {groups} active={sub} label="Sections du projet" store="projet" context={p.name}>
    {#snippet mobileExtra()}
      <span class="small muted">Statut du projet</span>
      {@render statusSelect()}
    {/snippet}
    {#snippet head(folded: boolean)}
      {#if folded}
        <a class="ws-proj-mini" href="/projets/{id}" title={p.name} style="--c:{STATUS[p.status].color}">{p.name.slice(0, 2).toUpperCase()}</a>
      {:else}
        <a class="ws-back" href="/projets"><Icon name="chevr" size="sm" />Projets</a>
        <div class="ws-proj">
          <b title={p.name}>{p.name}</b>
          <span>{PROJECT_TYPES[projectType(p.regie)].label}</span>
        </div>
        {@render statusSelect()}
      {/if}
    {/snippet}
    {#snippet top()}
      <PageHead ws portal {crumbs} title={section?.label ?? 'Section introuvable'} count={pageSlot.count} desc={DESC[sub] ?? ''} info={meta} />
    {/snippet}
    <div class="page">
    {#if sub === ''}<Overview bind:view reload={load} />
    {:else if sub === 'inventaire'}<Inventory bind:view reload={load} />
    {:else if sub === 'atelier'}<Workshop {view} />
    {:else if sub === 'planche'}<RackSheet {view} />
    {:else if sub === 'synoptique'}<Synoptic {view} />
    {:else if sub === 'cables'}<CableTable {view} />
    {:else if sub === 'preparation'}<Preparation {view} />
    {:else if sub === 'recette'}<Recette {view} />
    {:else if sub === 'reseau'}<Network {view} />
    {:else if sub === 'plans'}<Plans {view} />
    {:else if sub === 'dessins'}<Drawings {view} />
    {:else if sub === 'dossier'}<Dossier {view} />
    {:else if sub === 'taches'}<Tasks bind:view reload={load} />
    {:else if sub === 'historique'}<History projectId={id} />
    {:else if sub === 'racks'}<Racks {view} />
    {:else if sub === 'mobilier'}<Furniture {view} reload={load} />
    {:else}<EmptyState icon="search" message="Cette section n’existe pas." hint="Choisissez une section dans la barre latérale.">{#snippet action()}<a class="btn" href="/projets/{id}">Vue d’ensemble</a>{/snippet}</EmptyState>{/if}
    </div>
  </Workspace>
{:else}<Loading label="Chargement du projet" />{/if}

<style>
  /* Méta de l'en-tête : chaque information porte son libellé (.lbl-t, Inter 11,5 px) ; ce ne sont pas des liens. */
  .hm { display: flex; align-items: center; justify-content: flex-end; flex-wrap: wrap; gap: 6px 14px; min-height: 26px; }
  @media (max-width: 860px) { .hm { justify-content: flex-start; } }
  .hm-it { display: inline-flex; align-items: baseline; gap: 6px; color: var(--fg-2); cursor: default; }
  .hm-it :global(.lbl-t) { line-height: 1; }
  .hm-live { display: inline-flex; align-items: center; gap: 8px; cursor: default; }
  .hm-live .presence { padding: 0 0 0 6px; }
  .hm-live .presence :global(.av) { margin-left: -6px; }
</style>
