<script lang="ts">
  // Recette (section 11.5) : liaisons, équipements posés et points libres, verdicts commentés,
  // journal par point, avancement, procès-verbal PDF. Écran partagé en direct.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { relative } from '../../lib/format';
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

  async function load() { try { r = await api.get(`/projects/${pid}/recette`); if (openLog) loadLog(openLog); } catch (e) { fail(e); } }
  $effect(() => { view; untrack(load); });
  async function loadLog(key: string) { try { logs = (await api.get<{ entries: typeof logs }>(`/projects/${pid}/recette/log?key=${encodeURIComponent(key)}`)).entries; } catch (e) { fail(e); } }

  async function verdict(p: Pt, status: St, comment?: string) {
    const c = comment ?? (p.comment && !Object.values(ST).some((s) => s.prefill && p.comment === s.prefill) ? p.comment : ST[status].prefill);
    try { await api.post(`/projects/${pid}/recette/verdict`, { key: p.key, status, comment: c }); await load(); } catch (e) { fail(e); }
  }
  async function addNote(key: string) { if (!note.trim()) return; try { await api.post(`/projects/${pid}/recette/comment`, { key, comment: note }); note = ''; await loadLog(key); } catch (e) { fail(e); } }
  async function addFree() { if (!newFree.trim()) return; try { await api.post(`/projects/${pid}/recette/free`, { label: newFree }); newFree = ''; await load(); } catch (e) { fail(e); } }
  async function renameFree(p: Pt, label: string) { try { await api.patch(`/projects/${pid}/recette/free`, { key: p.key, label }); await load(); } catch (e) { fail(e); } }
  async function deleteFree(p: Pt) { if (!confirm(`Retirer « ${p.label} » ?`)) return; try { await api.post(`/projects/${pid}/recette/free/delete`, { key: p.key }); await load(); } catch (e) { fail(e); } }
  async function moveFree(i: number, d: number) {
    if (!r) return; const keys = r.free.map((p) => p.key); const j = i + d; if (j < 0 || j >= keys.length) return;
    [keys[i], keys[j]] = [keys[j]!, keys[i]!];
    try { await api.post(`/projects/${pid}/recette/free/order`, { keys }); await load(); } catch (e) { fail(e); }
  }
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const keep = (p: Pt) => (!filter || p.status === filter) && (!q || norm(`${p.label} ${p.detail} ${p.comment ?? ''}`).includes(norm(q)));

  // ----- Procès-verbal PDF -----
  async function pv() {
    if (!r) return;
    const { jsPDF } = await import('jspdf');
    const site = (await api.get<{ name: string }>('/site')).name;
    const doc = new jsPDF({ unit: 'mm', format: paper, orientation: 'portrait' });
    const W = doc.internal.pageSize.getWidth(), H = doc.internal.pageSize.getHeight(), m = 14;
    let y = m;
    const room = (h: number) => { if (y + h > H - m - 8) { doc.addPage(); y = m; } };
    doc.setFont('helvetica', 'bold'); doc.setFontSize(9); doc.setTextColor(91, 99, 109); doc.text(site.toUpperCase(), m, y + 4);
    doc.setFontSize(20); doc.setTextColor(21, 23, 26); doc.text('Procès-verbal de recette', m, y + 14);
    doc.setFont('helvetica', 'normal'); doc.setFontSize(10.5);
    doc.text(`${view.project.name}${view.project.client ? ` · ${view.project.client}` : ''}`, m, y + 21);
    doc.text(`${view.project.venue ?? ''}${view.project.venue ? ' · ' : ''}édité le ${new Date().toLocaleDateString('fr-FR')}`, m, y + 26.5);
    y += 34;
    // Récapitulatif
    doc.setDrawColor(21, 23, 26); doc.setLineWidth(0.3); doc.rect(m, y, W - 2 * m, 16);
    const cells: [string, string][] = [['Avancement', `${r.progress} %`], ['Points', String(r.total)], ['OK', String(r.counts.ok)], ['Réserves', String(r.counts.reserve)], ['KO', String(r.counts.ko)], ['À tester', String(r.counts.to_test)]];
    const cw = (W - 2 * m) / cells.length;
    cells.forEach(([l, v], i) => { doc.setFontSize(7); doc.setTextColor(91, 99, 109); doc.text(l.toUpperCase(), m + i * cw + 3, y + 5); doc.setFont('helvetica', 'bold'); doc.setFontSize(13); doc.setTextColor(21, 23, 26); doc.text(v, m + i * cw + 3, y + 12.5); doc.setFont('helvetica', 'normal'); });
    y += 24;
    const color: Record<St, [number, number, number]> = { to_test: [91, 99, 109], ok: [14, 133, 80], reserve: [183, 132, 0], ko: [207, 58, 47] };
    // Les polices standard du PDF n'ont pas la flèche : « » » la remplace.
    const pdfText = (t: string) => t.replace(/→/g, '»').replace(/↔/g, '<>');
    const line = (p: Pt) => {
      const txt = doc.splitTextToSize(pdfText(`${p.label}${p.detail ? ` — ${p.detail}` : ''}`), W - 2 * m - 30);
      const com = p.comment ? doc.splitTextToSize(pdfText(p.comment), W - 2 * m - 30) : [];
      const h = 4.6 * (txt.length + com.length) + 2;
      room(h);
      doc.setFont('helvetica', 'bold'); doc.setFontSize(8); doc.setTextColor(...color[p.status]); doc.text(ST[p.status].label.toUpperCase(), m, y + 3.5);
      doc.setFont('helvetica', 'normal'); doc.setFontSize(9); doc.setTextColor(21, 23, 26); doc.text(txt, m + 26, y + 3.5);
      if (com.length) { doc.setTextColor(91, 99, 109); doc.text(com, m + 26, y + 3.5 + 4.6 * txt.length); }
      y += h; doc.setDrawColor(225, 228, 232); doc.line(m, y - 0.8, W - m, y - 0.8);
    };
    const section = (title: string, groups: Group[]) => {
      if (!groups.some((g) => g.points.length)) return;
      room(14); doc.setFont('helvetica', 'bold'); doc.setFontSize(13); doc.setTextColor(21, 23, 26); doc.text(title, m, y + 5); y += 9;
      for (const g of groups) { if (!g.points.length) continue; room(10); doc.setFontSize(9.5); doc.setFont('helvetica', 'bold'); doc.setTextColor(91, 99, 109); doc.text(g.title || 'Sans élément', m, y + 4); y += 6; for (const p of g.points) line(p); y += 2; }
    };
    section('Liaisons', r.cables);
    section('Équipements', r.equipment);
    section('Points libres', [{ title: '', points: r.free }]);
    section('Réserves à lever', [{ title: '', points: r.reserves }]);
    // Signatures, jamais coupées.
    room(46);
    y += 4;
    const bw = (W - 2 * m - 8) / 2;
    for (const [i, t] of ['Prestataire', 'Client'].entries()) {
      const x = m + i * (bw + 8);
      doc.setDrawColor(21, 23, 26); doc.rect(x, y, bw, 40);
      doc.setFont('helvetica', 'bold'); doc.setFontSize(9); doc.setTextColor(21, 23, 26); doc.text(t, x + 3, y + 6);
      doc.setFont('helvetica', 'normal'); doc.setFontSize(8); doc.setTextColor(91, 99, 109); doc.text('Nom, date et signature', x + 3, y + 11);
    }
    const n = doc.getNumberOfPages();
    for (let i = 1; i <= n; i++) { doc.setPage(i); doc.setFontSize(8); doc.setTextColor(91, 99, 109); doc.text(`${view.project.name} · procès-verbal de recette`, m, H - 7); doc.text(`${i} / ${n}`, W - m, H - 7, { align: 'right' }); }
    const name = view.project.name.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^A-Za-z0-9]+/g, '_');
    doc.save(`Recette_${name}_${new Date().toISOString().slice(0, 10)}.pdf`);
    toast('Procès-verbal exporté.');
  }
