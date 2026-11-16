<script lang="ts">
  import { ask } from '../lib/ask.svelte';
  // Liens entre projets : un port externe relié au port externe d'un autre projet.
  // Avec `cable`, le lien de ce port (ou le formulaire pour le créer) ; sinon tous les liens.
  import Icon from './Icon.svelte';
  import { api } from '../lib/api';
  import { can } from '../lib/session.svelte';
  import { toast, fail } from '../lib/toast.svelte';
  import { join } from '../lib/live.svelte';

  interface P { cable_id: string; number: string | null; label: string; connector: string | null; signal: string | null }
  interface L { id: string; local: P | null; remote: { project_id: string; project_name: string | null; port: P | null }; warnings: string[] }
  interface Data { links: L[]; free: P[]; candidates: { project_id: string; project_name: string; ports: P[] }[] }
  let { pid, cable = null, chips = false }: {
    pid: string; cable?: string | null;
    /** Vue d'ensemble : une pastille par projet lié (nombre de liaisons), menant à ce projet. */
    chips?: boolean;
  } = $props();
  const canDesign = $derived(can('conception', 'ecriture'));
  let d = $state<Data | null>(null);
  let proj = $state(''), port = $state('');

  async function load() { try { d = await api.get<Data>(`/projects/${pid}/links${cable && canDesign ? '?candidates=true' : ''}`); } catch (e) { fail(e); } }
  $effect(() => { pid; cable; load(); });
  $effect(() => join(`project:${pid}`, (c) => { if (c.kind === 'link' || c.kind === 'resync' || c.kind === 'cable') load(); }));
  const mine = $derived(cable ? d?.links.find((l) => l.local?.cable_id === cable) ?? null : null);
  // Pastilles : les liens regroupés par projet distant.
  const byProject = $derived([...(d?.links ?? []).reduce((m, l) => {
    const k = l.remote.project_id, cur = m.get(k);
    m.set(k, { id: k, name: l.remote.project_name ?? 'Projet supprimé', n: (cur?.n ?? 0) + 1, warn: (cur?.warn ?? false) || l.warnings.length > 0 });
    return m;
  }, new Map<string, { id: string; name: string; n: number; warn: boolean }>()).values()]);
  const ports = $derived(d?.candidates.find((c) => c.project_id === proj)?.ports ?? []);
  async function link() {
    try { const r = await api.post<{ warnings: string[] }>(`/projects/${pid}/links`, { cable_id: cable, remote_project: proj, remote_cable: port }); toast(r.warnings.length ? `Lien créé. ${r.warnings.join(' ')}` : 'Lien créé.', r.warnings.length ? 'warn' : 'ok'); proj = ''; port = ''; await load(); } catch (e) { fail(e); }
  }
  async function unlink(l: L) {
    const other = l.remote.project_name ?? 'l’autre projet';
    if (!(await ask({ title: 'Défaire ce lien entre projets ?', text: `Le port externe ne sera plus relié à ${other}.`, confirmLabel: 'Défaire le lien', danger: true, run: () => api.post(`/projects/${pid}/links/delete`, { id: l.id }) }))) return;
    toast(`Lien avec ${other} défait.`); await load();
  }
</script>

