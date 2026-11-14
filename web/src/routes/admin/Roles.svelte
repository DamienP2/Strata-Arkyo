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

  const DOMAINS: [Domain, string][] = [['projets', 'Projets'], ['conception', 'Conception'], ['catalogue', 'Catalogue'], ['membres', 'Membres et rôles'], ['parametres', 'Paramètres et site']];
  const LEVELS: [Level, string][] = [['aucun', 'Aucun'], ['lecture', 'Lecture'], ['ecriture', 'Écriture'], ['admin', 'Admin']];
  const DOMAIN_HELP: Partial<Record<Domain, string>> = { conception: 'Conception : baies, synoptique, réseau, plans et dessins des projets.', parametres: 'Paramètres et site : référentiels, impression, mises à jour, sauvegardes.' };
  const LV: Record<Level, string> = { aucun: '—', lecture: 'Lecture', ecriture: 'Écriture', admin: 'Admin' };

  let roles = $state<Role[]>([]);
  let editing = $state<{ id: string | null; name: string; color: string; permissions: Permissions } | null>(null);
  let error = $state<string | null>(null);
  let deleting = $state<Role | null>(null);

  async function load() { try { roles = (await api.get<{ roles: Role[] }>('/roles')).roles; } catch (e) { fail(e); } }
  load();
  pageHead(() => ({ count: roles.length ? plural(roles.length, 'rôle') : '' }));

  function edit(r?: Role) {
    error = null;
    editing = r ? { id: r.id, name: r.name, color: r.color, permissions: { ...r.permissions } }
      : { id: null, name: '', color: '#2943E4', permissions: { projets: 'lecture', conception: 'aucun', catalogue: 'lecture', membres: 'aucun', parametres: 'aucun' } };
  }
  async function save(e: SubmitEvent) {
    e.preventDefault();
    const b = { name: editing!.name.trim(), color: editing!.color, permissions: editing!.permissions };
    try {
      if (editing!.id) await api.patch(`/roles/${editing!.id}`, b); else await api.post('/roles', b);
      toast(editing!.id ? 'Rôle modifié.' : 'Rôle créé.'); editing = null; load();
    } catch (x) { error = x instanceof ApiError ? x.message : 'Enregistrement impossible.'; }
  }
  async function duplicate(r: Role) { try { await api.post(`/roles/${r.id}/duplicate`); toast('Rôle dupliqué.'); load(); } catch (e) { fail(e); } }
  async function remove() {
    try { const r = await api.del<{ moved_members: number }>(`/roles/${deleting!.id}`); toast(r?.moved_members ? `Rôle supprimé : ${r.moved_members} membre(s) passent Technicien.` : 'Rôle supprimé.'); deleting = null; load(); }
    catch (e) { fail(e); }
  }
</script>

<HeadActions><button class="btn primary" onclick={() => edit()}><Icon name="plus" />Nouveau rôle</button></HeadActions>

<section class="card" style="padding:6px 18px 10px">
  <div class="tbl-wrap">
    <table class="tbl">
      <thead><tr><th>Rôle</th>{#each DOMAINS as [d, l]}<th title={DOMAIN_HELP[d]}>{#if DOMAIN_HELP[d]}<span class="th-help">{l}<span class="th-q" aria-hidden="true">?</span></span>{:else}{l}{/if}</th>{/each}<th class="num">Membres</th><th><span class="sr-only">Actions</span></th></tr></thead>
      <tbody>
        {#each roles as r (r.id)}
          <tr>
            <td><div style="display:flex;gap:10px;align-items:center"><span class="role-dot" style="--c:{visibleColor(r.color)}"></span><b>{r.name}</b>{#if r.builtin}<span class="chip">intégré</span>{/if}</div></td>
            {#each DOMAINS as [d]}<td class:muted={r.permissions[d] === 'aucun'}>{LV[r.permissions[d]]}</td>{/each}
            <td class="num">{r.active_members}</td>
            <td class="num"><div class="dl-actions row-acts">
              {#if !r.locked}<button class="btn sm ghost" title="Modifier" aria-label="Modifier {r.name}" onclick={() => edit(r)}><Icon name="pen" size="sm" /></button>{:else}<span class="slot" title="Rôle verrouillé"></span>{/if}
              <button class="btn sm ghost" title="Dupliquer" aria-label="Dupliquer {r.name}" onclick={() => duplicate(r)}><Icon name="copy" size="sm" /></button>
              {#if !r.builtin}<button class="btn sm ghost danger" title="Supprimer…" aria-label="Supprimer {r.name}" onclick={() => (deleting = r)}><Icon name="trash" size="sm" /></button>{:else}<span class="slot act-danger" title="Rôle intégré : non supprimable"></span>{/if}
            </div></td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</section>

{#if editing}
  <Dialog title={editing.id ? 'Modifier le rôle' : 'Nouveau rôle'} eyebrow="Équipe" onclose={() => (editing = null)}>
    <form id="rolef" class="stack" onsubmit={save}>
      <div style="display:grid;grid-template-columns:1fr auto;gap:12px">
        <div class="field"><label for="r-nm">Nom</label><input class="input" id="r-nm" bind:value={editing.name} required maxlength="40"></div>
        <div class="field"><label for="r-co">Couleur</label><input class="input" id="r-co" type="color" bind:value={editing.color} style="width:64px;padding:4px"></div>
      </div>
      <div class="perm-grid" role="group" aria-label="Droits par domaine">
        <span></span>{#each LEVELS as [, l]}<span class="h">{l}</span>{/each}
        {#each DOMAINS as [d, l]}
          <span title={DOMAIN_HELP[d]}>{l}</span>
          {#each LEVELS as [lv]}<input type="radio" name="p-{d}" value={lv} bind:group={editing.permissions[d]} aria-label="{l} : {lv}">{/each}
        {/each}
      </div>
      <p class="muted small">Vous ne pouvez pas accorder un niveau supérieur au vôtre.</p>
      {#if error}<div class="alert err" role="alert">{error}</div>{/if}
    </form>
    {#snippet footer()}<button class="btn ghost" onclick={() => (editing = null)}>Annuler</button><button class="btn primary" type="submit" form="rolef">Enregistrer</button>{/snippet}
  </Dialog>
{/if}
{#if deleting}
  <Confirm title="Supprimer le rôle {deleting.name} ?" eyebrow="Équipe" confirmLabel="Supprimer" danger onconfirm={remove} onclose={() => (deleting = null)}>
    <p>{deleting.active_members ? `Ses ${deleting.active_members} membre(s) passeront au rôle Technicien.` : 'Aucun membre n’a ce rôle.'}</p>
  </Confirm>
{/if}

<style>
  .th-help { display: inline-flex; align-items: center; gap: 5px; cursor: help; }
  .th-q { display: inline-grid; place-items: center; width: 15px; height: 15px; border-radius: 50%; font-size: 10.5px; font-weight: 700; color: var(--fg-3); box-shadow: inset 0 0 0 1px var(--line-2); }
</style>
