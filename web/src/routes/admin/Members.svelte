<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { copyText } from '../../lib/clipboard';
  import Icon from '../../components/Icon.svelte';
  import Avatar from '../../components/Avatar.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import SortTh, { type Sort } from '../../components/SortTh.svelte';
  import Loading from '../../components/Loading.svelte';
  import { api, ApiError } from '../../lib/api';
  import { session } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { relative } from '../../lib/format';
  import { route } from '../../lib/router.svelte';
  import type { Member, Role } from '../../lib/types';
  import { ask } from '../../lib/ask.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import HeadActions from '../../components/HeadActions.svelte';
  import { pageHead, plural } from '../../lib/pagehead.svelte';

  let members = $state<Member[] | null>(null);
  let roles = $state<Role[]>([]);
  let showRemoved = $state(false);
  let q = $state('');
  let creating = $state(false);
  let form = $state({ email: '', name: '', username: '', role_id: 'technician' });
  let formError = $state<string | null>(null);
  let secret = $state<{ who: string; password: string } | null>(null);
  let removing = $state<Member | null>(null);
  let resetting = $state<Member | null>(null);

  // Tri des colonnes Membre, Rôle et Ajouté, mémorisé dans le navigateur.
  const SORT_KEY = 'admin-members-sort';
  const SORT_KEYS = ['name', 'role', 'added'];
  function savedSort(): Sort {
    try { const s = JSON.parse(localStorage.getItem(SORT_KEY) ?? ''); if (SORT_KEYS.includes(s?.key) && (s.dir === 1 || s.dir === -1)) return s; } catch { /* stockage indisponible */ }
    return { key: 'name', dir: 1 };
  }
  let sort = $state<Sort>(savedSort());
  $effect(() => { const s = JSON.stringify(sort); try { localStorage.setItem(SORT_KEY, s); } catch { /* stockage indisponible */ } });

  const roleOf = (id: string) => roles.find((r) => r.id === id);
  const sortVal = (m: Member): string => sort.key === 'role' ? m.role_name : sort.key === 'added' ? m.created_at : m.name;
  const shown = $derived((members ?? [])
    .filter((m) => !q || `${m.name} ${m.email} ${m.username ?? ''}`.toLowerCase().includes(q.toLowerCase()))
    .sort((a, b) => sort.dir * sortVal(a).localeCompare(sortVal(b), 'fr', { numeric: true, sensitivity: 'base' })));

  async function load() {
    try {
      const [m, r] = await Promise.all([api.get<{ members: Member[] }>(`/members?removed=${showRemoved}`), api.get<{ roles: Role[] }>('/roles')]);
      members = m.members; roles = r.roles;
    } catch (e) { fail(e); members ??= []; }
  }
  $effect(() => { showRemoved; load(); });

  // Lien ciblé (palette de recherche) : /admin/membres?membre=<id> fait défiler jusqu'au membre et le met en surbrillance.
  const target = $derived(route.query.get('membre'));
  let handled = '';
  let hl = $state<string | null>(null);
  $effect(() => {
    const id = target, list = members;
    if (!id || id === handled || !list) return;
    untrack(async () => {
      if (!list.some((m) => m.id === id)) {
        // Membre retiré du site : on affiche les retirés puis on réessaie au chargement suivant.
        if (!showRemoved) { showRemoved = true; return; }
        handled = id; toast('Ce membre n’existe plus.', 'warn'); return;
      }
      handled = id; q = ''; hl = id;
      await tick();
      document.getElementById(`mem-${id}`)?.scrollIntoView({ block: 'center', behavior: 'smooth' });
    });
  });

  async function create() {
    formError = null;
    try {
      const r = await api.post<{ member: Member; temporary_password: string; reactivated: boolean }>('/members', { email: form.email.trim(), name: form.name.trim() || null, username: form.username.trim() || null, role_id: form.role_id });
      creating = false;
      secret = { who: r.member.name, password: r.temporary_password };
      form = { email: '', name: '', username: '', role_id: 'technician' };
      toast(r.reactivated ? 'Compte retiré réactivé.' : 'Membre ajouté.');
      load();
    } catch (x) { formError = x instanceof ApiError ? x.message : 'Ajout impossible.'; }
  }
  pageHead(() => ({ count: members?.length ? plural(shown.length, 'membre') : '' }));
  async function toggleActive(m: Member) {
    if (m.active && !(await ask({ title: `Désactiver le compte de ${m.name} ?`, text: 'La personne ne pourra plus se connecter jusqu’à sa réactivation. Ses données et son historique sont conservés.', confirmLabel: 'Désactiver', danger: true }))) return;
    patch(m, { active: !m.active });
  }
  // Changement en place (rôle, activation) : le nouvel état affiché dans la ligne suffit, sans message.
  async function patch(m: Member, body: object) {
    try { await api.patch(`/members/${m.id}`, body); } catch (e) { fail(e); } finally { load(); }
  }
  async function reset() {
    const m = resetting!;
    try { const r = await api.post<{ temporary_password: string }>(`/members/${m.id}/reset-password`); secret = { who: m.name, password: r.temporary_password }; resetting = null; }
    catch (e) { fail(e); }
  }
  async function remove() {
    try { await api.del(`/members/${removing!.id}`); toast('Membre retiré.'); removing = null; } catch (e) { fail(e); } finally { load(); }
  }
  async function copy(t: string) { if (await copyText(t)) toast('Mot de passe copié.'); else toast('Copie impossible dans ce navigateur : sélectionnez le mot de passe et copiez-le à la main.', 'warn'); }
