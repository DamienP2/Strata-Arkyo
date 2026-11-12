<script lang="ts">
  // Régies du projet (section 6.2) : ensembles ordonnés d’éléments, avec leurs totaux.
  // La régie principale est créée au premier élément et reçoit chaque nouveà l’élément.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Ill from '../../components/Ill.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import Vu from '../../components/Vu.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { nf } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { REGIE, CONTAINER_KINDS, type ProjectView, type Regie as RegieKind } from '../../lib/projects';

  interface Member { id: string; name: string; data: any; location?: string; status?: string; totals: { used_u: number; total_u: number | null; weight_kg: number; power_w: number; over_weight?: boolean; over_power?: boolean; over_u?: boolean } }
  interface R { id: string; project_id: string | null; name: string; kind: RegieKind; service_type: string | null; notes: string | null; main: boolean; rev: number; members: Member[]; totals: { containers: number; used_u: number; total_u: number; weight_kg: number; power_w: number; alerts: number } }

  let { view }: { view: ProjectView } = $props();
  let list = $state<R[] | null>(null);
  let edit = $state<{ r: R | null; name: string; kind: RegieKind; service_type: string; notes: string } | null>(null);
  let errs = $state<Record<string, string>>({});
  let deleting = $state<R | null>(null);
  let adding = $state<R | null>(null);
  let pick = $state<{ lib: R[] | null; regie: string; location: string } | null>(null);
  let busy = $state('');
  const writable = $derived(can('projets', 'ecriture'));
  const libWrite = $derived(can('catalogue', 'ecriture'));
  const pid = $derived(view.project.id);

  async function load() { try { list = (await api.get<{ regies: R[] }>(`/projects/${pid}/regies`)).regies; } catch (e) { fail(e); } }
  // Le projet se recharge à chaque événement en direct : les régies suivent.
  $effect(() => { view; untrack(load); });

  async function patch(r: R, body: object) {
    try { const n = await api.patch<R>(`/regies/${r.id}`, { rev: r.rev, ...body }); list = list!.map((x) => (x.id === r.id ? n : x)); return true; }
    catch (e) { fail(e); await load(); return false; }
  }
  const ids = (r: R) => r.members.map((m) => m.id);
  function move(r: R, i: number, d: number) {
    const m = ids(r); const j = i + d;
    if (j < 0 || j >= m.length) return;
    [m[i], m[j]] = [m[j]!, m[i]!];
    patch(r, { members: m });
  }
  const removeMember = (r: R, id: string) => patch(r, { members: ids(r).filter((x) => x !== id) });
  async function addMember(r: R, id: string) { if (await patch(r, { members: [...ids(r), id] })) adding = null; }
  const candidates = (r: R) => view.containers.filter((c) => !r.members.some((m) => m.id === c.id));

  function openEdit(r: R | null) {
    errs = {};
    edit = r ? { r, name: r.name, kind: r.kind, service_type: r.service_type ?? '', notes: r.notes ?? '' }
      : { r: null, name: '', kind: view.project.regie, service_type: view.project.service_type ?? '', notes: '' };
  }
  async function saveEdit(e: SubmitEvent) {
    e.preventDefault();
    if (!edit) return;
    const body = { name: edit.name, kind: edit.kind, service_type: edit.service_type, notes: edit.notes };
    try {
      if (edit.r) await api.patch(`/regies/${edit.r.id}`, { rev: edit.r.rev, ...body });
      else { await api.post(`/projects/${pid}/regies`, { ...body, members: [] }); toast(`Régie « ${edit.name.trim()} » créée.`); }
      edit = null; await load();
    } catch (x) {
      if (x instanceof ApiError && x.fields.length) errs = Object.fromEntries(x.fields.map((f) => [f.field, f.message])); else fail(x);
    }
  }
  async function remove() { try { await api.del(`/regies/${deleting!.id}`); toast('Régie supprimée.'); deleting = null; await load(); } catch (e) { fail(e); } }
  async function saveAsModel(r: R) {
    busy = r.id;
    try { const m = await api.post<R>(`/regies/${r.id}/save-as-template`, {}); toast(`Modèle « ${m.name} » créé avec ${m.members.length} élément${m.members.length > 1 ? 's' : ''} (en brouillon).`); }
    catch (e) { fail(e); } finally { busy = ''; }
  }
  async function openPick() {
    pick = { lib: null, regie: '', location: view.locations[0]?.id ?? '' };
    try { const l = (await api.get<{ regies: R[] }>('/regies')).regies; pick.lib = l; pick.regie = l[0]?.id ?? ''; } catch (e) { fail(e); pick = null; }
  }
  async function fromModel() {
    if (!pick) return;
    busy = 'pick';
    try {
      const r = await api.post<{ regie: R; skipped_items: number }>(`/projects/${pid}/regies/from-template`, { regie_id: pick.regie, location_id: pick.location });
      toast(`« ${r.regie.name} » ajoutée : ${r.regie.members.length} élément${r.regie.members.length > 1 ? 's' : ''} recréé${r.regie.members.length > 1 ? 's' : ''}.`);
      if (r.skipped_items) toast(`${r.skipped_items} fiche(s) disparue(s) du catalogue ignorée(s).`, 'warn');
      pick = null; await load();
    } catch (e) { fail(e); } finally { busy = ''; }
  }
  const pct = (m: Member) => (m.totals.total_u ? Math.min(100, (m.totals.used_u / m.totals.total_u) * 100) : 0);
  const over = (m: Member) => m.totals.over_weight || m.totals.over_power || m.totals.over_u;
