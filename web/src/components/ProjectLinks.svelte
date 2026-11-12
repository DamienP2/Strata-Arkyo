<script lang="ts">
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
  let { pid, cable = null }: { pid: string; cable?: string | null } = $props();
  const canDesign = $derived(can('conception', 'ecriture'));
  let d = $state<Data | null>(null);
  let proj = $state(''), port = $state('');

  async function load() { try { d = await api.get<Data>(`/projects/${pid}/links${cable && canDesign ? '?candidates=true' : ''}`); } catch (e) { fail(e); } }
  $effect(() => { pid; cable; load(); });
  $effect(() => join(`project:${pid}`, (c) => { if (c.kind === 'link' || c.kind === 'resync' || c.kind === 'cable') load(); }));
  const mine = $derived(cable ? d?.links.find((l) => l.local?.cable_id === cable) ?? null : null);
  const ports = $derived(d?.candidates.find((c) => c.project_id === proj)?.ports ?? []);
  async function link() {
    try { const r = await api.post<{ warnings: string[] }>(`/projects/${pid}/links`, { cable_id: cable, remote_project: proj, remote_cable: port }); toast(r.warnings.length ? `Lien créé. ${r.warnings.join(' ')}` : 'Lien créé.', r.warnings.length ? 'warn' : 'ok'); proj = ''; port = ''; await load(); } catch (e) { fail(e); }
  }
  async function unlink(l: L) { if (!confirm('Défaire ce lien entre projets ?')) return; try { await api.post(`/projects/${pid}/links/delete`, { id: l.id }); await load(); } catch (e) { fail(e); } }
</script>

{#snippet row(l: L)}
  <div class="pl">
    <div class="pl-t">
      {#if !cable}<span class="small"><b class="mono">{l.local?.number ?? '—'}</b> {l.local?.label ?? 'Extrémité disparue'}</span>{/if}
      <span class="small">↔ <a class="ilink" href="/projets/{l.remote.project_id}/synoptique">{l.remote.project_name ?? 'Projet'}</a> › <b class="mono">{l.remote.port?.number ?? ''}</b> {l.remote.port?.label ?? 'Extrémité disparue'}</span>
      {#each l.warnings as w}<span class="warn small"><Icon name="alert" size="sm" />{w}</span>{/each}
    </div>
    {#if canDesign}<button class="icon-btn sm" aria-label="Défaire le lien" onclick={() => unlink(l)}><Icon name="x" size="sm" /></button>{/if}
  </div>
{/snippet}

{#if d}
  {#if cable}
    <div class="box">
      <span class="flabel">Lien vers un autre projet</span>
      {#if mine}{@render row(mine)}
      {:else if canDesign}
        {#if !d.candidates.length}<p class="muted small" style="margin:0">Aucun port externe libre dans les autres projets.</p>
        {:else}
          <select class="input" bind:value={proj} aria-label="Projet"><option value="">Projet…</option>{#each d.candidates as c}<option value={c.project_id}>{c.project_name}</option>{/each}</select>
          {#if proj}<select class="input" bind:value={port} aria-label="Port externe"><option value="">Port externe…</option>{#each ports as p}<option value={p.cable_id}>{p.number ? `${p.number} · ` : ''}{p.label}</option>{/each}</select>{/if}
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
  .warn { color: var(--warn, #B78400); display: inline-flex; gap: 4px; align-items: center; }
  .links { margin-top: 20px; }
</style>
