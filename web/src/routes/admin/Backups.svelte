<script lang="ts">
  import { ask } from '../../lib/ask.svelte';
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import SortTh, { type Sort } from '../../components/SortTh.svelte';
  import Seg from '../../components/Seg.svelte';
  import Loading from '../../components/Loading.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Unavailable from './Unavailable.svelte';
  import { api, download } from '../../lib/api';
  import { toast, fail } from '../../lib/toast.svelte';
  import { bytes, dateTime, relative } from '../../lib/format';

  let st = $state<any>(null);
  let error = $state<string | null>(null);
  let busy = $state('');
  let label = $state('');
  let dir = $state('');
  let restoring = $state<any>(null);
  let deleting = $state<any>(null);

  const KIND: Record<string, string> = { auto: 'Automatique', manual: 'Manuelle', 'pre-update': 'Avant mise à jour' };
  // Tri par date (plus récente d'abord) ou par taille.
  let sort = $state<Sort>({ key: 'date', dir: -1 });
  const backups = $derived(st ? [...st.backups].sort((a: any, b: any) => sort.dir * (sort.key === 'size' ? a.size - b.size : String(a.manifest.created_at).localeCompare(String(b.manifest.created_at)))) : []);

  async function load() {
    try { st = await api.get<any>('/admin/backups'); dir = st.dir; error = null; } catch (e: any) { error = e.message; }
  }
  load();

  async function create() {
    busy = 'create';
    try { await api.post('/admin/backups', { label: label.trim() || null }); label = ''; toast('Sauvegarde terminée et vérifiée.'); }
    catch (e) { fail(e); } finally { busy = ''; await load(); }
  }
  async function saveDir() {
    busy = 'dir';
    try { await api.patch('/admin/settings', { backup_dir: dir.trim() }); toast('Dossier de sauvegarde enregistré.'); }
    catch (e) { fail(e); } finally { busy = ''; await load(); }
  }
  async function restore(restart: boolean) {
    try {
      await api.post(`/admin/backups/${encodeURIComponent(restoring.name)}/restore`, { restart });
      toast(restart ? 'Restauration préparée : le site redémarre.' : 'Restauration préparée : elle s’appliquera au prochain redémarrage.');
      restoring = null;
    } catch (e) { fail(e); } finally { await load(); }
  }
  async function remove() {
    try { await api.del(`/admin/backups/${encodeURIComponent(deleting.name)}`); toast('Sauvegarde supprimée.'); deleting = null; }
    catch (e) { fail(e); } finally { await load(); }
  }
  async function cancelRestore() {
    try { await api.del('/admin/restore'); toast('Restauration annulée.'); } catch (e) { fail(e); } finally { await load(); }
  }

  // Points de restauration de projet : fréquence et projets supprimés (section 13.2).
  let pInterval = $state(1);
  let orphans = $state<{ id: string; project_name: string; label: string | null; created_at: string }[]>([]);
  async function loadPoints() { try { pInterval = (await api.get<{ interval_h: number }>('/site/points')).interval_h; orphans = (await api.get<{ points: typeof orphans }>('/points/orphans')).points; } catch { /* droits insuffisants */ } }
  loadPoints();
  const FREQ: [number, string][] = [[1, 'chaque heure'], [6, 'toutes les 6 h'], [24, 'chaque jour']];
  async function setInterval_(h: number) { try { pInterval = (await api.put<{ interval_h: number }>('/site/points', { interval_h: h })).interval_h; toast('Fréquence des points de restauration enregistrée.'); } catch (e) { fail(e); loadPoints(); } }
  async function restoreOrphan(o: (typeof orphans)[number]) { if (!(await ask({ title: `Restaurer le projet supprimé « ${o.project_name} » ?`, confirmLabel: 'Restaurer' }))) return; try { await api.post(`/points/${o.id}/restore`); toast('Projet restauré.'); await loadPoints(); } catch (e) { fail(e); } }
</script>

