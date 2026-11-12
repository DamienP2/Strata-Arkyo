<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import Unavailable from './Unavailable.svelte';
  import { api } from '../../lib/api';
  import { toast, fail } from '../../lib/toast.svelte';
  import { bytes, dateTime, relative } from '../../lib/format';

  let st = $state<any>(null);
  let error = $state<string | null>(null);
  let busy = $state('');
  let confirmRollback = $state(false);
  let fileInput = $state<HTMLInputElement>();

  const PHASES: Record<string, string> = { idle: 'Au repos', checking: 'Recherche en cours', downloading: 'Téléchargement', testing: 'Essai du paquet', scheduled: 'Installation planifiée', restarting: 'Redémarrage en cours' };

  async function load() {
    try { st = await api.get<any>('/admin/update'); error = null; } catch (e: any) { error = e.message; }
  }
  load();
  // Pendant un téléchargement ou avant un redémarrage, l'état change sans action de la personne.
  $effect(() => {
    if (!st || st.status.phase === 'idle') return;
    const t = setInterval(load, 3000);
    return () => clearInterval(t);
  });

  async function act(name: string, f: () => Promise<unknown>, ok: string) {
    busy = name;
    try { await f(); toast(ok); } catch (e) { fail(e); } finally { busy = ''; await load(); }
  }
  const check = () => act('check', () => api.post('/admin/update/check'), 'Recherche terminée.');
  const install = (when: 'now' | 'night') => act(when, () => api.post('/admin/update/install', { when }), when === 'now' ? 'Installation lancée : le site redémarrera après le compte à rebours.' : 'Installation planifiée pour cette nuit.');
  const cancel = () => act('cancel', () => api.del('/admin/update/schedule'), 'Installation annulée.');
  const setting = (b: object) => act('settings', () => api.patch('/admin/settings', b), 'Réglage enregistré.');
  async function upload() {
    const f = fileInput?.files?.[0];
    if (!f) return;
    await act('upload', () => api.upload('/admin/update/upload?when=now', f), 'Paquet vérifié : installation lancée.');
    if (fileInput) fileInput.value = "";
  }
  async function rollback() {
    await act('rollback', () => api.post('/admin/update/rollback'), 'Retour à la version précédente : le site redémarre.');
    confirmRollback = false;
  }
  const previous = $derived(st?.history?.previous as string | null);
  const available = $derived(st?.status?.available);
  const newer = $derived(available && available.version !== st?.current);
</script>