</script>

<HeadActions><button class="btn primary" onclick={() => (creating = true)}><Icon name="plus" />Nouveau membre</button></HeadActions>

<div class="toolbar">
  <label class="search"><Icon name="search" /><input class="input" bind:value={q} placeholder="Nom, e-mail ou identifiant" aria-label="Rechercher un membre"></label>
  <label class="toggle-row"><input type="checkbox" bind:checked={showRemoved}>Afficher les membres retirés du site</label>
</div>

<section class="card tbl-card">
  {#if members === null}<Loading label="Chargement des membres" />
  {:else}
  <div class="tbl-wrap">
    <table class="tbl stack mem-tbl">
      <thead><tr><SortTh label="Membre" key="name" bind:sort /><SortTh label="Rôle" key="role" bind:sort /><th>État</th><SortTh label="Ajouté" key="added" bind:sort /><th><span class="sr-only">Actions</span></th></tr></thead>
      <tbody>
        {#each shown as m (m.id)}
          {@const self = m.id === session.me?.account.id}
          {@const roleLocked = self || (m.role_id === 'owner' && !session.me?.is_owner)}
          <tr id="mem-{m.id}" class:focus={hl === m.id} class:flash={hl === m.id}>
            <td class="first"><div class="m-who"><Avatar name={m.name} color={roleOf(m.role_id)?.color} /><div><b>{m.name}</b>{#if self} <span class="chip">vous</span>{/if}<br><span class="muted small">{m.email}{m.username ? ` · ${m.username}` : ''}</span></div></div></td>
            <td data-l="Rôle">
              {#if m.removed}<span class="muted">{m.role_name}</span>
              {:else if roleLocked}<span class="role-ro" title={self ? 'Vous ne pouvez pas changer votre propre rôle.' : 'Seul un propriétaire peut changer ce rôle.'}>{m.role_name}</span>
              {:else}
                <select class="input sm" value={m.role_id} aria-label="Rôle de {m.name}" onchange={(e) => patch(m, { role_id: (e.target as HTMLSelectElement).value })}>
                  {#each roles as r}<option value={r.id}>{r.name}</option>{/each}
                </select>
              {/if}
            </td>
            <td data-l="État">{#if m.removed}<span class="chip">Retiré</span>{:else if !m.active}<span class="chip warn">Désactivé</span>{:else if m.must_change_password}<span class="chip warn">Mot de passe temporaire</span>{:else}<span class="chip ok">Actif</span>{/if}</td>
            <td class="muted small" data-l="Ajouté">{relative(m.created_at)}</td>
            <td class="acts"><div class="dl-actions">
              {#if !m.removed && !self}
                <button class="icon-btn sm" title="Réinitialiser le mot de passe de {m.name}" aria-label="Réinitialiser le mot de passe de {m.name}" onclick={() => (resetting = m)}><Icon name="lock" size="sm" /></button>
                <button class="btn sm" onclick={() => toggleActive(m)}>{m.active ? 'Désactiver' : 'Réactiver'}</button>
                <button class="icon-btn sm danger" title="Retirer {m.name} du site" aria-label="Retirer {m.name} du site" onclick={() => (removing = m)}><Icon name="trash" size="sm" /></button>
              {/if}
            </div></td>
          </tr>
        {:else}<tr><td colspan="5" class="first"><EmptyState card={false} icon="users" message={q ? 'Aucun membre ne correspond à cette recherche.' : 'Aucun membre.'} /></td></tr>{/each}
      </tbody>
    </table>
  </div>
  {/if}
</section>

{#if creating}
  <Dialog title="Nouveau membre" eyebrow="Équipe" onclose={() => (creating = false)} onsubmit={create} submitLabel="Ajouter">
    <div class="stack">
      <div class="field"><label for="m-em">E-mail</label><input class="input" id="m-em" type="email" bind:value={form.email} required></div>
      <div class="form-grid">
        <div class="field"><label for="m-nm">Nom affiché</label><input class="input" id="m-nm" bind:value={form.name} placeholder="Facultatif"></div>
        <div class="field"><label for="m-un">Identifiant</label><input class="input" id="m-un" bind:value={form.username} placeholder="Facultatif"></div>
      </div>
      <div class="field"><label for="m-ro">Rôle</label><select class="input" id="m-ro" bind:value={form.role_id} aria-describedby="m-ro-help">{#each roles.filter((r) => r.id !== 'owner') as r}<option value={r.id}>{r.name}</option>{/each}</select>
        <button type="button" class="ux3-info" id="m-ro-help" title="Un mot de passe temporaire est créé : transmettez-le à la personne, elle le remplacera à sa première connexion." aria-label="Un mot de passe temporaire est créé : transmettez-le à la personne, elle le remplacera à sa première connexion."><Icon name="info" size="sm" /></button>
      </div>
      {#if formError}<div class="alert err" role="alert">{formError}</div>{/if}
    </div>
  </Dialog>
{/if}

{#if secret}
  <Dialog title="Mot de passe temporaire" eyebrow={secret.who} locked onclose={() => (secret = null)}>
    <p>Transmettez-le de vive voix ou par un canal sûr. Il ne sera plus affiché.</p>
    <input class="secret" readonly value={secret.password} data-autofocus aria-label="Mot de passe temporaire" onfocus={(e) => (e.target as HTMLInputElement).select()}>
    {#snippet footer()}<button class="btn" onclick={() => copy(secret!.password)}><Icon name="copy" />Copier</button><button class="btn primary" onclick={() => (secret = null)}>C’est noté</button>{/snippet}
  </Dialog>
{/if}

{#if resetting}
  <Confirm title="Réinitialiser le mot de passe ?" eyebrow={resetting.name} confirmLabel="Réinitialiser" onconfirm={reset} onclose={() => (resetting = null)}>
    <p>Un nouveau mot de passe temporaire remplace l’actuel, et toutes les sessions de {resetting.name} sont fermées.</p>
  </Confirm>
{/if}
{#if removing}
  <Confirm title="Retirer {removing.name} ?" eyebrow="Équipe" confirmLabel="Retirer" danger onconfirm={remove} onclose={() => (removing = null)}>
    <p>Le compte ne pourra plus se connecter. Son historique reste dans le journal, et il peut être réactivé en l’ajoutant de nouveau avec le même e-mail.</p>
  </Confirm>
{/if}

<style>
  .m-who { display: flex; gap: 12px; align-items: center; min-width: 0; }
  .role-ro { font-weight: 550; color: var(--fg-2); }
  .mem-tbl .chip { white-space: nowrap; }
  .mem-tbl select { width: auto; max-width: 100%; }
</style>
