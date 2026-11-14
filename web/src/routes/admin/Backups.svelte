<script lang="ts">
  import { ask } from '../../lib/ask.svelte';
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
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
  async function setInterval_(h: number) { try { pInterval = (await api.put<{ interval_h: number }>('/site/points', { interval_h: h })).interval_h; toast('Fréquence des points de restauration enregistrée.'); } catch (e) { fail(e); } }
  async function restoreOrphan(o: (typeof orphans)[number]) { if (!(await ask({ title: `Restaurer le projet supprimé « ${o.project_name} » ?`, confirmLabel: 'Restaurer' }))) return; try { await api.post(`/points/${o.id}/restore`); toast('Projet restauré.'); await loadPoints(); } catch (e) { fail(e); } }
</script>

{#if error}<Unavailable message={error} />
{:else if st}
  {#if st.pending_restore}
    <div class="alert warn"><Icon name="alert" /><span style="flex:1">Une restauration de la sauvegarde du <b>{dateTime(st.pending_restore.created_at)}</b> s’appliquera au prochain démarrage du service.</span><button class="btn sm" onclick={cancelRestore}>Annuler</button></div>
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
      <div class="field"><label for="bd">Dossier des sauvegardes</label><input class="input mono" id="bd" bind:value={dir}><span class="help">Idéalement un disque réseau (NAS) monté sur le serveur, pour survivre à une panne de la machine.</span></div>
      <div class="row-actions" style="justify-content:flex-start"><button class="btn" onclick={saveDir} disabled={!!busy || dir === st.dir}>Enregistrer</button></div>
    </section>
  </div>

  <section class="card" style="padding:6px 18px 10px">
    <div class="card-head" style="padding-top:12px"><h2>Sauvegardes disponibles</h2>{#if st.backups.length}<span class="muted small">{st.backups.length} dans le dossier</span>{/if}</div>
    {#if st.backups.length}
    <div class="tbl-wrap">
      <table class="tbl">
        <thead><tr><th>Date</th><th>Type</th><th>Version</th><th class="num">Taille</th><th></th></tr></thead>
        <tbody>
          {#each st.backups as b (b.name)}
            <tr>
              <td><b>{dateTime(b.manifest.created_at)}</b>{#if b.manifest.label}<br><span class="muted small">{b.manifest.label}</span>{/if}</td>
              <td>{KIND[b.manifest.kind] ?? b.manifest.kind}</td>
              <td class="mono">{b.manifest.app_version} · schéma {b.manifest.schema}</td>
              <td class="num">{bytes(b.size)}</td>
              <td><div class="dl-actions">
                <button class="btn sm ghost" onclick={() => download(`/admin/backups/${encodeURIComponent(b.name)}/download`)} title="Télécharger"><Icon name="download" size="sm" /></button>
                <button class="btn sm" onclick={() => (restoring = b)}>Restaurer</button>
                <button class="btn sm ghost danger" onclick={() => (deleting = b)} title="Supprimer…" aria-label="Supprimer cette sauvegarde"><Icon name="trash" size="sm" /></button>
              </div></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {:else}<p class="muted small" style="margin:10px 0 8px">Aucune sauvegarde dans ce dossier : elles apparaîtront ici, avec leur date, leur taille et un bouton pour restaurer.</p>{/if}
  </section>
{:else}<span class="spin"></span>{/if}

<section class="card stack" style="margin-top:20px">
  <div class="card-head"><div><h2>Points de restauration des projets</h2><span class="muted small">Un point automatique est pris si le projet a changé ; les 48 derniers sont gardés par projet.</span></div></div>
  <div class="bk-freq">
    <span class="flabel" id="pfreq">Fréquence des points de restauration</span>
    <div class="seg" role="group" aria-labelledby="pfreq">{#each FREQ as [h, l]}<button class:on={pInterval === h} aria-pressed={pInterval === h} onclick={() => setInterval_(h)}>{l}</button>{/each}</div>
    <span class="muted small"><Icon name="check" size="sm" /> Enregistré : {FREQ.find(([h]) => h === pInterval)?.[1] ?? `toutes les ${pInterval} h`}</span>
  </div>
</section>

<section class="card stack" style="margin-top:20px">
  <div class="card-head"><div><h2>Projets supprimés</h2><span class="muted small">Un projet supprimé se récupère depuis son dernier point de restauration.</span></div></div>
  {#each orphans as o (o.id)}
    <div style="display:flex;gap:10px;align-items:center;border-top:1px solid var(--line);padding:6px 0"><span style="flex:1"><b>{o.project_name}</b> <span class="muted small">{o.label ?? 'point'} · {dateTime(o.created_at)}</span></span><button class="icon-btn sm" aria-label="Télécharger" onclick={() => download(`/points/${o.id}/download`)}><Icon name="download" size="sm" /></button><button class="btn sm" onclick={() => restoreOrphan(o)}>Restaurer</button></div>
  {:else}<p class="muted small" style="margin:0">Aucun projet supprimé n’a de point de restauration.</p>{/each}
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