</script>

<div class="rg-head">
  <p class="muted" style="max-width:68ch;margin:0">Une régie assemble des éléments dans un ordre donné. La régie principale reçoit chaque nouveà l’élément ; un même élément peut figurer dans plusieurs régies.</p>
  {#if writable}<div class="row-actions" style="margin:0"><button class="btn" onclick={openPick}><Icon name="copy" size="sm" />Depuis un modèle</button><button class="btn primary" onclick={() => openEdit(null)}><Icon name="plus" size="sm" />Nouvelle régie</button></div>{/if}
</div>

{#if list === null}<span class="spin"></span>
{:else if !list.length}
  <section class="card empty-card">
    <Ill kind={REGIE[view.project.regie].ill} />
    <p class="lead">Pas encore de régie : la régie principale apparaît avec le premier élément.</p>
    <a class="btn primary" href="/projets/{pid}/inventaire"><Icon name="box" size="sm" />Ajouter un élément</a>
  </section>
{:else}
  <div class="rg-list">
    {#each list as r (r.id)}
      {@const t = r.totals}
      <section class="card rg">
        <div class="rg-top">
          <span class="rg-ill"><Ill kind={REGIE[r.kind].ill} w={320} /></span>
          <div class="rg-title">
            <div class="cont-top">{#if r.main}<span class="chip accent">Principale</span>{/if}<span class="chip">{REGIE[r.kind].label}</span>{#if r.service_type}<span class="chip">{r.service_type}</span>{/if}{#if t.alerts}<span class="chip err"><Icon name="alert" />{t.alerts} alerte{t.alerts > 1 ? 's' : ''}</span>{/if}</div>
            <h2>{r.name}</h2>
            {#if r.notes}<p class="muted small" style="margin:0">{r.notes}</p>{/if}
          </div>
          <dl class="rg-kpi">
            <div><dt>Éléments</dt><dd class="mono">{t.containers}</dd></div>
            <div><dt>Hauteur</dt><dd class="mono">{nf(t.used_u, 1).replace(',0', '')} / {nf(t.total_u)} U</dd>{#if t.total_u}<Vu value={(t.used_u / t.total_u) * 100} mode="load" n={14} />{/if}</div>
            <div><dt>Poids</dt><dd class="mono">{nf(t.weight_kg, 1)} kg</dd></div>
            <div><dt>Conso</dt><dd class="mono">{nf(t.power_w)} W</dd></div>
          </dl>
        </div>

        <ol class="rg-strip" aria-label="Éléments de {r.name}, dans l’ordre">
          {#each r.members as m, i (m.id)}
            <li class="rg-m" class:alert={over(m)}>
              <span class="rg-pos mono">{i + 1}</span>
              <button class="rg-vis" title="Ouvrir dans l’inventaire" onclick={() => navigate(`/projets/${pid}/inventaire`)}>
                {#if m.data?.rackable}<span class="mini-rack" class:case={m.data.kind !== 'rack'} style="height:96px;width:38px"><i style="height:{pct(m)}%;--c:var(--sig-v)"></i></span>{:else}<span class="mini-case"></span>{/if}
              </button>
              <span class="rg-mi">
                <b>{m.name}</b>
                <span class="muted small">{CONTAINER_KINDS[m.data?.kind as keyof typeof CONTAINER_KINDS]?.label ?? ''}{m.location ? ` · ${m.location}` : ''}</span>
                <span class="muted small mono">{m.data?.rackable ? `${nf(m.totals.used_u, 1).replace(',0', '')}/${m.totals.total_u} U · ` : ''}{nf(m.totals.weight_kg, 1)} kg · {nf(m.totals.power_w)} W</span>
              </span>
              {#if writable}
                <span class="rg-act">
                  <button class="icon-btn sm" disabled={i === 0} onclick={() => move(r, i, -1)} aria-label="Avancer {m.name}" title="Avancer">‹</button>
                  <button class="icon-btn sm" disabled={i === r.members.length - 1} onclick={() => move(r, i, 1)} aria-label="Reculer {m.name}" title="Reculer">›</button>
                  <button class="icon-btn sm" onclick={() => removeMember(r, m.id)} aria-label="Retirer {m.name} de la régie" title="Retirer de la régie"><Icon name="x" size="sm" /></button>
                </span>
              {/if}
            </li>
          {/each}
          {#if writable}
            <li class="rg-add">
              {#if adding === r}
                {@const cand = candidates(r)}
                {#if cand.length}<select class="input" aria-label="Élément à ajouter" onchange={(e) => addMember(r, (e.target as HTMLSelectElement).value)}><option value="">Choisir un élément…</option>{#each cand as c}<option value={c.id}>{c.name}</option>{/each}</select>
                {:else}<span class="muted small">Tous les éléments y sont déjà.</span>{/if}
                <button class="btn sm ghost" onclick={() => (adding = null)}>Annuler</button>
              {:else}<button class="btn ghost" onclick={() => (adding = r)}><Icon name="plus" size="sm" />Ajouter un élément</button>{/if}
            </li>
          {/if}
        </ol>

        {#if writable || libWrite}
          <div class="rg-foot">
            {#if libWrite}<button class="btn sm ghost" disabled={!r.members.length || busy === r.id} onclick={() => saveAsModel(r)}>{#if busy === r.id}<span class="spin"></span>{:else}<Icon name="copy" size="sm" />{/if}Enregistrer comme modèle</button>{/if}
            <span style="flex:1"></span>
            {#if writable}<button class="btn sm ghost" onclick={() => openEdit(r)}><Icon name="pen" size="sm" />Modifier</button>
              {#if !r.main}<button class="btn sm ghost danger" onclick={() => (deleting = r)}><Icon name="trash" size="sm" />Supprimer</button>{/if}{/if}
          </div>
        {/if}
      </section>
    {/each}
  </div>
{/if}

{#if edit}
  <Dialog title={edit.r ? 'Modifier la régie' : 'Nouvelle régie'} eyebrow={view.project.name} onclose={() => (edit = null)}>
    <form id="rgf" class="stack" onsubmit={saveEdit}>
      <div class="field"><label for="rg-n">Nom</label><input class="input" id="rg-n" bind:value={edit.name} maxlength="120" required placeholder="Régie plateau">{#if errs.name}<span class="ferr">{errs.name}</span>{/if}</div>
      <div class="field"><span class="flabel">Type</span>
        <div class="kinds">{#each Object.entries(REGIE) as [k, v]}<button type="button" class="kind" class:on={edit.kind === k} onclick={() => (edit!.kind = k as RegieKind)}><Ill kind={v.ill} /><b>{v.label}</b></button>{/each}</div>
      </div>
      <div class="field"><label for="rg-s">Type de projet</label><input class="input" id="rg-s" bind:value={edit.service_type} maxlength="80" placeholder="Sport, concert, TV…">{#if errs.service_type}<span class="ferr">{errs.service_type}</span>{/if}</div>
      <div class="field"><label for="rg-no">Notes</label><textarea class="input" id="rg-no" rows="3" bind:value={edit.notes} maxlength="2000"></textarea></div>
    </form>
    {#snippet footer()}<button class="btn ghost" onclick={() => (edit = null)}>Annuler</button><button class="btn primary" type="submit" form="rgf">{edit!.r ? 'Enregistrer' : 'Créer la régie'}</button>{/snippet}
  </Dialog>
{/if}

{#if pick}
  <Dialog title="Ajouter depuis un modèle de régie" eyebrow="Bibliothèque" onclose={() => (pick = null)} wide>
    {#if pick.lib === null}<span class="spin"></span>
    {:else if !pick.lib.length}<p class="muted">La bibliothèque ne compte aucun modèle de régie. Enregistrez une régie comme modèle, ou créez-en un dans <a class="ilink" href="/catalogue/regies">Catalogue › Modèles de régies</a>.</p>
    {:else}
      <p class="muted small">Chaque élément du modèle est recréé dans le projet, avec son matériel, ses circuits et sa disposition dans la baie.</p>
      <div class="pick-list">
        {#each pick.lib as l (l.id)}
          <label class="pick" class:on={pick.regie === l.id}>
            <input type="radio" name="rgm" value={l.id} bind:group={pick.regie}>
            <span class="pick-ill"><Ill kind={REGIE[l.kind].ill} /></span>
            <span class="rg-mi"><b>{l.name}</b><span class="muted small">{REGIE[l.kind].label}{l.service_type ? ` · ${l.service_type}` : ''} · {l.members.map((m) => m.name).join(', ') || 'vide'}</span><span class="muted small mono">{l.totals.containers} élément{l.totals.containers > 1 ? 's' : ''} · {nf(l.totals.weight_kg, 1)} kg · {nf(l.totals.power_w)} W</span></span>
          </label>
        {/each}
      </div>
      <div class="field" style="max-width:340px"><label for="rg-l">Emplacement des éléments</label><select class="input" id="rg-l" bind:value={pick.location}>{#each view.locations as l}<option value={l.id}>{l.name}</option>{/each}</select></div>
    {/if}
    {#snippet footer()}<button class="btn ghost" onclick={() => (pick = null)}>Annuler</button><button class="btn primary" disabled={!pick?.regie || !pick?.location || busy === 'pick'} onclick={fromModel}>{#if busy === 'pick'}<span class="spin"></span>{/if}Ajouter au projet</button>{/snippet}
  </Dialog>
{/if}

{#if deleting}
  <Confirm title="Supprimer « {deleting.name} » ?" confirmLabel="Supprimer" danger onconfirm={remove} onclose={() => (deleting = null)}>
    <p>Seule la régie disparaît : ses {deleting.members.length} élément{deleting.members.length > 1 ? 's' : ''} restent dans le projet.</p>
  </Confirm>
{/if}

<style>
  .rg-head { display: flex; gap: 16px; align-items: flex-end; justify-content: space-between; flex-wrap: wrap; margin-bottom: 18px; }
  .rg-list { display: grid; gap: 18px; }
  .rg { padding: 20px 22px; display: grid; gap: 18px; }
  .rg-top { display: grid; grid-template-columns: 170px minmax(0, 1fr) auto; gap: 20px; align-items: center; }
  .rg-ill { display: block; width: 170px; }
  .rg-title h2 { font-size: 26px; margin: 6px 0 2px; letter-spacing: -.01em; }
  .rg-kpi { display: grid; grid-template-columns: repeat(4, auto); gap: 6px 24px; margin: 0; }
  .rg-kpi dt { font: 600 10.5px var(--mono); letter-spacing: .08em; text-transform: uppercase; color: var(--fg-3); }
  .rg-kpi dd { margin: 2px 0 4px; font-size: 18px; font-weight: 600; }
  .rg-strip { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 10px; }
  .rg-m { display: grid; grid-template-columns: auto auto minmax(0, 1fr) auto; gap: 10px; align-items: center; padding: 10px 10px 10px 8px; border-radius: 14px; background: var(--surface-2); border: 1px solid transparent; }
  .rg-m.alert { border-color: color-mix(in srgb, var(--err) 50%, transparent); }
  .rg-pos { width: 22px; height: 22px; border-radius: 50%; display: grid; place-items: center; font-size: 11px; background: var(--fg); color: var(--bg); }
  .rg-vis { border: 0; background: none; padding: 0; cursor: pointer; }
  .rg-mi { display: flex; flex-direction: column; min-width: 0; }
  .rg-mi b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rg-act { display: flex; flex-direction: column; gap: 2px; }
  .rg-act .icon-btn { width: 28px; height: 26px; font-size: 18px; line-height: 1; }
  .rg-add { display: flex; gap: 8px; align-items: center; justify-content: center; padding: 10px; border: 1.5px dashed var(--line); border-radius: 14px; min-height: 84px; flex-wrap: wrap; }
  .rg-add select { max-width: 200px; height: 38px; }
  .rg-foot { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; border-top: 1px solid var(--line); padding-top: 12px; }
  .kinds { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
  .kind { display: grid; gap: 4px; justify-items: center; padding: 8px; border-radius: 14px; border: 1.5px solid var(--line); background: var(--surface); cursor: pointer; font: inherit; color: inherit; }
  .kind.on { border-color: var(--fg); box-shadow: 0 0 0 3px color-mix(in srgb, var(--fg) 12%, transparent); }
  .kind b { font-size: 13px; }
  .pick-list { display: grid; gap: 8px; max-height: 46vh; overflow: auto; margin: 10px 0 14px; }
  .pick { display: grid; grid-template-columns: auto 110px minmax(0, 1fr); gap: 12px; align-items: center; padding: 10px 12px; border-radius: 14px; border: 1.5px solid var(--line); cursor: pointer; }
  .pick.on { border-color: var(--fg); }
  .pick-ill { display: block; width: 110px; }
  .empty-card { padding: 28px; display: grid; gap: 14px; justify-items: start; }
  .empty-card :global(.ill) { width: 260px; }
  .ferr { color: var(--err); font-size: 13px; }
  @media (max-width: 980px) { .rg-top { grid-template-columns: 120px minmax(0, 1fr); } .rg-ill { width: 120px; } .rg-kpi { grid-column: 1 / -1; grid-template-columns: repeat(2, auto); justify-content: start; } }
  @media (max-width: 560px) { .rg-top { grid-template-columns: 1fr; } .rg-ill { display: none; } .kinds { grid-template-columns: 1fr; } .pick { grid-template-columns: auto minmax(0, 1fr); } .pick-ill { display: none; } }
</style>
