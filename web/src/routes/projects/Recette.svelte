<script lang="ts">
  import { ask } from '../../lib/ask.svelte';
  // Recette (section 11.5) : liaisons, équipements posés et points libres, verdicts commentés,
  // journal par point, avancement, procès-verbal PDF. Écran partagé en direct.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Loading from '../../components/Loading.svelte';
  import SaveState from '../../components/SaveState.svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { relative, pl } from '../../lib/format';
  import type { ProjectView } from '../../lib/projects';

  type St = 'to_test' | 'ok' | 'reserve' | 'ko';
  interface Pt { key: string; label: string; detail: string; deleted: boolean; status: St; comment: string | null; by: string | null; at: string | null }
  interface Group { title: string; points: Pt[] }
  interface R { progress: number; total: number; counts: Record<St, number>; cables: Group[]; equipment: Group[]; free: Pt[]; deleted: Pt[]; reserves: Pt[] }

  let { view }: { view: ProjectView } = $props();
  const pid = $derived(view.project.id);
  const writable = $derived(can('projets', 'ecriture'));
  const ST: Record<St, { label: string; prefill: string }> = {
    to_test: { label: 'À tester', prefill: '' }, ok: { label: 'OK', prefill: 'Conforme.' },
    reserve: { label: 'Réserve', prefill: 'Conforme avec réserve : ' }, ko: { label: 'KO', prefill: 'Non conforme : ' },
  };
  let r = $state<R | null>(null);
  let filter = $state<'' | St>('');
  let q = $state('');
  let openLog = $state<string | null>(null);
  let logs = $state<{ at: string; by: string | null; status: string | null; comment: string | null }[]>([]);
  let note = $state('');
  let newFree = $state('');
  let paper = $state<'a4' | 'a3'>('a4');
  let showDeleted = $state(false);
  let exporting = $state(false);
  let renaming = $state<{ p: Pt; label: string } | null>(null);

  const MISSING = 'Extrémité supprimée';
  const UNNUMBERED = 'Câble non numéroté';
  // Les liaisons dont un bout a disparu sont mises à part, repliées, avec leur nombre : elles
  // ne comptent ni dans le total ni dans l'avancement (on ne peut pas juger un câble qui n'existe plus).
  const missing = $derived(r?.cables.find((g) => g.title === MISSING) ?? null);
  const liaisons = $derived((r?.cables ?? []).filter((g) => g.title !== MISSING));
  const nPts = (gs: Group[]) => gs.reduce((n, g) => n + g.points.length, 0);
  // Avancement recalculé sur les seuls points à juger : l'écran et le procès-verbal donnent les mêmes chiffres.
  const stats = $derived.by(() => {
    const pts = r ? [...liaisons.flatMap((g) => g.points), ...r.equipment.flatMap((g) => g.points), ...r.free] : [];
    const counts: Record<St, number> = { to_test: 0, ok: 0, reserve: 0, ko: 0 };
    for (const p of pts) counts[p.status] = (counts[p.status] ?? 0) + 1;
    const total = pts.length, tested = total - counts.to_test;
    return { total, tested, counts, progress: total ? Math.round((100 * tested) / total) : 0 };
  });
  const totalHint = $derived(r ? `Une liaison par câble (${nPts(liaisons)}), un point par équipement rangé dans un élément (${nPts(r.equipment)}) et les points libres (${r.free.length}).${missing ? ` Les ${missing.points.length} liaison${pl(missing.points.length)} sans extrémité ne compte${missing.points.length > 1 ? 'nt' : ''} pas.` : ''}` : '');
  /** Titre d'un point : pour un câble sans numéro, ses extrémités (sinon toutes les lignes s'appelleraient pareil). */
  const unnumbered = (p: Pt) => p.label === UNNUMBERED && !!p.detail;
  async function load() { try { r = await api.get(`/projects/${pid}/recette`); if (openLog) loadLog(openLog); } catch (e) { fail(e); } }
  $effect(() => { view; untrack(load); });
  async function loadLog(key: string) { try { logs = (await api.get<{ entries: typeof logs }>(`/projects/${pid}/recette/log?key=${encodeURIComponent(key)}`)).entries; } catch (e) { fail(e); } }

  async function verdict(p: Pt, status: St, comment?: string) {
    const c = comment ?? (p.comment && !Object.values(ST).some((s) => s.prefill && p.comment === s.prefill) ? p.comment : ST[status].prefill);
    try { await api.post(`/projects/${pid}/recette/verdict`, { key: p.key, status, comment: c }); await load(); } catch (e) { fail(e); }
  }
  async function addNote(key: string) { if (!note.trim()) return; try { await api.post(`/projects/${pid}/recette/comment`, { key, comment: note }); note = ''; await loadLog(key); } catch (e) { fail(e); } }
  async function addFree() { if (!newFree.trim()) return; try { await api.post(`/projects/${pid}/recette/free`, { label: newFree }); newFree = ''; await load(); } catch (e) { fail(e); } }
  async function renameFree() {
    if (!renaming) return;
    const { p, label } = renaming;
    if (!label.trim() || label === p.label) { renaming = null; return; }
    try { await api.patch(`/projects/${pid}/recette/free`, { key: p.key, label: label.trim() }); renaming = null; toast(`Point « ${label.trim()} » renommé.`); await load(); } catch (e) { fail(e); }
  }
  async function deleteFree(p: Pt) {
    if (!(await ask({ title: `Supprimer « ${p.label} » ?`, text: 'Le point libre et son journal sont retirés de la recette.', confirmLabel: 'Supprimer', danger: true, run: () => api.post(`/projects/${pid}/recette/free/delete`, { key: p.key }) }))) return;
    toast(`« ${p.label} » supprimé.`);
    await load();
  }
  async function moveFree(i: number, d: number) {
    if (!r) return; const keys = r.free.map((p) => p.key); const j = i + d; if (j < 0 || j >= keys.length) return;
    [keys[i], keys[j]] = [keys[j]!, keys[i]!];
    try { await api.post(`/projects/${pid}/recette/free/order`, { keys }); await load(); } catch (e) { fail(e); }
  }
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const keep = (p: Pt) => (!filter || p.status === filter) && (!q || norm(`${p.label} ${p.detail} ${p.comment ?? ''}`).includes(norm(q)));

  // ----- Procès-verbal PDF -----
  async function pv() {
    if (!r || exporting) return;
    exporting = true;
    try {
      const kit = await import('../../lib/pdfkit');
      const { drawPV, pvEnds } = await import('../../lib/pv');
      const st = await kit.siteInfo();
      const doc = await kit.newDoc(paper === 'a3' ? 'A3' : 'A4', false);
      // Mêmes chiffres qu'à l'écran : les liaisons sans extrémité ne comptent pas.
      const data = { ...$state.snapshot(r), progress: stats.progress, total: stats.total, counts: stats.counts, reserves: r.reserves.filter((p) => !missing?.points.some((m) => m.key === p.key)) };
      drawPV(doc, data, view, st.name, st.logo, paper === 'a3' ? 'A3' : 'A4', await pvEnds(view.project.id));
      kit.paginate(doc, 1, undefined, `${view.project.name} · procès-verbal de recette`);
      const name = view.project.name.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^A-Za-z0-9]+/g, '_');
      doc.save(`Recette_${name}_${new Date().toISOString().slice(0, 10)}.pdf`);
      toast('Procès-verbal exporté.');
    } catch (e) { fail(e); } finally { exporting = false; }
  }