</script>

{#snippet row(p: Pt, free = false, i = 0)}
  <div class="pt" class:open={openLog === p.key}>
    <div class="pt-main">
      <div class="pt-t"><b>{p.label}</b>{#if p.detail}<span class="muted small">{p.detail}</span>{/if}</div>
      <div class="pt-st" role="group" aria-label="Verdict de {p.label}">
        {#each Object.entries(ST) as [k, s]}<button class="sv {k}" class:on={p.status === k} disabled={!writable || p.deleted} onclick={() => verdict(p, k as St)}>{s.label}</button>{/each}
      </div>
      <div class="pt-c">
        {#if p.status !== 'to_test'}<input class="input" value={p.comment ?? ''} disabled={!writable || p.deleted} maxlength="2000" onchange={(e) => verdict(p, p.status, (e.target as HTMLInputElement).value)} aria-label="Commentaire du verdict">{/if}
        {#if p.at}<span class="muted small">{p.by ?? ''} · {relative(p.at)}</span>{/if}
      </div>
      <div class="pt-a">
        {#if free && writable}<button class="icon-btn sm" disabled={i === 0} onclick={() => moveFree(i, -1)} aria-label="Monter">↑</button><button class="icon-btn sm" disabled={i === (r?.free.length ?? 1) - 1} onclick={() => moveFree(i, 1)} aria-label="Descendre">↓</button>
          <button class="icon-btn sm" onclick={() => { const l = prompt('Point libre', p.label); if (l) renameFree(p, l); }} aria-label="Renommer"><Icon name="pen" size="sm" /></button><button class="icon-btn sm" onclick={() => deleteFree(p)} aria-label="Retirer"><Icon name="trash" size="sm" /></button>{/if}
        <button class="btn sm ghost" onclick={() => { openLog = openLog === p.key ? null : p.key; if (openLog) loadLog(p.key); }}>Journal</button>
      </div>
    </div>
    {#if openLog === p.key}
      <div class="pt-log">
        {#each logs as l}<div class="lg"><span class="muted small">{new Date(l.at).toLocaleString('fr-FR')} · {l.by ?? '—'}</span>{#if l.status}<span class="chip">{ST[l.status as St]?.label}</span>{/if}<span>{l.comment ?? ''}</span></div>{:else}<span class="muted small">Aucune entrée.</span>{/each}
        {#if writable && !p.deleted}<div class="lg-add"><input class="input" bind:value={note} maxlength="2000" placeholder="Ajouter un commentaire au journal" onkeydown={(e) => { if (e.key === 'Enter') addNote(p.key); }}><button class="btn sm" onclick={() => addNote(p.key)}>Ajouter</button></div>{/if}
      </div>
    {/if}
  </div>
{/snippet}

{#if !r}<span class="spin"></span>
{:else}
  <div class="rc-head card">
    <div class="rc-big"><span class="eyebrow">Recette</span><b class="mono">{r.progress} %</b><span class="muted small">{r.total - r.counts.to_test} point{r.total - r.counts.to_test > 1 ? 's' : ''} testé{r.total - r.counts.to_test > 1 ? 's' : ''} sur {r.total}</span></div>
    <div class="rc-counts">
      {#each Object.entries(ST) as [k, s]}<button class="cnt {k}" class:on={filter === k} onclick={() => (filter = filter === k ? '' : (k as St))}><b class="mono">{r.counts[k as St]}</b>{s.label}</button>{/each}
    </div>
    <div class="rc-act">
      <select class="input" bind:value={paper} aria-label="Papier"><option value="a4">A4</option><option value="a3">A3</option></select>
      <button class="btn primary" onclick={pv}><Icon name="download" size="sm" />Procès-verbal PDF</button>
    </div>
  </div>
  <div class="toolbar" style="margin:12px 0"><label class="search" style="flex:1"><Icon name="search" /><input class="input" bind:value={q} placeholder="Rechercher un point…" aria-label="Rechercher"></label></div>

  {#each [['Liaisons', r.cables], ['Équipements', r.equipment]] as [title, groups]}
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
    {#if writable}<div class="lg-add"><input class="input" bind:value={newFree} maxlength="200" placeholder="Nouveau point : essai de bascule secours, lecture timecode…" onkeydown={(e) => { if (e.key === 'Enter') addFree(); }}><button class="btn sm" onclick={addFree}><Icon name="plus" size="sm" />Ajouter</button></div>{/if}
  </section>
  {#if r.deleted.length}
    <button class="btn sm ghost" style="margin-top:12px" onclick={() => (showDeleted = !showDeleted)}>{r.deleted.length} verdict(s) d’éléments supprimés</button>
    {#if showDeleted}<section class="card rc-g">{#each r.deleted as p (p.key)}{@render row(p)}{/each}</section>{/if}
  {/if}
{/if}

<style>
  .rc-head { padding: 18px 20px; display: grid; grid-template-columns: auto minmax(0, 1fr) auto; gap: 20px; align-items: center; }
  .rc-big { display: flex; flex-direction: column; }
  .rc-big b { font-size: 44px; line-height: 1; letter-spacing: -.02em; }
  .rc-counts { display: flex; gap: 8px; flex-wrap: wrap; }
  .cnt { display: flex; gap: 8px; align-items: baseline; padding: 8px 12px; border-radius: 12px; border: 1.5px solid transparent; background: var(--surface-2); cursor: pointer; font: inherit; color: inherit; }
  .cnt.on { border-color: var(--fg); }
  .cnt b { font-size: 18px; }
  .cnt.ok b { color: var(--ok, #0E8550); } .cnt.reserve b { color: var(--warn, #B78400); } .cnt.ko b { color: var(--err); }
  .rc-act { display: flex; gap: 8px; align-items: center; }
  .rc-act select { width: auto; height: 40px; }
  .rc-h { font-size: 22px; margin: 22px 0 10px; }
  .rc-g { padding: 12px 14px; display: grid; gap: 4px; margin-bottom: 10px; }
  .pt { border-top: 1px solid var(--line); padding: 8px 0; }
  .pt:first-of-type { border-top: 0; }
  .pt-main { display: grid; grid-template-columns: minmax(200px, 1.3fr) auto minmax(180px, 1fr) auto; gap: 12px; align-items: center; }
  .pt-t { display: flex; flex-direction: column; min-width: 0; }
  .pt-st { display: flex; gap: 4px; }
  .sv { padding: 5px 10px; border-radius: 99px; border: 1.5px solid var(--line); background: var(--surface); cursor: pointer; font: 600 12.5px var(--font); color: var(--fg-2); }
  .sv.on.ok { background: var(--ok, #0E8550); border-color: var(--ok, #0E8550); color: #fff; }
  .sv.on.reserve { background: var(--warn, #B78400); border-color: var(--warn, #B78400); color: #fff; }
  .sv.on.ko { background: var(--err); border-color: var(--err); color: #fff; }
  .sv.on.to_test { background: var(--fg); border-color: var(--fg); color: var(--bg); }
  .pt-c { display: flex; flex-direction: column; gap: 2px; }
  .pt-c .input { height: 34px; font-size: 14px; }
  .pt-a { display: flex; gap: 4px; align-items: center; }
  .pt-log { margin: 8px 0 4px; padding: 10px; border-radius: 12px; background: var(--surface-2); display: grid; gap: 6px; }
  .lg { display: flex; gap: 8px; align-items: baseline; flex-wrap: wrap; font-size: 14px; }
  .lg-add { display: flex; gap: 8px; margin-top: 6px; }
  .lg-add .input { flex: 1; height: 38px; }
  @media (max-width: 980px) { .rc-head { grid-template-columns: 1fr; } .pt-main { grid-template-columns: 1fr; } }
</style>
