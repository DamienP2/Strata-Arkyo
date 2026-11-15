<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import { api, ApiError } from '../../lib/api';
  import { toast, fail } from '../../lib/toast.svelte';
  import type { Domain, Level, Permissions, Role } from '../../lib/types';
  import { visibleColor } from '../../lib/color';
  import HeadActions from '../../components/HeadActions.svelte';
  import { pageHead, plural } from '../../lib/pagehead.svelte';
  import { pl } from '../../lib/format';
  import Loading from '../../components/Loading.svelte';

  const DOMAINS: [Domain, string][] = [['projets', 'Projets'], ['conception', 'Conception'], ['catalogue', 'Catalogue'], ['membres', 'Membres et rôles'], ['parametres', 'Paramètres et site']];
  const LEVELS: [Level, string][] = [['aucun', 'Aucun'], ['lecture', 'Lecture'], ['ecriture', 'Écriture'], ['admin', 'Admin']];
  const DOMAIN_HELP: Partial<Record<Domain, string>> = { conception: 'Conception : racks, synoptique, réseau, plans et dessins des projets.', parametres: 'Paramètres et site : référentiels, impression, mises à jour, sauvegardes.' };
  const LV: Record<Level, string> = { aucun: '—', lecture: 'Lecture', ecriture: 'Écriture', admin: 'Admin' };

  let roles = $state<Role[] | null>(null);
  let editing = $state<{ id: string | null; name: string; color: string; permissions: Permissions } | null>(null);
  let error = $state<string | null>(null);
  let deleting = $state<Role | null>(null);

  async function load() { try { roles = (await api.get<{ roles: Role[] }>('/roles')).roles; } catch (e) { fail(e); roles ??= []; } }
  load();
  pageHead(() => ({ count: roles?.length ? plural(roles.length, 'rôle') : '' }));

  function edit(r?: Role) {
    error = null;
    editing = r ? { id: r.id, name: r.name, color: r.color, permissions: { ...r.permissions } }
      : { id: null, name: '', color: '#2943E4', permissions: { projets: 'lecture', conception: 'aucun', catalogue: 'lecture', membres: 'aucun', parametres: 'aucun' } };
  }
  async function save() {
    error = null;
    const b = { name: editing!.name.trim(), color: editing!.color, permissions: editing!.permissions };
    try {
      if (editing!.id) await api.patch(`/roles/${editing!.id}`, b); else await api.post('/roles', b);
      toast(editing!.id ? 'Rôle modifié.' : 'Rôle créé.'); editing = null; load();
    } catch (x) { error = x instanceof ApiError ? x.message : 'Enregistrement impossible.'; }
  }
  async function duplicate(r: Role) { try { await api.post(`/roles/${r.id}/duplicate`); toast('Rôle dupliqué.'); load(); } catch (e) { fail(e); } }
  async function remove() {
    try { const r = await api.del<{ moved_members: number }>(`/roles/${deleting!.id}`); toast(r?.moved_members ? `Rôle supprimé : ${r.moved_members} membre${pl(r.moved_members)} ${pl(r.moved_members, 'passent', 'passe')} Technicien.` : 'Rôle supprimé.'); deleting = null; load(); }
    catch (e) { fail(e); }
  }
</script>

<HeadActions><button class="btn primary" onclick={() => edit()}><Icon name="plus" />Nouveau rôle</button></HeadActions>

<section class="card tbl-card">
  {#if roles === null}<Loading label="Chargement des rôles" />
  {:else}
  <div class="tbl-wrap">
    <table class="tbl stack">
      <thead><tr><th>Rôle</th>{#each DOMAINS as [d, l]}<th title={DOMAIN_HELP[d]}>{#if DOMAIN_HELP[d]}<span class="th-help">{l}<span class="th-q" aria-hidden="true">?</span></span>{:else}{l}{/if}</th>{/each}<th class="num">Membres</th><th><span class="sr-only">Actions</span></th></tr></thead>
      <tbody>
        {#each roles as r (r.id)}
          <tr>
            <td class="first"><div class="r-name"><span class="role-dot" style="--c:{visibleColor(r.color)}"></span><b>{r.name}</b>{#if r.builtin}<span class="chip">intégré</span>{/if}</div></td>
            {#each DOMAINS as [d, l]}<td class:muted={r.permissions[d] === 'aucun'} data-l={l}>{LV[r.permissions[d]]}</td>{/each}
            <td class="num" data-l="Membres">{r.active_members}</td>
            <td class="acts"><div class="dl-actions">
              {#if !r.locked}<button class="icon-btn sm" title="Modifier {r.name}" aria-label="Modifier {r.name}" onclick={() => edit(r)}><Icon name="pen" size="sm" /></button>{:else}<span class="slot" title="Rôle verrouillé"></span>{/if}
              <button class="icon-btn sm" title="Dupliquer {r.name}" aria-label="Dupliquer {r.name}" onclick={() => duplicate(r)}><Icon name="copy" size="sm" /></button>
              {#if !r.builtin}<button class="icon-btn sm danger" title="Supprimer {r.name}" aria-label="Supprimer {r.name}" onclick={() => (deleting = r)}><Icon name="trash" size="sm" /></button>{:else}<span class="slot act-danger" title="Rôle intégré : non supprimable"></span>{/if}
            </div></td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  {/if}
</section>

{#if editing}
  <Dialog title={editing.id ? 'Modifier le rôle' : 'Nouveau rôle'} eyebrow="Équipe" onclose={() => (editing = null)} onsubmit={save} submitLabel={editing.id ? 'Enregistrer' : 'Créer'}>
    <div class="stack">
      <div class="r-head">
        <div class="field"><label for="r-nm">Nom</label><input class="input" id="r-nm" bind:value={editing.name} required maxlength="40"></div>
        <div class="field"><label for="r-co">Couleur</label><input class="input" id="r-co" type="color" bind:value={editing.color}></div>
      </div>
      <div class="perm-grid" role="group" aria-label="Droits par domaine">
        <span></span>{#each LEVELS as [, l]}<span class="h">{l}</span>{/each}
        {#each DOMAINS as [d, l]}
          <span title={DOMAIN_HELP[d]}>{l}</span>
          {#each LEVELS as [lv, lvl]}<input type="radio" name="p-{d}" value={lv} bind:group={editing.permissions[d]} aria-label="{l} : {lvl}">{/each}
        {/each}
      </div>
      <p class="muted small">Vous ne pouvez pas accorder un niveau supérieur au vôtre.</p>
      {#if error}<div class="alert err" role="alert">{error}</div>{/if}
    </div>
  </Dialog>
{/if}
{#if deleting}
  <Confirm title="Supprimer le rôle {deleting.name} ?" eyebrow="Équipe" confirmLabel="Supprimer" danger onconfirm={remove} onclose={() => (deleting = null)}>
    <p>{deleting.active_members ? `${deleting.active_members > 1 ? `Ses ${deleting.active_members} membres passeront` : 'Son membre passera'} au rôle Technicien.` : 'Aucun membre n’a ce rôle.'}</p>
  </Confirm>
{/if}

<style>
  .th-help { display: inline-flex; align-items: center; gap: 5px; cursor: help; }
  .th-q { display: inline-grid; place-items: center; width: 16px; height: 16px; border-radius: 50%; font-size: 11px; font-weight: 700; color: var(--fg-3); box-shadow: inset 0 0 0 1px var(--line-2); }
  .r-name { display: flex; gap: 10px; align-items: center; }
  /* Nom et couleur sur une ligne : le nuancier garde sa largeur fixe. */
  .r-head { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 14px 16px; }
</style>