{#snippet row(l: L)}
  <div class="pl">
    <div class="pl-t">
      {#if !cable}<span class="small">{#if l.local?.number}<b class="mono">{l.local.number}</b>{:else}<span class="muted">non numéroté</span>{/if} {l.local?.label ?? 'Extrémité disparue'}</span>{/if}
      <span class="small">↔ <a class="ilink" href="/projets/{l.remote.project_id}/synoptique">{l.remote.project_name ?? 'Projet'}</a> › <b class="mono">{l.remote.port?.number ?? ''}</b> {l.remote.port?.label ?? 'Extrémité disparue'}</span>
      {#each l.warnings as w}<span class="warn small"><Icon name="alert" size="sm" />{w}</span>{/each}
    </div>
    {#if canDesign}<button class="icon-btn sm danger" aria-label="Défaire le lien" title="Défaire le lien" onclick={() => unlink(l)}><Icon name="x" size="sm" /></button>{/if}
  </div>
{/snippet}

{#if d}
  {#if chips}
    {#if d.links.length || d.free.length}
      <section class="card pl-card">
        <div class="card-head"><h2>Projets liés</h2>{#if canDesign}<a class="btn ghost sm" href="/projets/{pid}/synoptique" title="Sélectionnez un port externe dans le synoptique pour le relier à un autre projet">Lier</a>{/if}</div>
        {#if byProject.length}
          <ul class="pl-chips">
            {#each byProject as r (r.id)}<li><a class="pl-chip" href="/projets/{r.id}" title="{r.n} liaison{r.n > 1 ? 's' : ''} avec {r.name}{r.warn ? ' (à vérifier)' : ''}">{#if r.warn}<Icon name="alert" size="sm" />{/if}<span class="pl-n">{r.name}</span><span class="mono">{r.n}</span></a></li>{/each}
          </ul>
        {/if}
        {#if d.free.length}<p class="muted small pl-free">{d.free.length} port{d.free.length > 1 ? 's' : ''} externe{d.free.length > 1 ? 's' : ''} non lié{d.free.length > 1 ? 's' : ''}.</p>{/if}
      </section>
    {/if}
  {:else if cable}
    <div class="box">
      <span class="flabel">Lien vers un autre projet</span>
      {#if mine}{@render row(mine)}
      {:else if canDesign}
        {#if !d.candidates.length}<p class="muted small" style="margin:0">Aucun port externe libre dans les autres projets.</p>
        {:else}
          <select class="input sm" bind:value={proj} aria-label="Projet"><option value="">Choisir un projet</option>{#each d.candidates as c}<option value={c.project_id}>{c.project_name}</option>{/each}</select>
          {#if proj}<select class="input sm" bind:value={port} aria-label="Port externe"><option value="">Choisir un port externe</option>{#each ports as p}<option value={p.cable_id}>{p.number ? `${p.number} · ` : ''}{p.label}</option>{/each}</select>{/if}
          <button class="btn sm" disabled={!port} onclick={link}><Icon name="syn" size="sm" />Lier</button>
        {/if}
      {:else}<p class="muted small" style="margin:0">Aucun lien.</p>{/if}
    </div>
  {:else if d.links.length || d.free.length}
    <section class="card links">
      <div class="card-head"><h2>Liens avec d’autres projets</h2><span class="muted small">{d.free.length} port{d.free.length > 1 ? 's' : ''} externe{d.free.length > 1 ? 's' : ''} non lié{d.free.length > 1 ? 's' : ''}</span></div>
      {#each d.links as l (l.id)}{@render row(l)}{:else}<p class="muted small">Aucun lien. Sélectionnez un port externe dans le synoptique pour le relier à un autre projet.</p>{/each}
    </section>
  {/if}
{/if}

<style>
  .box { display: grid; gap: 6px; padding: 10px; border-radius: 12px; background: var(--surface-2); }
  .pl { display: flex; gap: 8px; align-items: flex-start; padding: 6px 0; border-top: 1px solid var(--line); }
  .box .pl { border-top: 0; }
  .pl-t { flex: 1; display: grid; gap: 2px; min-width: 0; }
  .warn { color: var(--warn); display: inline-flex; gap: 4px; align-items: center; }
  .links { margin-top: 20px; }
  .pl-chips { display: flex; flex-wrap: wrap; gap: 6px; margin: 12px 0 0; padding: 0; list-style: none; }
  .pl-chips li { min-width: 0; max-width: 100%; }
  /* Pastille cliquable : gabarit de .chip, contour plus marqué au survol. */
  .pl-chip { display: inline-flex; align-items: center; gap: 6px; height: 28px; max-width: 100%; padding: 0 10px; border-radius: 999px; background: var(--surface-3); box-shadow: inset 0 0 0 1px var(--line); color: var(--fg-2); font: 600 var(--fs-xs) var(--font); text-decoration: none; white-space: nowrap; transition: box-shadow .12s, color .12s; }
  .pl-chip:hover { color: var(--fg); box-shadow: inset 0 0 0 1px var(--line-2); }
  .pl-n { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .pl-chip :global(.ic) { flex: none; color: var(--warn); }
  .pl-chip .mono { color: var(--fg-3); font-variant-numeric: tabular-nums; }
  .pl-free { margin: 10px 0 0; }
</style>