</script>

{#snippet row(p: Pt, free = false, i = 0)}
  {@const title = unnumbered(p) ? p.detail : p.label}
  <div class="pt" class:open={openLog === p.key}>
    <div class="pt-main">
      <div class="pt-t"><b>{title}</b>{#if unnumbered(p)}<span class="muted small">non numéroté</span>{:else if p.detail}<span class="muted small">{p.detail}</span>{/if}</div>
      <div class="pt-st">
        <div class="pt-sv" role="group" aria-label="Verdict de {title}">
          {#each Object.entries(ST) as [k, s]}<button class="sv {k}" class:on={p.status === k} aria-pressed={p.status === k} disabled={!writable || p.deleted} onclick={() => verdict(p, k as St)}>{s.label}</button>{/each}
        </div>
        <button class="btn sm ghost pt-log-b" aria-expanded={openLog === p.key} onclick={() => { openLog = openLog === p.key ? null : p.key; if (openLog) loadLog(p.key); }}>Journal</button>
      </div>
      <div class="pt-c">
        {#if p.status !== 'to_test'}<input class="input sm" value={p.comment ?? ''} disabled={!writable || p.deleted} maxlength="2000" onchange={(e) => verdict(p, p.status, (e.target as HTMLInputElement).value)} aria-label="Commentaire du verdict de {title}">{/if}
        {#if p.at}<span class="muted small">{p.by ?? ''} · {relative(p.at)}</span>{/if}
      </div>
      <div class="pt-a">
        {#if free && writable}<button class="icon-btn sm" disabled={i === 0} onclick={() => moveFree(i, -1)} aria-label="Monter « {p.label} »" title="Monter « {p.label} »">↑</button><button class="icon-btn sm" disabled={i === (r?.free.length ?? 1) - 1} onclick={() => moveFree(i, 1)} aria-label="Descendre « {p.label} »" title="Descendre « {p.label} »">↓</button>
          <button class="icon-btn sm" onclick={() => (renaming = { p, label: p.label })} aria-label="Renommer « {p.label} »" title="Renommer « {p.label} »"><Icon name="pen" size="sm" /></button><button class="icon-btn sm danger" onclick={() => deleteFree(p)} aria-label="Supprimer « {p.label} »" title="Supprimer « {p.label} »"><Icon name="trash" size="sm" /></button>{/if}
      </div>
    </div>
    {#if openLog === p.key}
      <div class="pt-log">
        {#each logs as l}<div class="lg"><span class="muted small">{new Date(l.at).toLocaleString('fr-FR')} · {l.by ?? 'Système'}</span>{#if l.status}<span class="chip">{ST[l.status as St]?.label}</span>{/if}<span>{l.comment ?? ''}</span></div>{:else}<span class="muted small">Aucune entrée.</span>{/each}
        {#if writable && !p.deleted}<div class="lg-add"><input class="input sm" bind:value={note} maxlength="2000" placeholder="Ajouter un commentaire au journal" aria-label="Commentaire du journal" onkeydown={(e) => { if (e.key === 'Enter') addNote(p.key); }}><button class="btn sm" onclick={() => addNote(p.key)}>Ajouter</button></div>{/if}
      </div>
    {/if}
  </div>
{/snippet}

{#if !r}<Loading label="Chargement de la recette" />
{:else}
  <div class="rc-head card">
    <div class="rc-big"><span class="eyebrow">Recette</span><b class="mono">{stats.progress}<span class="pc">%</span></b><span class="muted small" title={totalHint}>{stats.tested} point{stats.tested > 1 ? 's' : ''} testé{stats.tested > 1 ? 's' : ''} sur {stats.total} <span class="rc-q" aria-hidden="true">?</span></span></div>
    <div class="rc-counts" role="group" aria-label="Filtrer par verdict">
      {#each Object.entries(ST) as [k, s]}<button class="cnt {k}" class:on={filter === k} aria-pressed={filter === k} onclick={() => (filter = filter === k ? '' : (k as St))}><b class="mono">{stats.counts[k as St]}</b>{s.label}</button>{/each}
    </div>
    <div class="rc-act">
      <select class="input" bind:value={paper} aria-label="Papier du procès-verbal"><option value="a4">A4</option><option value="a3">A3</option></select>
      <button class="btn primary" disabled={exporting} onclick={pv}>{#if exporting}<span class="spin" aria-hidden="true"></span>Génération du procès-verbal{:else}<Icon name="download" size="sm" />Procès-verbal PDF{/if}</button>
    </div>
  </div>
  <div class="rc-tools"><label class="search"><Icon name="search" /><input class="input" type="search" bind:value={q} placeholder="Rechercher un point" aria-label="Rechercher un point"></label><span class="grow"></span><SaveState /></div>

  {#if missing && missing.points.some(keep)}
    <details class="card compact rc-miss">
      <summary><Icon name="alert" size="sm" />{missing.points.length} liaison{missing.points.length > 1 ? 's' : ''} dont une extrémité n’existe plus (équipement ou port supprimé), hors recette<a class="ag-all small" href="/projets/{pid}/cables?cable={missing.points.map((p) => p.key.slice(6)).join(',')}" onclick={(e) => e.stopPropagation()}>Voir ces câbles</a></summary>
      <p class="muted small">À corriger dans le synoptique (rebrancher ou supprimer le câble) : elles ne comptent pas dans l’avancement.</p>
      {#each missing.points.filter(keep) as p (p.key)}{@render row(p)}{/each}
    </details>
  {/if}
  {#each [['Liaisons', liaisons], ['Équipements', r.equipment]] as [title, groups]}
    {#if (groups as Group[]).some((g) => g.points.some(keep))}
      <h2 class="rc-h">{title}</h2>
      {#each groups as Group[] as g (g.title)}
        {#if g.points.some(keep)}<section class="card rc-g"><span class="lbl-t">{g.title || 'Sans élément'}</span>{#each g.points.filter(keep) as p (p.key)}{@render row(p)}{/each}</section>{/if}
      {/each}
    {/if}
  {/each}
  <h2 class="rc-h">Points libres</h2>
  <section class="card rc-g">
    {#each r.free as p, i (p.key)}{#if keep(p)}{@render row(p, true, i)}{/if}{:else}<span class="muted small">Aucun point libre.</span>{/each}
    {#if writable}<div class="lg-add"><input class="input sm" bind:value={newFree} maxlength="200" placeholder="Nouveau point : essai de bascule secours, lecture timecode" aria-label="Nouveau point libre" onkeydown={(e) => { if (e.key === 'Enter') addFree(); }}><button class="btn sm" onclick={addFree}><Icon name="plus" size="sm" />Ajouter</button></div>{/if}
  </section>
  {#if r.deleted.length}
    <button class="btn sm ghost rc-del" aria-expanded={showDeleted} onclick={() => (showDeleted = !showDeleted)}>{r.deleted.length} verdict{pl(r.deleted.length)} d’éléments supprimés</button>
    {#if showDeleted}<section class="card rc-g">{#each r.deleted as p (p.key)}{@render row(p)}{/each}</section>{/if}
  {/if}
{/if}

{#if renaming}
  <Dialog title="Renommer le point libre" eyebrow="Recette" onclose={() => (renaming = null)} onsubmit={renameFree} submitLabel="Renommer" submitDisabled={!renaming.label.trim()}>
    <div class="field"><label for="rc-ren">Libellé</label><input class="input" id="rc-ren" bind:value={renaming.label} maxlength="200" required></div>
  </Dialog>
{/if}

<style>
  .rc-head { padding: 18px 20px; display: grid; grid-template-columns: auto minmax(0, 1fr) auto; gap: 20px; align-items: center; }
  .rc-big { display: flex; flex-direction: column; }
  .rc-big b { font-size: 44px; line-height: 1; letter-spacing: -.02em; }
  .rc-big .pc { margin-left: .08em; font-size: .6em; }
  .rc-counts { display: flex; gap: 8px; flex-wrap: wrap; }
  /* Compteurs-filtres : le filtre actif se marque comme un .fchip.on (fond d'accent, liseré). */
  .cnt { display: flex; gap: 8px; align-items: baseline; padding: 8px 12px; border-radius: 12px; border: 0; background: var(--surface-2); cursor: pointer; font: inherit; color: inherit; transition: background .12s, box-shadow .12s; }
  .cnt:hover:not(.on) { box-shadow: inset 0 0 0 1px var(--line-2); }
  .cnt.on { background: var(--accent-soft); color: var(--accent-2); box-shadow: inset 0 0 0 1.5px var(--accent); }
  .cnt:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .cnt b { font-size: 18px; }
  .cnt.ok b { color: var(--ok); } .cnt.reserve b { color: var(--warn); } .cnt.ko b { color: var(--err); }
  .rc-act { display: flex; gap: 8px; align-items: center; }
  .rc-act select { width: auto; }
  .rc-q { display: inline-grid; place-items: center; width: 16px; height: 16px; border-radius: 50%; border: 1px solid currentColor; font-size: 11px; cursor: help; }
  /* La page empile ses blocs avec un écart régulier : pas de marge en plus. */
  .rc-tools { display: flex; gap: 10px; align-items: center; margin: 0; }
  .rc-tools .search { flex: 1; max-width: 520px; }
  .grow { flex: 1; }
  .rc-miss { margin: 0; box-shadow: 0 0 0 1px color-mix(in srgb, var(--warn) 40%, var(--line)); }
  .rc-miss summary { cursor: pointer; font-weight: 600; display: flex; gap: 8px; align-items: center; color: var(--warn); }
  .rc-miss summary a { margin-left: auto; }
  .rc-h { font-size: 20px; margin: 6px 0 -8px; }
  .rc-g { padding: 12px 14px; display: grid; gap: 4px; margin: 0; }
  .rc-del { justify-self: start; margin-top: 12px; }
  .pt { border-top: 1px solid var(--line); padding: 8px 0; }
  .pt:first-of-type { border-top: 0; }
  .pt-main { display: grid; grid-template-columns: minmax(200px, 1.3fr) auto minmax(180px, 1fr) auto; gap: 12px; align-items: center; }
  .pt-a:empty { display: none; }
  .pt-t { display: flex; flex-direction: column; min-width: 0; overflow-wrap: anywhere; }
  .pt-st { display: flex; gap: 6px; align-items: center; }
  .pt-sv { display: flex; gap: 4px; align-items: center; }
  /* Verdicts (variante légitime .sv) : pilules de 28px sans bordure, colorées selon le verdict. */
  .sv { --c: var(--fg-2); height: 28px; padding: 0 11px; border-radius: 99px; border: 0; background: var(--surface-2); cursor: pointer; font: 600 12.5px var(--font); color: var(--fg-2); transition: background .12s, color .12s, box-shadow .12s; }
  .sv.ok { --c: var(--ok); } .sv.reserve { --c: var(--warn); } .sv.ko { --c: var(--err); }
  .sv:not(.on):not(:disabled):hover { color: var(--c); box-shadow: inset 0 0 0 1px var(--c); }
  .sv:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .sv:disabled { cursor: not-allowed; }
  .sv:disabled:not(.on) { opacity: .45; }
  /* Verdict choisi : la couleur du résumé (texte, liseré et fond teinté), lisible dans les deux thèmes. */
  .sv.on.ok, .sv.on.reserve, .sv.on.ko { background: color-mix(in srgb, var(--c) 18%, var(--surface)); box-shadow: inset 0 0 0 1.5px var(--c); color: var(--c); }
  .sv.on.to_test { background: var(--surface-3); box-shadow: inset 0 0 0 1.5px var(--fg-3); color: var(--fg); }
  .pt-c { display: flex; flex-direction: column; gap: 2px; }
  .pt-a { display: flex; gap: 4px; align-items: center; }
  .pt-log { margin: 8px 0 4px; padding: 10px; border-radius: 12px; background: var(--surface-2); display: grid; gap: 6px; }
  .lg { display: flex; gap: 8px; align-items: baseline; flex-wrap: wrap; font-size: 14px; }
  .lg-add { display: flex; gap: 8px; margin-top: 6px; }
  .lg-add .input { flex: 1; }
  @media (max-width: 980px) { .rc-head { grid-template-columns: 1fr; } .pt-main { grid-template-columns: 1fr; } }
  /* Téléphone : synthèse sur une ligne (pourcentage et quatre compteurs), verdicts en groupe pleine largeur de 44px sous le libellé. */
  @media (max-width: 640px) {
    .rc-head { grid-template-columns: auto minmax(0, 1fr); justify-items: stretch; gap: 10px 12px; padding: 12px 14px; }
    .rc-big b { font-size: 32px; }
    .rc-big .eyebrow { margin-bottom: 2px; }
    .rc-counts { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 6px; width: auto; }
    .cnt { flex-direction: column; align-items: center; gap: 0; padding: 6px 4px; font-size: 12px; }
    .cnt b { font-size: 17px; }
    .rc-act { grid-column: 1 / -1; }
    .rc-act .btn { flex: 1; }
    .pt-st { flex-wrap: wrap; }
    .pt-sv { flex: 1 1 100%; display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 4px; padding: 2px; border-radius: 12px; background: var(--surface-2); }
    .sv { min-height: 44px; height: 44px; padding: 0 4px; border-radius: 10px; font-size: 14px; background: transparent; }
    .pt-log-b { margin-left: auto; }
  }
</style>
