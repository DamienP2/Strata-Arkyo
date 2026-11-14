<script lang="ts">
  import { copyText } from '../../lib/clipboard';
  import Icon from '../../components/Icon.svelte';
  import Avatar from '../../components/Avatar.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import { api, ApiError } from '../../lib/api';
  import { session } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { relative } from '../../lib/format';
  import type { Member, Role } from '../../lib/types';

  let members = $state<Member[]>([]);
  let roles = $state<Role[]>([]);
  let showRemoved = $state(false);
  let q = $state('');
  let creating = $state(false);
  let form = $state({ email: '', name: '', username: '', role_id: 'technician' });
  let formError = $state<string | null>(null);
  let secret = $state<{ who: string; password: string } | null>(null);
  let removing = $state<Member | null>(null);
  let resetting = $state<Member | null>(null);

  const roleOf = (id: string) => roles.find((r) => r.id === id);
  const shown = $derived(members.filter((m) => !q || `${m.name} ${m.email} ${m.username ?? ''}`.toLowerCase().includes(q.toLowerCase())));

  async function load() {
    try {
      const [m, r] = await Promise.all([api.get<{ members: Member[] }>(`/members?removed=${showRemoved}`), api.get<{ roles: Role[] }>('/roles')]);
      members = m.members; roles = r.roles;
    } catch (e) { fail(e); }
  }
  $effect(() => { showRemoved; load(); });

  async function create(e: SubmitEvent) {
    e.preventDefault();
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
  async function patch(m: Member, body: object, ok: string) {
    try { await api.patch(`/members/${m.id}`, body); toast(ok); } catch (e) { fail(e); } finally { load(); }
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

<div class="toolbar" style="display:flex;gap:10px;flex-wrap:wrap;align-items:center">
  <label class="search" style="flex:1;min-width:220px"><Icon name="search" /><input class="input" bind:value={q} placeholder="Nom, e-mail, identifiant…" aria-label="Rechercher un membre"></label>
  <label class="layer" style="display:flex;gap:8px;align-items:center"><input type="checkbox" bind:checked={showRemoved}>Afficher les retirés</label>
  <button class="btn primary" onclick={() => (creating = true)}><Icon name="plus" />Ajouter un membre</button>
</div>

<section class="card" style="padding:6px 18px 10px">
  <div class="tbl-wrap">
    <table class="tbl">
      <thead><tr><th>Membre</th><th>Rôle</th><th>État</th><th>Ajouté</th><th></th></tr></thead>
      <tbody>
        {#each shown as m (m.id)}
          {@const self = m.id === session.me?.account.id}
          <tr>
            <td><div style="display:flex;gap:12px;align-items:center"><Avatar name={m.name} color={roleOf(m.role_id)?.color} /><div><b>{m.name}</b>{#if self} <span class="chip">vous</span>{/if}<br><span class="muted small">{m.email}{m.username ? ` · ${m.username}` : ''}</span></div></div></td>
            <td>
              {#if m.removed}<span class="muted">{m.role_name}</span>
              {:else}
                <select class="input" style="height:38px;width:auto" value={m.role_id} disabled={self} aria-label="Rôle de {m.name}" onchange={(e) => patch(m, { role_id: (e.target as HTMLSelectElement).value }, 'Rôle changé.')}>
                  {#each roles as r}<option value={r.id}>{r.name}</option>{/each}
                </select>
              {/if}
            </td>
            <td>{#if m.removed}<span class="chip">Retiré</span>{:else if !m.active}<span class="chip warn">Désactivé</span>{:else if m.must_change_password}<span class="chip info">Mot de passe temporaire</span>{:else}<span class="chip ok">Actif</span>{/if}</td>
            <td class="muted small">{relative(m.created_at)}</td>
            <td><div class="dl-actions">
              {#if !m.removed && !self}
                <button class="btn sm" onclick={() => patch(m, { active: !m.active }, m.active ? 'Compte désactivé.' : 'Compte réactivé.')}>{m.active ? 'Désactiver' : 'Réactiver'}</button>
                <button class="btn sm ghost" title="Réinitialiser le mot de passe" onclick={() => (resetting = m)}><Icon name="lock" size="sm" /></button>
                <button class="btn sm ghost danger" title="Retirer du site" onclick={() => (removing = m)}><Icon name="trash" size="sm" /></button>
              {/if}
            </div></td>
          </tr>
        {:else}<tr><td colspan="5" class="empty">Aucun membre ne correspond.</td></tr>{/each}
      </tbody>
    </table>
  </div>
</section>

{#if creating}
  <Dialog title="Ajouter un membre" eyebrow="Équipe" onclose={() => (creating = false)}>
    <form id="newm" class="stack" onsubmit={create}>
      <div class="field"><label for="m-em">E-mail</label><input class="input" id="m-em" type="email" bind:value={form.email} required></div>
      <div class="form-grid" style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
        <div class="field"><label for="m-nm">Nom affiché</label><input class="input" id="m-nm" bind:value={form.name} placeholder="Facultatif"></div>
        <div class="field"><label for="m-un">Identifiant</label><input class="input" id="m-un" bind:value={form.username} placeholder="Facultatif"></div>
      </div>
      <div class="field"><label for="m-ro">Rôle</label><select class="input" id="m-ro" bind:value={form.role_id}>{#each roles.filter((r) => r.id !== 'owner') as r}<option value={r.id}>{r.name}</option>{/each}</select></div>
      <p class="muted small">Un mot de passe temporaire est créé : transmettez-le à la personne, elle le remplacera à sa première connexion.</p>
      {#if formError}<div class="alert err" role="alert">{formError}</div>{/if}
    </form>
    {#snippet footer()}<button class="btn ghost" onclick={() => (creating = false)}>Annuler</button><button class="btn primary" type="submit" form="newm">Ajouter</button>{/snippet}
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
