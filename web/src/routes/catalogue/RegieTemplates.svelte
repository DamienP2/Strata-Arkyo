<script lang="ts">
  // Bibliothèque des modèles de régies (section 6.2) : chaque modèle assemble des modèles de
  // éléments dans un ordre donné ; on le recopie dans un projet depuis l'onglet Régies.
  import Icon from '../../components/Icon.svelte';
  import Ill from '../../components/Ill.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import HeadActions from '../../components/HeadActions.svelte';
  import { pageHead, plural } from '../../lib/pagehead.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { nf, relative } from '../../lib/format';
  import { REGIE, CONTAINER_KINDS, type Regie as RegieKind } from '../../lib/projects';

  interface Member { id: string; name: string; data: any; status: string; totals: { weight_kg: number; power_w: number } }
  interface R { id: string; name: string; kind: RegieKind; service_type: string | null; notes: string | null; rev: number; updated_at: string; members: Member[]; totals: { containers: number; used_u: number; total_u: number; weight_kg: number; power_w: number; alerts: number } }
  interface T { id: string; name: string; status: string; container: any }

  let list = $state<R[] | null>(null);
  let templates = $state<T[]>([]);
  let q = $state('');
  let edit = $state<{ r: R | null; name: string; kind: RegieKind; service_type: string; notes: string; members: string[] } | null>(null);
  let errs = $state<Record<string, string>>({});
  let deleting = $state<R | null>(null);
  const writable = $derived(can('catalogue', 'ecriture'));
  const STATUS: Record<string, string> = { draft: 'brouillon', inactive: 'inactif', active: 'actif' };

  async function load() {
    try {
      const [r, t] = await Promise.all([api.get<{ regies: R[] }>('/regies'), api.get<{ templates: T[] }>('/templates')]);
      list = r.regies; templates = t.templates;
    } catch (e) { fail(e); }
  }
  load();
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const shown = $derived((list ?? []).filter((r) => !q || norm(r.name).includes(norm(q))));
  pageHead(() => ({ count: list?.length ? plural(list.length, 'modèle de régie', 'modèles de régie') : '' }));
  /** Nom du modèle d’élément sans le préfixe « {régie} · » qu'il reçoit à l'enregistrement. */
  const short = (r: { name: string }, n: string) => (n.startsWith(`${r.name} · `) ? n.slice(r.name.length + 3) : n);
  const tName = (id: string) => templates.find((t) => t.id === id)?.name ?? 'Modèle supprimé';

  function openEdit(r: R | null) {
    errs = {};
    edit = r ? { r, name: r.name, kind: r.kind, service_type: r.service_type ?? '', notes: r.notes ?? '', members: r.members.map((m) => m.id) }
      : { r: null, name: '', kind: 'car', service_type: '', notes: '', members: [] };
  }
  function move(i: number, d: number) {
    const m = edit!.members, j = i + d;
    if (j < 0 || j >= m.length) return;
    [m[i], m[j]] = [m[j]!, m[i]!];
  }
  async function save(e: SubmitEvent) {
    e.preventDefault();
    const b = { name: edit!.name, kind: edit!.kind, service_type: edit!.service_type, notes: edit!.notes, members: edit!.members };
    try {
      if (edit!.r) await api.patch(`/regies/${edit!.r.id}`, { rev: edit!.r.rev, ...b });
      else await api.post('/regies', b);
      toast(edit!.r ? 'Modèle enregistré.' : `Modèle « ${edit!.name.trim()} » créé.`);
      edit = null; await load();
    } catch (x) { if (x instanceof ApiError && x.fields.length) errs = Object.fromEntries(x.fields.map((f) => [f.field, f.message])); else fail(x); }
  }
  async function remove() { try { await api.del(`/regies/${deleting!.id}`); toast('Modèle supprimé.'); deleting = null; await load(); } catch (e) { fail(e); } }
</script>

