<script lang="ts">
  // Page d'un projet : en-tête, sections, et temps réel (salle du projet).
  import { untrack } from 'svelte';
  import SecNav, { type SecGroup } from '../../components/SecNav.svelte';
  import Avatar from '../../components/Avatar.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { route, navigate } from '../../lib/router.svelte';
  import { join, live } from '../../lib/live.svelte';
  import { STATUS, STATUS_ORDER, REGIE, keyDate, daysBetween, fdate, urgency, type ProjectView, type Status } from '../../lib/projects';
  import Overview from './Overview.svelte';
  import Inventory from './Inventory.svelte';
  import Regies from './Regies.svelte';
  import Workshop from './Workshop.svelte';
  import RackSheet from './RackSheet.svelte';
  import Synoptic from './Synoptic.svelte';
  import CableTable from './CableTable.svelte';
  import Tasks from './Tasks.svelte';
  import History from './History.svelte';
  import Racks from './Racks.svelte';

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

  const groups = $derived<SecGroup[]>([{ label: 'Organiser', items: [
    { key: '', label: 'Vue d’ensemble', href: `/projets/${id}` },
    { key: 'inventaire', label: 'Inventaire', href: `/projets/${id}/inventaire` },
    { key: 'regies', label: 'Régies', href: `/projets/${id}/regies` },
    { key: 'taches', label: 'Tâches', href: `/projets/${id}/taches`, badge: view?.tasks.filter((t) => t.status === 'todo' || t.status === 'in_progress').length || undefined },
    { key: 'historique', label: 'Historique', href: `/projets/${id}/historique` },
  ] }, { label: 'Dessiner', lock: true, items: [
    { key: 'racks', label: 'Racks', href: `/projets/${id}/racks` },
    { key: 'planche', label: 'Planche d’élévations', href: `/projets/${id}/planche` },
    { key: 'synoptique', label: 'Synoptique', href: `/projets/${id}/synoptique` },
    { key: 'cables', label: 'Câbles', href: `/projets/${id}/cables` },
  ] }, { label: 'Sur le terrain', items: [
    { key: 'atelier', label: 'Atelier', href: `/projets/${id}/atelier` },
  ] }]);

  async function setStatus(s: Status) {
    if (!view) return;
    try { view.project = await api.patch(`/projects/${id}`, { ...view.project, status: s }); toast(`Statut : ${STATUS[s].label}.`); }
    catch (e) { fail(e); load(); }
  }
  const p = $derived(view?.project);
  const kd = $derived(p ? keyDate(p) : null);
  const days = $derived(view && kd ? daysBetween(view.today, kd) : null);
</script>

{#if gone}
  <div class="page"><h1 class="display">Projet introuvable</h1><p class="lead">Il a peut-être été supprimé. <a class="ilink" href="/projets">Revenir aux projets</a>.</p></div>
{:else if view && p}
  <div class="page">
    <header class="ptop">
      <div class="ptop-main">
        <nav class="crumbs" aria-label="Fil d’Ariane"><a class="crumb" href="/projets">Projets</a><span class="sep">›</span><span>{p.intervention === 'prestation' ? 'Prestation' : 'Intégration'} · {REGIE[p.regie].label}{p.service_type ? ` · ${p.service_type}` : ''}</span></nav>
        <h1 class="display">{p.name}</h1>
        <p class="pmeta">
          {#if days != null && days >= 0 && !['done', 'cancelled'].includes(p.status)}<span class="tally" style="--c:{urgency(days)}">J-{days}</span>{/if}
          <span>{p.intervention === 'prestation' ? `Tournage ${fdate(p.shoot_start)} → ${fdate(p.shoot_end)}` : `Mise en service ${fdate(p.commissioning)}`}</span>
          {#if p.venue}<span>{p.venue}</span>{/if}<span>{p.client ?? 'Projet interne'}</span>
        </p>
      </div>
      <div style="display:flex;flex-direction:column;gap:12px;align-items:flex-end">
        <label class="st-select" style="--c:{STATUS[p.status].color}"><i class="dot"></i>
          <select value={p.status} aria-label="Statut du projet" disabled={!can('projets', 'ecriture')} onchange={(e) => setStatus((e.target as HTMLSelectElement).value as Status)}>{#each STATUS_ORDER as s}<option value={s}>{STATUS[s].label}</option>{/each}</select>
        </label>
        {#if presence.length}<div class="presence" title="Présents sur ce projet">{#each presence as m (m.id)}<Avatar name={m.name} color={m.color} size="sm" />{/each}<span class="mode live" style="margin-left:8px"><i></i><span>En direct</span></span></div>{/if}
      </div>
    </header>
    <SecNav {groups} active={sub} label="Sections du projet" />
    {#if sub === ''}<Overview bind:view reload={load} />
    {:else if sub === 'inventaire'}<Inventory bind:view reload={load} />
    {:else if sub === 'regies'}<Regies {view} />
    {:else if sub === 'atelier'}<Workshop {view} />
    {:else if sub === 'planche'}<RackSheet {view} />
    {:else if sub === 'synoptique'}<Synoptic {view} />
    {:else if sub === 'cables'}<CableTable {view} />
    {:else if sub === 'taches'}<Tasks bind:view reload={load} />
    {:else if sub === 'historique'}<History projectId={id} />
    {:else if sub === 'racks'}<Racks {view} />
    {:else}<p class="lead">Section introuvable.</p>{/if}
  </div>
{:else}<span class="spin"></span>{/if}