{#if error}<Unavailable message={error} />
{:else if st}
  <div class="grid-2">
    <section class="card stack">
      <div class="card-head"><div><p class="eyebrow">Version installée</p><h2 class="mono" style="font-size:34px">{st.current}</h2></div><span class="chip {newer ? 'info' : 'ok'}">{newer ? 'Mise à jour disponible' : 'À jour'}</span></div>
      <dl class="kv">
        <dt>État</dt><dd>{PHASES[st.status.phase] ?? st.status.phase}</dd>
        <dt>Dernière recherche</dt><dd>{relative(st.last_check)}</dd>
        <dt>Source</dt><dd>{st.source ? (st.source.startsWith('github:') ? `GitHub · ${st.source.slice(7)}` : st.source) : 'Désactivée (fichier seulement)'}</dd>
        <dt>Installation gérée</dt><dd>{st.managed ? 'Oui' : 'Non (hors paquet Debian)'}</dd>
      </dl>
      {#if st.status.last_check_error}<div class="alert warn"><Icon name="alert" /><span>{st.status.last_check_error}</span></div>{/if}
      {#if st.status.last_error}<div class="alert err"><Icon name="alert" /><span>{st.status.last_error}</span></div>{/if}
      <div class="row-actions" style="justify-content:flex-start">
        <button class="btn" onclick={check} disabled={!!busy}>{#if busy === 'check'}<span class="spin"></span>{:else}<Icon name="refresh" />{/if}Rechercher</button>
        <button class="btn" onclick={() => fileInput?.click()} disabled={!!busy || !st.managed}>{#if busy === 'upload'}<span class="spin"></span>{:else}<Icon name="upload" />{/if}Installer un paquet hors ligne</button>
        <input type="file" accept=".saupd" bind:this={fileInput} onchange={upload} hidden>
        {#if previous}<button class="btn ghost" onclick={() => (confirmRollback = true)} disabled={!!busy}><Icon name="undo" />Revenir à {previous}</button>{/if}
      </div>
    </section>

    <section class="card stack">
      {#if st.status.scheduled}
        <div class="card-head"><div><p class="eyebrow">Installation planifiée</p><h2 class="mono" style="font-size:34px">{st.status.scheduled.version}</h2></div></div>
        <p>Le site redémarrera {dateTime(st.status.scheduled.at)}. Les personnes connectées sont prévenues avant la bascule.</p>
        <div class="row-actions" style="justify-content:flex-start"><button class="btn" onclick={cancel} disabled={!!busy}>Annuler l’installation</button></div>
      {:else if newer}
        <div class="card-head"><div><p class="eyebrow">Disponible · canal {available.channel}</p><h2 class="mono" style="font-size:34px">{available.version}</h2></div><span class="muted small">{bytes(available.size)}</span></div>
        {#if available.notes?.length}<ul class="notes">{#each available.notes as n}<li>{n}</li>{/each}</ul>{/if}
        <p class="muted small">Une sauvegarde complète est prise avant la bascule. Si la nouvelle version ne démarre pas, le site revient seul à la version actuelle.</p>
        <div class="row-actions" style="justify-content:flex-start">
          <button class="btn primary" onclick={() => install('now')} disabled={!!busy || !st.managed}>{#if busy === 'now'}<span class="spin"></span>{:else}<Icon name="download" />{/if}Installer maintenant</button>
          <button class="btn" onclick={() => install('night')} disabled={!!busy || !st.managed}><Icon name="clock" />Cette nuit</button>
        </div>
      {:else}
        <div class="card-head"><h2>Aucune mise à jour en attente</h2></div>
        <p class="muted">Le moteur cherche les nouvelles versions {st.auto_check ? 'chaque jour' : 'seulement à la demande'} sur le canal {st.channel === 'beta' ? 'Bêta' : 'Stable'}.</p>
      {/if}
      <div class="field"><span class="flabel">Canal</span>
        <div class="seg">{#each [['stable', 'Stable'], ['beta', 'Bêta']] as [k, l]}<button class:on={st.channel === k} onclick={() => setting({ update_channel: k })} disabled={!!busy}>{l}</button>{/each}</div>
      </div>
      <label class="layer" style="gap:10px;display:flex;align-items:center"><input type="checkbox" checked={st.auto_check} onchange={(e) => setting({ update_auto_check: (e.target as HTMLInputElement).checked })}>Rechercher automatiquement chaque jour</label>
    </section>
  </div>

  {#if st.installed?.length}
    <section class="card">
      <div class="card-head"><h2>Versions présentes sur la machine</h2></div>
      <div class="chips" style="display:flex;gap:8px;flex-wrap:wrap;padding-top:10px">{#each st.installed as v}<span class="chip {v.active ? 'ok' : ''} mono">{v.version}{v.active ? ' · active' : ''}</span>{/each}</div>
    </section>
  {/if}
{:else}<span class="spin"></span>{/if}

{#if confirmRollback}
  <Confirm title="Revenir à la version {previous} ?" eyebrow="Retour arrière" confirmLabel="Revenir et redémarrer" danger onconfirm={rollback} onclose={() => (confirmRollback = false)}>
    <p>Le site redémarre sur la version {previous}. Les données saisies depuis restent en place : les évolutions de la base sont compatibles avec l’ancienne version.</p>
  </Confirm>
{/if}
