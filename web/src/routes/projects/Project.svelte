<script lang="ts">
  // Page d'un projet : en-tête, sections, et temps réel (salle du projet).
  import { untrack } from 'svelte';
  import Workspace from '../../components/Workspace.svelte';
  import type { SecGroup } from '../../components/SecNav.svelte';
  import Avatar from '../../components/Avatar.svelte';
  import Icon from '../../components/Icon.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { route, navigate } from '../../lib/router.svelte';
  import { join, live } from '../../lib/live.svelte';
  import { STATUS, STATUS_ORDER, REGIE, keyDate, daysBetween, period, urgency, type ProjectView, type Status } from '../../lib/projects';
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

  const groups = $derived<SecGroup[]>([{ label: 'Projet', items: [
    { key: '', label: 'Vue d’ensemble', href: `/projets/${id}`, icon: 'grid' },
    { key: 'inventaire', label: 'Inventaire', href: `/projets/${id}/inventaire`, icon: 'box' },
    { key: 'taches', label: 'Tâches', href: `/projets/${id}/taches`, icon: 'check', badge: view?.tasks.filter((t) => t.status === 'todo' || t.status === 'in_progress').length || undefined },
  ] }, { label: 'Conception', items: [
    { key: 'racks', label: 'Racks', href: `/projets/${id}/racks`, icon: 'server' },
    { key: 'mobilier', label: 'Mobilier', href: `/projets/${id}/mobilier`, icon: 'layers' },
    { key: 'planche', label: 'Planche d’élévations', href: `/projets/${id}/planche`, icon: 'sheet' },
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
    planche: 'Les élévations des baies et du mobilier mises en page sur des feuilles, pour l’impression ou l’export.',
    plans: 'Les éléments du projet placés sur le site, avec les liaisons tracées et leurs longueurs réelles.',
    dessins: 'Schémas de principe, implantations vidéo et audio, plans de feu avec adressage DMX.',
  };
  const section = $derived(groups.flatMap((g) => g.items.map((i) => ({ ...i, group: g.label }))).find((i) => i.key === sub));

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
  <div class="page"><h1 class="display">Projet introuvable</h1><p class="lead">Il a peut-être été supprimé. <a class="ilink" href="/projets">Revenir aux projets</a>.</p></div>
{:else if view && p}
  {#snippet statusSelect()}
    <label class="st-select sm" style="--c:{STATUS[p.status].color}"><i class="dot"></i>
      <select value={p.status} aria-label="Statut du projet" disabled={!can('projets', 'ecriture')} onchange={(e) => setStatus((e.target as HTMLSelectElement).value as Status)}>{#each STATUS_ORDER as s}<option value={s}>{STATUS[s].label}</option>{/each}</select>
    </label>
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
          <span>{[REGIE[p.regie].label, p.service_type].filter(Boolean).join(' · ')}</span>
        </div>
        {@render statusSelect()}
      {/if}
    {/snippet}
    {#snippet top()}
      <header class="ws-top">
        <div class="ws-top-main">
          <span class="ws-crumb">{section?.group ?? 'Projet'}</span>
          <h1>{section?.label ?? 'Section'}</h1>
          {#if DESC[sub]}<p class="ws-desc">{DESC[sub]}</p>{/if}
        </div>
        <div class="ws-top-meta">
          {#if days != null && days >= 0 && !['done', 'cancelled'].includes(p.status)}<span class="tally" style="--c:{urgency(days)}">J-{days}</span>{/if}
          <span class="meta"><span class="mk">Exploitation</span>{period(p)}</span>
          {#if p.venue}<span class="meta hide-m"><span class="mk">Lieu</span>{p.venue}</span>{/if}
          <span class="meta hide-m"><span class="mk">Client</span>{p.client ?? 'Projet interne'}</span>
          {#if presence.length}<span class="presence" title="Présents sur ce projet">{#each presence as m (m.id)}<Avatar name={m.name} color={m.color} size="sm" />{/each}</span><span class="mode live"><i></i><span>En direct</span></span>{/if}
        </div>
      </header>
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
    {:else}<p class="lead">Section introuvable.</p>{/if}
    </div>
  </Workspace>
{:else}<span class="spin"></span>{/if}

<style>
  /* En-tête : chaque information porte son étiquette (ce ne sont pas des liens). */
  .meta { display: inline-flex; align-items: baseline; gap: 6px; color: var(--fg-2); cursor: default; }
  .mk { font: 600 10px/1 var(--mono); letter-spacing: .08em; text-transform: uppercase; color: var(--fg-3); }
  .ws-desc { margin: 2px 0 0; max-width: 640px; font-size: 13px; line-height: 1.35; color: var(--fg-3); }
</style>