<p class="muted" style="max-width:70ch">Un modèle de régie assemble des modèles d’éléments dans un ordre donné. Créez-en un ici ou depuis l’onglet Régies d’un projet (« Enregistrer comme modèle »), puis recopiez-le dans n’importe quel projet : chaque élément est recréé avec son contenu.</p>
{#if writable && list?.length}<HeadActions><button class="btn primary" onclick={() => openEdit(null)}><Icon name="plus" />Nouveau modèle de régie</button></HeadActions>{/if}

{#if list === null}<span class="spin"></span>
{:else if !list.length}
  <EmptyState icon="layers" message="La bibliothèque ne compte encore aucun modèle de régie.">{#snippet action()}{#if writable}<button class="btn primary" onclick={() => openEdit(null)}><Icon name="plus" />Nouveau modèle de régie</button>{/if}{/snippet}</EmptyState>
{:else}
<div class="toolbar" style="display:flex;gap:10px;flex-wrap:wrap;align-items:center">
  <label class="search" style="flex:1;min-width:220px;max-width:480px"><Icon name="search" /><input class="input" bind:value={q} placeholder="Nom du modèle…" aria-label="Rechercher un modèle de régie"></label>
</div>
{#if !shown.length}
  <EmptyState icon="search" message="Aucun modèle ne correspond à cette recherche." />
{:else}
  <div class="rt-grid">
    {#each shown as r (r.id)}
      <article class="card rt">
        <span class="rt-ill"><Ill kind={REGIE[r.kind].ill} /></span>
        <div class="rt-body">
          <div class="cont-top"><span class="chip">{REGIE[r.kind].label}</span>{#if r.service_type}<span class="chip">{r.service_type}</span>{/if}</div>
          <b class="rt-name">{r.name}</b>
          <ol class="rt-members">{#each r.members as m, i (m.id + i)}<li title={m.name}><span class="mono">{i + 1}</span><span class="rt-mn">{short(r, m.name)}</span>{#if m.status !== 'active'}<span class="muted small">{STATUS[m.status]}</span>{/if}</li>{:else}<li class="muted">Aucun élément</li>{/each}</ol>
          <span class="muted small mono">{r.totals.containers} élément{r.totals.containers > 1 ? 's' : ''} · {nf(r.totals.total_u)} U · {nf(r.totals.weight_kg, 1)} kg · {nf(r.totals.power_w)} W · modifié {relative(r.updated_at)}</span>
        </div>
        {#if writable}<div class="dl-actions"><button class="btn sm ghost" title="Modifier" aria-label="Modifier {r.name}" onclick={() => openEdit(r)}><Icon name="pen" size="sm" /></button><button class="btn sm ghost danger" title="Supprimer" aria-label="Supprimer {r.name}" onclick={() => (deleting = r)}><Icon name="trash" size="sm" /></button></div>{/if}
      </article>
    {/each}
  </div>
{/if}
{/if}

{#if edit}
  <Dialog title={edit.r ? 'Modifier le modèle' : 'Nouveau modèle de régie'} eyebrow="Bibliothèque" onclose={() => (edit = null)} wide>
    <form id="rtf" class="stack" onsubmit={save}>
      <div class="fg2">
        <div class="field"><label for="rt-n">Nom</label><input class="input" id="rt-n" bind:value={edit.name} maxlength="120" required placeholder="Car HD 6 caméras">{#if errs.name}<span class="ferr">{errs.name}</span>{/if}</div>
        <div class="field"><label for="rt-s">Type de projet</label><input class="input" id="rt-s" bind:value={edit.service_type} maxlength="80" placeholder="Sport, concert, TV…"></div>
      </div>
      <div class="field"><span class="flabel">Type</span><div class="seg">{#each Object.entries(REGIE) as [k, v]}<button type="button" class:on={edit.kind === k} onclick={() => (edit!.kind = k as RegieKind)}>{v.label}</button>{/each}</div></div>
      <div class="field"><span class="flabel">Éléments, dans l’ordre</span>
        <ol class="rt-edit">
          {#each edit.members as id, i (id + i)}
            <li><span class="mono">{i + 1}</span><b>{tName(id)}</b><span style="flex:1"></span>
              <button type="button" class="icon-btn sm" disabled={i === 0} onclick={() => move(i, -1)} aria-label="Monter">↑</button>
              <button type="button" class="icon-btn sm" disabled={i === edit.members.length - 1} onclick={() => move(i, 1)} aria-label="Descendre">↓</button>
              <button type="button" class="icon-btn sm" onclick={() => edit!.members.splice(i, 1)} aria-label="Retirer"><Icon name="x" size="sm" /></button></li>
          {/each}
        </ol>
        {#if templates.length}
          <select class="input" style="max-width:360px" aria-label="Ajouter un modèle d’élément" onchange={(e) => { const s = e.target as HTMLSelectElement; if (s.value) edit!.members.push(s.value); s.value = ''; }}>
            <option value="">+ Ajouter un modèle d’élément…</option>
            {#each templates as t}<option value={t.id}>{t.name} · {CONTAINER_KINDS[t.container.kind as keyof typeof CONTAINER_KINDS]?.label}{t.status !== 'active' ? ` (${STATUS[t.status]})` : ''}</option>{/each}
          </select>
        {:else}<p class="muted small">Aucun modèle d’élément : créez-en depuis l’inventaire d’un projet.</p>{/if}
        {#if errs.members}<span class="ferr">{errs.members}</span>{/if}
      </div>
      <div class="field"><label for="rt-no">Notes</label><textarea class="input" id="rt-no" rows="2" bind:value={edit.notes} maxlength="2000"></textarea></div>
    </form>
    {#snippet footer()}<button class="btn ghost" onclick={() => (edit = null)}>Annuler</button><button class="btn primary" type="submit" form="rtf">{edit!.r ? 'Enregistrer' : 'Créer le modèle'}</button>{/snippet}
  </Dialog>
{/if}

{#if deleting}
  <Confirm title="Supprimer « {deleting.name} » ?" confirmLabel="Supprimer" danger onconfirm={remove} onclose={() => (deleting = null)}>
    <p>Le modèle de régie quitte la bibliothèque. Ses modèles d’éléments restent, ainsi que les régies déjà recopiées dans les projets.</p>
  </Confirm>
{/if}

<style>
  .rt-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 460px), 1fr)); gap: 16px; }
  .rt { padding: 16px; display: grid; grid-template-columns: 120px minmax(0, 1fr) auto; gap: 14px; align-items: start; }
  .rt-ill { display: block; width: 120px; }
  .rt-body { display: grid; gap: 6px; min-width: 0; }
  .rt-name { font-size: 18px; }
  .rt-members { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 6px; }
  .rt-members li { max-width: 100%; white-space: nowrap; display: flex; gap: 6px; align-items: center; padding: 4px 10px 4px 4px; border-radius: 99px; background: var(--surface-2); font-size: 13px; }
  .rt-members li .mono, .rt-edit li .mono { width: 20px; height: 20px; border-radius: 50%; display: grid; place-items: center; font-size: 10.5px; background: var(--fg); color: var(--bg); }
  .rt-mn { overflow: hidden; text-overflow: ellipsis; }
  .rt-edit { list-style: none; margin: 0 0 8px; padding: 0; display: grid; gap: 6px; }
  .rt-edit li { display: flex; gap: 8px; align-items: center; padding: 6px 8px; border-radius: 12px; background: var(--surface-2); }
  .fg2 { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
  .ferr { color: var(--err); font-size: 13px; }
  @media (max-width: 560px) { .rt { grid-template-columns: minmax(0, 1fr) auto; } .rt-ill { display: none; } .fg2 { grid-template-columns: 1fr; } }
</style>
