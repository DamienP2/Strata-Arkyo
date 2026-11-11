<script lang="ts">
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
</script>

{#if error}<Unavailable message={error} />
{:else if st}
  {#if st.pending_restore}
    <div class="alert warn"><Icon name="alert" /><span style="flex:1">Une restauration de la sauvegarde du <b>{dateTime(st.pending_restore.created_at)}</b> s’appliquera au prochain démarrage du service.</span><button class="btn sm" onclick={cancelRestore}>Annuler</button></div>
  {/if}
  {#if st.last_error}<div class="alert err"><Icon name="alert" /><span>La dernière sauvegarde automatique a échoué : {st.last_error}</span></div>{/if}

  <div class="grid-2">
    <section class="card stack">
      <div class="card-head"><div><p class="eyebrow">Dernière réussie</p><h2>{relative(st.last_success)}</h2></div></div>
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
                <button class="btn sm ghost danger" onclick={() => (deleting = b)} title="Supprimer"><Icon name="trash" size="sm" /></button>
              </div></td>
            </tr>
          {:else}<tr><td colspan="5" class="empty">Aucune sauvegarde pour l’instant.</td></tr>{/each}
        </tbody>
      </table>
    </div>
  </section>
{:else}<span class="spin"></span>{/if}

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
