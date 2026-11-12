<script lang="ts">
  // Points de restauration d'un projet (section 13.2) et export portable.
  import Icon from './Icon.svelte';
  import { api, download } from '../lib/api';
  import { can } from '../lib/session.svelte';
  import { toast, fail } from '../lib/toast.svelte';
  import { relative } from '../lib/format';

  interface Pt { id: string; label: string | null; auto: boolean; created_at: string; size: number }
  let { pid, onrestored }: { pid: string; onrestored?: () => void } = $props();
  const writable = $derived(can('projets', 'ecriture'));
  let points = $state<Pt[]>([]);
  let interval = $state(1);
  let label = $state('');
  let busy = $state(false);
  async function load() { try { const r = await api.get<{ points: Pt[]; interval_h: number }>(`/projects/${pid}/points`); points = r.points; interval = r.interval_h; } catch (e) { fail(e); } }
  $effect(() => { pid; load(); });
  async function take() { busy = true; try { await api.post(`/projects/${pid}/points`, { label: label || null }); label = ''; toast('Point de restauration créé.'); await load(); } catch (e) { fail(e); } finally { busy = false; } }
  async function restore(p: Pt) {
    if (!confirm(`Remettre le projet dans l’état du ${new Date(p.created_at).toLocaleString('fr-FR')} ? L’état actuel est d’abord gardé dans un point.`)) return;
    busy = true; try { await api.post(`/points/${p.id}/restore`); toast('Projet restauré.'); await load(); onrestored?.(); } catch (e) { fail(e); } finally { busy = false; }
  }
  async function remove(p: Pt) { if (!confirm('Supprimer ce point de restauration ?')) return; try { await api.post(`/points/${p.id}/delete`); await load(); } catch (e) { fail(e); } }
  const ko = (n: number) => `${Math.max(1, Math.round(n / 1024))} Ko`;
</script>

<section class="card pts">
  <div class="card-head"><div><h2>Points de restauration</h2><span class="muted small">Automatiques toutes les {interval} h si le projet a changé (48 gardés) ; manuels gardés jusqu’à suppression.</span></div>
    <button class="btn sm ghost" onclick={() => download(`/projects/${pid}/export`)}><Icon name="download" size="sm" />Exporter le projet</button></div>
  {#if writable}<div class="new"><input class="input" bind:value={label} maxlength="120" placeholder="Libellé (facultatif) : avant tournage, livraison…" aria-label="Libellé du point"><button class="btn sm primary" disabled={busy} onclick={take}><Icon name="plus" size="sm" />Créer un point</button></div>{/if}
  {#each points as p (p.id)}
    <div class="pt"><span class="chip sm">{p.auto ? 'auto' : 'manuel'}</span><span class="t"><b>{p.label ?? (p.auto ? 'Point automatique' : 'Point manuel')}</b><span class="muted small">{new Date(p.created_at).toLocaleString('fr-FR')} · {relative(p.created_at)} · {ko(p.size)}</span></span>
      <button class="icon-btn sm" aria-label="Télécharger" onclick={() => download(`/points/${p.id}/download`)}><Icon name="download" size="sm" /></button>
      {#if writable}<button class="btn sm" disabled={busy} onclick={() => restore(p)}>Restaurer</button><button class="icon-btn sm" aria-label="Supprimer le point" onclick={() => remove(p)}><Icon name="trash" size="sm" /></button>{/if}
    </div>
  {:else}<p class="muted small">Aucun point pour l’instant.</p>{/each}
</section>

<style>
  .pts { display: grid; gap: 8px; margin-bottom: 20px; }
  .new { display: flex; gap: 8px; }
  .new .input { flex: 1; }
  .pt { display: flex; gap: 10px; align-items: center; padding: 6px 0; border-top: 1px solid var(--line); }
  .t { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  .chip.sm { font-size: 11px; padding: 1px 7px; border-radius: 99px; background: var(--surface-2); font-weight: 600; }
</style>