{#if error}<Unavailable message={error} />
{:else if st}
  {#if st.pending_restore}
    <div class="alert warn"><Icon name="alert" /><span style="flex:1">Une restauration de la sauvegarde du <b>{dateTime(st.pending_restore.created_at)}</b> s’appliquera au prochain démarrage du service.</span><button class="btn sm ghost" onclick={cancelRestore}>Annuler la restauration</button></div>
  {/if}
  {#if st.last_error}<div class="alert err"><Icon name="alert" /><span>La dernière sauvegarde automatique a échoué : {st.last_error}</span></div>{/if}
  {#if !st.last_success}
    <div class="alert warn bk-none"><Icon name="alert" /><span><b>Aucune sauvegarde n’a encore été faite.</b> En cas de panne du serveur, rien ne pourrait être récupéré. Faites-en une maintenant ; la suivante sera prise automatiquement le {dateTime(st.next_auto)}.</span><button class="btn" onclick={create} disabled={!!busy}>{#if busy === 'create'}<span class="spin"></span>{:else}<Icon name="download" />{/if}Sauvegarder</button></div>
  {/if}

  <div class="grid-2">
    <section class="card stack">
      <div class="card-head"><div><p class="eyebrow">Dernière sauvegarde réussie</p><h2>{st.last_success ? relative(st.last_success) : 'Aucune pour l’instant'}</h2></div></div>
      <p class="muted">Une sauvegarde automatique est prise chaque nuit à 03:00 ; les 7 dernières sont gardées. Prochaine : {dateTime(st.next_auto)}.</p>
      <div class="field"><label for="bl">Sauvegarder maintenant</label>
        <div style="display:flex;gap:8px"><input class="input" id="bl" bind:value={label} placeholder="Libellé facultatif, ex. « avant la finale »"><button class="btn primary" onclick={create} disabled={!!busy}>{#if busy === 'create'}<span class="spin"></span>{:else}<Icon name="download" />{/if}Sauvegarder</button></div>
      </div>
    </section>
    <section class="card stack">
      <div class="card-head"><h2>Emplacement</h2></div>
      <div class="field"><label for="bd">Dossier des sauvegardes</label><input class="input mono" id="bd" bind:value={dir}><button type="button" class="ux3-info" title="Idéalement un disque réseau (NAS) monté sur le serveur, pour survivre à une panne de la machine." aria-label="Idéalement un disque réseau (NAS) monté sur le serveur, pour survivre à une panne de la machine."><Icon name="info" size="sm" /></button></div>
      <div class="row-actions" style="justify-content:flex-start"><button class="btn" onclick={saveDir} disabled={!!busy || dir === st.dir}>Enregistrer</button></div>
    </section>
  </div>

  <section class="card tbl-card">
    <div class="card-head" style="padding-top:12px"><h2>Sauvegardes disponibles</h2>{#if st.backups.length}<span class="muted small">{st.backups.length} dans le dossier</span>{/if}</div>
    {#if st.backups.length}
    <div class="tbl-wrap">
      <table class="tbl stack">
        <thead><tr><SortTh label="Date" key="date" bind:sort /><th>Type</th><th>Version</th><SortTh label="Taille" key="size" bind:sort cls="num" /><th><span class="sr-only">Actions</span></th></tr></thead>
        <tbody>
          {#each backups as b (b.name)}
            <tr>
              <td class="first"><div><b>{dateTime(b.manifest.created_at)}</b>{#if b.manifest.label}<br><span class="muted small">{b.manifest.label}</span>{/if}</div></td>
              <td data-l="Type">{KIND[b.manifest.kind] ?? b.manifest.kind}</td>
              <td class="mono" data-l="Version">{b.manifest.app_version} · schéma {b.manifest.schema}</td>
              <td class="num" data-l="Taille">{bytes(b.size)}</td>
              <td class="acts"><div class="dl-actions">
                <button class="icon-btn sm" onclick={() => download(`/admin/backups/${encodeURIComponent(b.name)}/download`)} title="Télécharger la sauvegarde" aria-label="Télécharger la sauvegarde"><Icon name="download" size="sm" /></button>
                <button class="btn sm" onclick={() => (restoring = b)}>Restaurer</button>
                <button class="icon-btn sm danger" onclick={() => (deleting = b)} title="Supprimer la sauvegarde" aria-label="Supprimer la sauvegarde"><Icon name="trash" size="sm" /></button>
              </div></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {:else}<EmptyState card={false} icon="disk" message="Aucune sauvegarde dans ce dossier" hint="Elles apparaîtront ici, avec leur date, leur taille et un bouton pour restaurer." />{/if}
  </section>
{:else}<Loading label="Chargement des sauvegardes" />{/if}

<section class="card stack" style="margin-top:20px">
  <div class="card-head"><div><h2>Points de restauration des projets</h2><button type="button" class="ux3-info" title="Un point automatique est pris si le projet a changé ; les 48 derniers sont gardés par projet." aria-label="Un point automatique est pris si le projet a changé ; les 48 derniers sont gardés par projet."><Icon name="info" size="sm" /></button></div></div>
  <div class="bk-freq">
    <span class="flabel" aria-hidden="true">Fréquence des points de restauration</span>
    <Seg label="Fréquence des points de restauration" bind:value={pInterval} options={FREQ.map(([h, l]) => ({ value: h, label: l }))} onchange={setInterval_} />
    <span class="muted small"><Icon name="check" size="sm" /> Enregistré : {FREQ.find(([h]) => h === pInterval)?.[1] ?? `toutes les ${pInterval} h`}</span>
  </div>
</section>

<section class="card stack" style="margin-top:20px">
  <div class="card-head"><div><h2>Projets supprimés</h2><button type="button" class="ux3-info" title="Un projet supprimé se récupère depuis son dernier point de restauration." aria-label="Un projet supprimé se récupère depuis son dernier point de restauration."><Icon name="info" size="sm" /></button></div></div>
  {#each orphans as o (o.id)}
    <div style="display:flex;gap:10px;align-items:center;border-top:1px solid var(--line);padding:6px 0"><span style="flex:1"><b>{o.project_name}</b> <span class="muted small">{o.label ?? 'point'} · {dateTime(o.created_at)}</span></span><button class="icon-btn sm" title="Télécharger le point de restauration" aria-label="Télécharger le point de restauration" onclick={() => download(`/points/${o.id}/download`)}><Icon name="download" size="sm" /></button><button class="btn sm" onclick={() => restoreOrphan(o)}>Restaurer</button></div>
  {:else}<EmptyState card={false} icon="folder" message="Aucun projet supprimé à récupérer" hint="Un projet supprimé apparaît ici tant qu’il lui reste un point de restauration." />{/each}
</section>

{#if restoring}
  <Confirm title="Restaurer cette sauvegarde ?" eyebrow={dateTime(restoring.manifest.created_at)} confirmLabel="Restaurer et redémarrer" danger onconfirm={() => restore(true)} onclose={() => (restoring = null)}>
    <p>Le site reviendra exactement à son état du {dateTime(restoring.manifest.created_at)}. Tout ce qui a été saisi depuis sera perdu ; les données actuelles sont mises de côté sur le serveur, pas effacées.</p>
    <button class="btn ghost" style="align-self:flex-start" onclick={() => restore(false)}>Préparer seulement, appliquer au prochain redémarrage</button>
  </Confirm>
{/if}
{#if deleting}
  <Confirm title="Supprimer cette sauvegarde ?" eyebrow={dateTime(deleting.manifest.created_at)} confirmLabel="Supprimer" danger onconfirm={remove} onclose={() => (deleting = null)}>
    <p>Le fichier {deleting.name} sera effacé du dossier des sauvegardes.</p>
  </Confirm>
{/if}

<style>
  .bk-none { align-items: center; margin-bottom: 16px; }
  .bk-none > span { flex: 1; }
  .bk-freq { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; }
  .bk-freq .small { display: inline-flex; gap: 4px; align-items: center; }
</style>
