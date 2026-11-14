<script lang="ts">
  import { ask } from '../../lib/ask.svelte';
  // Dessins et plan de feu (section 11.2) : schéma de principe, implantations vidéo et audio,
  // plan de feu. Éléments posés en mm de page ; équipements tirés de la nomenclature (un
  // exemplaire une fois par nature) ; liens ; adressage DMX. Enregistrement automatique.
  import { untrack, onDestroy } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { join, sendCursor, sendSelection, type PresenceMember } from '../../lib/live.svelte';
  import RemoteCursors from '../../components/RemoteCursors.svelte';
  import { relative } from '../../lib/format';
  import { loadRefs, refList, refName, fnColor } from '../../lib/catalog.svelte';
  import { withLightTheme, standalone, pdf as toPdf, save as saveFile } from '../../lib/svgexport';
  import type { Paper } from '../../lib/sheet';
  import type { ProjectView } from '../../lib/projects';

  type Nature = 'schematic' | 'video' | 'audio' | 'lighting';
  interface PageSum { id: string; nature: Nature; name: string; position: number; rev: number; count: number; updated_at: string }
  interface Snap { item_id: string | null; name: string; reference: string | null; category: string | null; tag: string; function: string | null; images: string[]; dmx_channels: number | null; dmx_modes: [string, number][] }
  interface Bom { instance_id: string; functions: string[]; snapshot: Snap; placed: { nature: Nature; page: string; element: string }[] }
  interface Patch { console: number; universe: number; address: number; mode: string | null }
  interface Index { pages: PageSum[]; bom: Bom[]; dmx: { fixtures: { key: string; item_id: string | null; patch: Patch | null; channels: number }[]; conflicts: [string, string][] } }
  interface Anchor { element: string | null; x: number; y: number }
  type El = { id: string; type: 'text' | 'rect' | 'ellipse' | 'line' | 'image' | 'zone' | 'link' | 'equipment'; [k: string]: any };
  interface PageData { name: string; paper: Paper; landscape: boolean; grid_mm: number; snap: boolean; background: string | null; scale: number | null; legend: boolean; dmx_display: 'address' | 'channel' | 'number'; elements: El[] }
  interface Page extends PageData { id: string; nature: Nature; rev: number; lock: { account_name: string; taken_at: string; mine: boolean } | null }

  let { view }: { view: ProjectView } = $props();
  const pid = $derived(view.project.id);
  const canDesign = $derived(can('conception', 'ecriture'));
  const NATURES: [Nature, string, string][] = [['schematic', 'Schémas de principe', 'Schéma'], ['video', 'Implantations vidéo', 'Vidéo'], ['audio', 'Implantations audio', 'Audio'], ['lighting', 'Plans de feu', 'Plan de feu']];
  const NFUN: Record<Nature, string[]> = { schematic: [], video: ['V'], audio: ['A'], lighting: ['L'] };
  const SIZES: Record<string, [number, number]> = { A4: [210, 297], A3: [297, 420], A2: [420, 594], A1: [594, 841], A0: [841, 1189] };

  let idx = $state<Index | null>(null);
  let pageId = $state<string | null>(null);
  let pg = $state<Page | null>(null);
  const mine = $derived(!!pg?.lock?.mine);

  async function loadIndex() { try { idx = await api.get(`/projects/${pid}/drawings`); if (idx && (!pageId || !idx.pages.some((p) => p.id === pageId)) && idx.pages[0]) await openPage(idx.pages[0].id); } catch (e) { fail(e); } }
  async function loadPage() { if (!pageId) { pg = null; return; } try { pg = await api.get(`/projects/${pid}/drawings/${pageId}`); dirty = false; } catch (e) { fail(e); } }
  async function openPage(id: string) {
    if (pageId === id) return;
    await flush(); await release();
    pageId = id; sel = new Set(); tool = 'select'; placing = null;
    await loadPage();
    if (canDesign) await take();
  }
  $effect(() => { view; untrack(() => { loadRefs(); if (!idx) loadIndex(); }); });

  // ----- Main -----
  let beat: ReturnType<typeof setInterval> | null = null;
  let lastAction = Date.now();
  let requested = $state<string | null>(null);
  async function take() { if (!canDesign || !pageId) return; try { const r = await api.post<{ mine: boolean }>(`/projects/${pid}/drawings/${pageId}/lock`); if (r.mine && !beat) beat = setInterval(heartbeat, 20_000); await loadPage(); } catch (e) { fail(e); } }
  async function heartbeat() {
    if (Date.now() - lastAction > 10 * 60_000) { await flush(); await release(); await loadPage(); toast('Main cédée après 10 minutes sans action.', 'info'); return; }
    try { const r = await api.post<{ mine: boolean }>(`/projects/${pid}/drawings/${pageId}/lock`); if (!r.mine) { stopBeat(); await loadPage(); } } catch { /* prochain battement */ }
  }
  function stopBeat() { if (beat) { clearInterval(beat); beat = null; } }
  async function release() { stopBeat(); if (!pageId || !pg?.lock?.mine) return; try { await api.del(`/projects/${pid}/drawings/${pageId}/lock`); } catch { /* déjà libre */ } }
  $effect(() => join(`project:${pid}`, (c) => {
    if (c.kind === 'lock' && pageId && c.id === `draw:${pageId}`) {
      if (c.action === 'request' && mine) requested = c.actor;
      if (c.action === 'taken') { if (mine) stopBeat(); loadPage(); }
      if (c.action === 'released' && !mine) loadPage();
      return;
    }
    if (c.kind === 'drawing') { if (!mine || c.id !== pageId) { loadIndex(); if (c.id === pageId || c.id === 'dmx') loadPage(); } }
  }));
  onDestroy(() => { stopBeat(); if (saveT) { clearTimeout(saveT); } if (dirty && pg && mine) { void persist(); } if (pageId && pg?.lock?.mine) fetch(`/api/projects/${pid}/drawings/${pageId}/lock`, { method: 'DELETE', headers: { 'X-Strata-Request': '1' }, keepalive: true }); });

  // ----- Enregistrement automatique -----
  let dirty = $state(false), saving = $state(false);
  let saveT: ReturnType<typeof setTimeout> | null = null;
  // Annuler / rétablir sur 50 états : on garde l'état validé avant chaque modification.
  let committed = '';
  let undoStack = $state<string[]>([]), redoStack = $state<string[]>([]);
  $effect(() => { pg?.id; untrack(() => { committed = pg ? JSON.stringify($state.snapshot(pg.elements)) : ''; undoStack = []; redoStack = []; }); });
  function changed(record = true) {
    if (!mine || !pg) return;
    const now = JSON.stringify($state.snapshot(pg.elements));
    if (record && now !== committed) { undoStack = [...undoStack.slice(-49), committed]; redoStack = []; }
    committed = now;
    dirty = true; lastAction = Date.now(); if (saveT) clearTimeout(saveT); saveT = setTimeout(persist, 700);
  }
  function undo() { if (!pg || !undoStack.length) return; redoStack = [...redoStack, JSON.stringify($state.snapshot(pg.elements))]; pg.elements = JSON.parse(undoStack.at(-1)!); undoStack = undoStack.slice(0, -1); sel = new Set(); changed(false); }
  function redo() { if (!pg || !redoStack.length) return; undoStack = [...undoStack, JSON.stringify($state.snapshot(pg.elements))]; pg.elements = JSON.parse(redoStack.at(-1)!); redoStack = redoStack.slice(0, -1); sel = new Set(); changed(false); }
  // Presse-papiers interne : les blocs équipement (uniques) et les liens ne se copient pas.
  let clip: El[] = [];
  function copySel() { clip = selList.filter((e) => e.type !== 'equipment' && e.type !== 'link').map((e) => structuredClone($state.snapshot(e)) as El); if (clip.length) toast(`${clip.length} élément${clip.length > 1 ? 's' : ''} copié${clip.length > 1 ? 's' : ''}.`, 'info'); }
  function paste() {
    if (!pg || !clip.length) return;
    const copies = clip.map((e) => { const c = structuredClone(e); c.id = uid(); if (c.type === 'line') c.points = c.points.map((q: number[]) => [q[0]! + 5, q[1]! + 5]); else { c.x += 5; c.y += 5; } return c; });
    clip = copies.map((c) => structuredClone(c));
    pg.elements = [...pg.elements, ...copies]; sel = new Set(copies.map((c) => c.id)); changed();
  }
  // Enregistrement : en cas d'échec, le dessin local est gardé (rien n'est effacé) et on
  // réessaie ; en cas de conflit avec une autre modification, on laisse choisir.
  let failed = $state(false), conflict = $state(false);
  let retryT: ReturnType<typeof setTimeout> | null = null;
  async function persist() {
    if (!pg || !dirty) return;
    saveT = null; saving = true; dirty = false;
    const { id, nature, rev, lock, ...data } = pg; void nature; void lock;
    try { const r = await api.put<{ rev: number }>(`/projects/${pid}/drawings/${id}`, { rev, ...data }); if (pg && pg.id === id) pg.rev = r.rev; failed = false; refreshIndex(); }
    catch (e) {
      dirty = true;
      if (e instanceof ApiError && e.code === 'stale') conflict = true;
      else { failed = true; if (retryT) clearTimeout(retryT); retryT = setTimeout(() => { retryT = null; if (failed && dirty) persist(); }, 5000); }
    } finally { saving = false; }
  }
  /** Conflit : garder ma version (écrase l'autre) ou reprendre la version enregistrée. */
  async function keepMine() {
    if (!pg) return;
    try { const cur = await api.get<{ rev: number }>(`/projects/${pid}/drawings/${pg.id}`); pg.rev = cur.rev; conflict = false; dirty = true; await persist(); } catch (e) { fail(e); }
  }
  async function takeTheirs() { conflict = false; dirty = false; failed = false; await loadPage(); }
  async function flush() { if (saveT) { clearTimeout(saveT); saveT = null; } if (dirty) await persist(); }
  async function refreshIndex() { try { idx = await api.get(`/projects/${pid}/drawings`); } catch { /* plus tard */ } }

  // ----- Pages -----
  async function addPage(n: Nature) { try { const r = await api.post<{ id: string }>(`/projects/${pid}/drawings`, { nature: n }); await refreshIndex(); await openPage(r.id); } catch (e) { fail(e); } }
  async function delPage() { if (!pg || !(await ask({ title: `Supprimer la page « ${pg.name} » ?`, confirmLabel: 'Supprimer', danger: true }))) return; try { await api.post(`/projects/${pid}/drawings/${pg.id}/delete`); stopBeat(); pageId = null; pg = null; await loadIndex(); } catch (e) { fail(e); } }
  async function movePage(p: PageSum, d: number) {
    const list = idx!.pages.filter((x) => x.nature === p.nature).map((x) => x.id); const i = list.indexOf(p.id), j = i + d; if (j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j]!, list[i]!];
    try { await api.post(`/projects/${pid}/drawings/order`, { ids: list }); await refreshIndex(); } catch (e) { fail(e); }
  }
  async function sync() {
    await flush();
    try { const r = await api.post<{ updated: number; removed: number }>(`/projects/${pid}/drawings/${pageId}/sync`); toast(`Synchronisé : ${r.updated} mis à jour, ${r.removed} retiré${r.removed > 1 ? 's' : ''}.`); await loadPage(); await refreshIndex(); } catch (e) { fail(e); }
  }

  // ----- Géométrie -----
  const size = $derived.by((): [number, number] => { const s = SIZES[pg?.paper ?? 'A3'] ?? [297, 420]; return pg?.landscape ? [s[1], s[0]] : [s[0], s[1]]; });
  const byId = $derived(new Map((pg?.elements ?? []).map((e) => [e.id, e])));
  const center = (e: El): [number, number] => (e.type === 'line' ? [e.points[0][0], e.points[0][1]] : e.type === 'text' ? [e.x, e.y] : [e.x + e.w / 2, e.y + e.h / 2]);
  const anchorPt = (a: Anchor): [number, number] => { const e = a.element ? byId.get(a.element) : null; return e ? center(e) : [a.x, a.y]; };
  /** Bout d'un lien posé sur le bord de l'élément, vers le point `toward`. */
  function edge(a: Anchor, toward: [number, number]): [number, number] {
    const c = anchorPt(a), e = a.element ? byId.get(a.element) : null;
    if (!e || e.type === 'line' || e.type === 'text' || !('w' in e)) return c;
    const dx = toward[0] - c[0], dy = toward[1] - c[1];
    if (!dx && !dy) return c;
    const s = Math.min(dx ? e.w / 2 / Math.abs(dx) : Infinity, dy ? e.h / 2 / Math.abs(dy) : Infinity);
    return s >= 1 ? c : [c[0] + dx * s, c[1] + dy * s];
  }
  /** Points de construction d'un lien : départ, arrivée, et points de contrôle selon le tracé. */
  function geo(l: El): { a: [number, number]; b: [number, number]; mx: number; dx: number } {
    const ca = anchorPt(l.from), cb = anchorPt(l.to);
    if (l.style === 'orthogonal') {
      const mx = (ca[0] + cb[0]) / 2;
      return { a: edge(l.from, [mx, ca[1]]), b: edge(l.to, [mx, cb[1]]), mx, dx: 0 };
    }
    if (l.style === 'curve') {
      const dx = Math.max(20, Math.abs(cb[0] - ca[0]) / 2);
      return { a: edge(l.from, [ca[0] + dx, ca[1]]), b: edge(l.to, [cb[0] - dx, cb[1]]), mx: 0, dx };
    }
    return { a: edge(l.from, cb), b: edge(l.to, ca), mx: 0, dx: 0 };
  }
  function linkPath(l: El): string {
    const g = geo(l), [x1, y1] = g.a, [x2, y2] = g.b;
    if (l.style === 'orthogonal') return `M${x1} ${y1} H${g.mx} V${y2} H${x2}`;
    if (l.style === 'curve') return `M${x1} ${y1} C${x1 + g.dx} ${y1} ${x2 - g.dx} ${y2} ${x2} ${y2}`;
    return `M${x1} ${y1} L${x2} ${y2}`;
  }
  /** Pointe de flèche au bout (x2, y2), venant de (x1, y1). */
  function arrow(x1: number, y1: number, x2: number, y2: number, w: number): string {
    const a = Math.atan2(y2 - y1, x2 - x1), s = 2.2 + w * 2;
    const p = (da: number) => `${x2 - s * Math.cos(a + da)},${y2 - s * Math.sin(a + da)}`;
    return `${x2},${y2} ${p(0.4)} ${p(-0.4)}`;
  }
  function linkEnds(l: El): { s: [number, number, number, number]; e: [number, number, number, number] } {
    const g = geo(l), [x1, y1] = g.a, [x2, y2] = g.b;
    if (l.style === 'orthogonal') return { s: [g.mx, y1, x1, y1], e: [g.mx, y2, x2, y2] };
    if (l.style === 'curve') return { s: [x1 + g.dx, y1, x1, y1], e: [x2 - g.dx, y2, x2, y2] };
    return { s: [x2, y2, x1, y1], e: [x1, y1, x2, y2] };
  }
  const linkColor = (l: El) => l.color ?? (l.signal_id ? sigColor(l.signal_id) : '#15171A');
  function sigColor(id: string) { const s = refList('signals').find((x) => x.id === id) as any; return s?.data?.color || fnColor(s?.data?.function) || '#15171A'; }

  // DMX
  const conflictKeys = $derived(new Set((idx?.dmx.conflicts ?? []).flat()));
  const chans = (e: El) => { const m = e.dmx?.mode; return (m && e.snapshot.dmx_modes.find((x: [string, number]) => x[0] === m)?.[1]) || e.snapshot.dmx_channels || 1; };
  function dmxText(e: El): string {
    const d: Patch | null = e.dmx; if (!d || !d.address) return 'DMX —';
    if (pg?.dmx_display === 'channel') return `Ch ${(d.universe - 1) * 512 + d.address}`;
    if (pg?.dmx_display === 'number') return `#${d.console}`;
    return `U${d.universe}.${String(d.address).padStart(3, '0')}`;
  }
  function lines(e: El): string[] {
    const s: Snap = e.snapshot, o: string[] = [];
    for (const k of e.show as string[]) {
      if (k === 'label') o.push(e.label ?? s.name);
      if (k === 'name') o.push(s.name);
      if (k === 'reference' && s.reference) o.push(s.reference);
      if (k === 'tag') o.push(s.tag);
      if (k === 'category' && s.category) o.push(s.category);
      if (k === 'dmx' && pg?.nature === 'lighting') o.push(dmxText(e));
    }
    return o;
  }

  // ----- Vue et interactions -----
  let svgEl = $state<SVGSVGElement>(null!);
  let wrapW = $state(900), wrapH = $state(640);
  let zoom = $state(1), panX = $state(0), panY = $state(0);
  const fitK = $derived(Math.min((wrapW - 40) / size[0], (wrapH - 40) / size[1]));
  const k = $derived(fitK * zoom);
  const ox = $derived((wrapW - size[0] * k) / 2 + panX), oy = $derived((wrapH - size[1] * k) / 2 + panY);
  const toPage = (e: { clientX: number; clientY: number }): [number, number] => { const r = svgEl.getBoundingClientRect(); return [(e.clientX - r.left - ox) / k, (e.clientY - r.top - oy) / k]; };
  const snapV = (v: number) => (pg?.snap && pg.grid_mm > 0 ? Math.round(v / pg.grid_mm) * pg.grid_mm : Math.round(v * 10) / 10);
  function wheel(e: WheelEvent) {
    e.preventDefault();
    const r = svgEl.getBoundingClientRect(), mx = e.clientX - r.left, my = e.clientY - r.top;
    const nz = Math.min(8, Math.max(0.3, zoom * (e.deltaY < 0 ? 1.12 : 1 / 1.12)));
    const before = toPage(e);
    zoom = nz;
    // Garder le point sous la souris.
    const k2 = fitK * nz, ox2 = (wrapW - size[0] * k2) / 2, oy2 = (wrapH - size[1] * k2) / 2;
    panX = mx - ox2 - before[0] * k2; panY = my - oy2 - before[1] * k2;
  }

  type Tool = 'select' | 'text' | 'rect' | 'ellipse' | 'line' | 'zone' | 'link' | 'image';
  let tool = $state<Tool>('select');
  let sel = $state(new Set<string>());
  let placing = $state<string | null>(null);
  let draft = $state<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  let lineDraft = $state<[number, number][]>([]);
  let linkFrom = $state<Anchor | null>(null);
  let hover = $state<[number, number] | null>(null);
  let drag: { ids: string[]; x0: number; y0: number; orig: Map<string, El>; resize?: boolean } | null = null;
  let pan: { x: number; y: number; px: number; py: number } | null = null;
  let remoteSel = $state<Record<string, { member: PresenceMember; ids: string[] }>>({});
  const rsel = $derived(new Map(Object.values(remoteSel).flatMap((r) => r.ids.map((id) => [id, r.member.color] as const))));
  $effect(() => { const ids = [...sel]; if (pageId) sendSelection(`project:${pid}/draw:${pageId}`, ids); });
  const selList = $derived([...sel].map((id) => byId.get(id)).filter(Boolean) as El[]);
  const uid = () => Math.random().toString(36).slice(2, 10);

  function setTool(t: Tool) { tool = t; draft = null; lineDraft = []; linkFrom = null; if (t !== 'select') placing = null; }
  function hit(e: Event): El | null { const g = (e.target as Element).closest('[data-el]'); return g ? byId.get(g.getAttribute('data-el')!) ?? null : null; }

  async function down(e: PointerEvent) {
    if (e.button !== 0 || !pg) return;
    const p = toPage(e), h = hit(e);
    if (!mine || (tool === 'select' && !placing)) {
      if (h && mine) {
        if (e.shiftKey) { const s = new Set(sel); if (s.has(h.id)) s.delete(h.id); else s.add(h.id); sel = s; }
        else if (!sel.has(h.id)) sel = new Set([h.id]);
        const resize = (e.target as Element).classList.contains('rz');
        drag = { ids: [...sel], x0: p[0], y0: p[1], orig: new Map([...sel].map((id) => [id, structuredClone($state.snapshot(byId.get(id)!))])), resize };
        svgEl.setPointerCapture(e.pointerId);
      } else if (h) { sel = new Set([h.id]); }
      else { if (!e.shiftKey) sel = new Set(); pan = { x: e.clientX, y: e.clientY, px: panX, py: panY }; svgEl.setPointerCapture(e.pointerId); }
      return;
    }
    const sp: [number, number] = [snapV(p[0]), snapV(p[1])];
    if (placing) { await placeEquipment(placing, sp); return; }
    if (tool === 'text') {
      const t = prompt('Texte :'); setTool('select');
      if (t?.trim()) add({ id: uid(), type: 'text', x: sp[0], y: sp[1], w: 0, rot: 0, text: t, size: 4, color: null, bold: false });
    } else if (tool === 'rect' || tool === 'ellipse' || tool === 'zone' || tool === 'image') {
      draft = { x0: sp[0], y0: sp[1], x1: sp[0], y1: sp[1] }; svgEl.setPointerCapture(e.pointerId);
    } else if (tool === 'line') {
      lineDraft = [...lineDraft, sp];
    } else if (tool === 'link') {
      const a: Anchor = h && h.type !== 'link' ? { element: h.id, x: 0, y: 0 } : { element: null, x: sp[0], y: sp[1] };
      if (!linkFrom) linkFrom = a;
      else { add({ id: uid(), type: 'link', from: linkFrom, to: a, style: 'straight', arrow_start: false, arrow_end: true, label: null, color: null, width: 0.5, signal_id: null }); linkFrom = null; }
    }
  }
  function move(e: PointerEvent) {
    const p = toPage(e); hover = p;
    if (pageId) sendCursor(`project:${pid}/draw:${pageId}`, p[0], p[1]);
    if (pan) { panX = pan.px + e.clientX - pan.x; panY = pan.py + e.clientY - pan.y; return; }
    if (draft) { draft = { ...draft, x1: snapV(p[0]), y1: snapV(p[1]) }; return; }
    if (drag && pg) {
      const dx = snapV(p[0] - drag.x0), dy = snapV(p[1] - drag.y0);
      pg.elements = pg.elements.map((el) => {
        const o = drag!.orig.get(el.id); if (!o) return el;
        if (drag!.resize && 'w' in o && o.type !== 'text') return { ...el, w: Math.max(2, o.w + dx), h: Math.max(2, o.h + dy) };
        if (o.type === 'line') return { ...el, points: o.points.map((q: number[]) => [q[0]! + dx, q[1]! + dy]) };
        if (o.type === 'link') return { ...el, from: o.from.element ? o.from : { ...o.from, x: o.from.x + dx, y: o.from.y + dy }, to: o.to.element ? o.to : { ...o.to, x: o.to.x + dx, y: o.to.y + dy } };
        return { ...el, x: o.x + dx, y: o.y + dy };
      });
    }
  }
  async function up() {
    pan = null;
    if (drag) { const moved = drag.ids.some((id) => JSON.stringify(byId.get(id)) !== JSON.stringify(drag!.orig.get(id))); drag = null; if (moved) changed(); }
    if (draft) {
      const d = draft; draft = null;
      const x = Math.min(d.x0, d.x1), y = Math.min(d.y0, d.y1), w = Math.abs(d.x1 - d.x0) || 30, h = Math.abs(d.y1 - d.y0) || 20;
      if (tool === 'rect') add({ id: uid(), type: 'rect', x, y, w, h, rot: 0, stroke: '#15171A', fill: null, width: 0.5, radius: 0 });
      if (tool === 'ellipse') add({ id: uid(), type: 'ellipse', x, y, w, h, rot: 0, stroke: '#15171A', fill: null, width: 0.5 });
      if (tool === 'zone') { const label = prompt('Nom de la zone :') ?? ''; add({ id: uid(), type: 'zone', x, y, w, h, label, color: '#3A7BD5' }); }
      if (tool === 'image') pickImage({ x, y, w, h });
      setTool('select');
    }
  }
  function finishLine() { if (lineDraft.length >= 2) add({ id: uid(), type: 'line', points: lineDraft, stroke: '#15171A', width: 0.5, dash: false, arrow_start: false, arrow_end: false }); lineDraft = []; setTool('select'); }
  function add(el: El) { if (!pg) return; pg.elements = [...pg.elements, el]; sel = new Set([el.id]); side = 'props'; changed(); }
  function patchSel(p: Record<string, unknown>) { if (!pg) return; pg.elements = pg.elements.map((e) => (sel.has(e.id) ? { ...e, ...p } : e)); changed(); }
  function removeSel() {
    if (!pg || !sel.size) return;
    const gone = sel, centers = new Map([...gone].map((id) => [id, byId.get(id) ? center(byId.get(id)!) : [0, 0]]));
    pg.elements = pg.elements.filter((e) => !gone.has(e.id)).map((e) => {
      if (e.type !== 'link') return e;
      const fix = (a: Anchor) => (a.element && gone.has(a.element) ? { element: null, x: centers.get(a.element)![0]!, y: centers.get(a.element)![1]! } : a);
      return { ...e, from: fix(e.from), to: fix(e.to) };
    });
    sel = new Set(); changed();
  }
  function duplicateSel() {
    if (!pg) return;
    const copies = selList.filter((e) => e.type !== 'equipment' && e.type !== 'link').map((e) => { const c = structuredClone($state.snapshot(e)) as El; c.id = uid(); if (c.type === 'line') c.points = c.points.map((q: number[]) => [q[0]! + 5, q[1]! + 5]); else { c.x += 5; c.y += 5; } return c; });
    if (!copies.length) return; pg.elements = [...pg.elements, ...copies]; sel = new Set(copies.map((c) => c.id)); changed();
  }
  function key(e: KeyboardEvent) {
    if ((e.target as HTMLElement)?.closest('input,textarea,select')) return;
    if (e.key === 'Escape') { setTool('select'); sel = new Set(); }
    if (e.key === 'Enter' && tool === 'line') finishLine();
    if (!mine) return;
    if (e.key === 'Delete' || e.key === 'Backspace') { e.preventDefault(); removeSel(); }
    if ((e.ctrlKey || e.metaKey) && e.key === 'd') { e.preventDefault(); duplicateSel(); }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z') { e.preventDefault(); if (e.shiftKey) redo(); else undo(); }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y') { e.preventDefault(); redo(); }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c') { copySel(); }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'x') { copySel(); removeSel(); }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'v') { e.preventDefault(); paste(); }
    if (e.key.startsWith('Arrow') && sel.size) {
      e.preventDefault(); const st = e.shiftKey ? 10 : (pg?.grid_mm || 1), dx = e.key === 'ArrowLeft' ? -st : e.key === 'ArrowRight' ? st : 0, dy = e.key === 'ArrowUp' ? -st : e.key === 'ArrowDown' ? st : 0;
      pg!.elements = pg!.elements.map((el) => (!sel.has(el.id) ? el : el.type === 'line' ? { ...el, points: el.points.map((q: number[]) => [q[0]! + dx, q[1]! + dy]) } : el.type === 'link' ? el : { ...el, x: el.x + dx, y: el.y + dy }));
      changed();
    }
  }

  // ----- Équipements -----
  let side = $state<'place' | 'props' | 'page' | 'dmx'>('place');
  let q = $state('');
  const fitsNature = (b: Bom) => !NFUN[pg?.nature ?? 'schematic'].length || b.functions.some((f) => NFUN[pg!.nature].includes(f));
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const toPlace = $derived((idx?.bom ?? []).filter((b) => fitsNature(b) && !b.placed.some((p) => p.nature === pg?.nature) && !pg?.elements.some((e) => e.type === 'equipment' && e.instance_id === b.instance_id) && (!q || norm(`${b.snapshot.name} ${b.snapshot.tag} ${b.snapshot.reference ?? ''}`).includes(norm(q)))));
  const placedHere = $derived((pg?.elements ?? []).filter((e) => e.type === 'equipment'));
  const placedElsewhere = $derived((idx?.bom ?? []).filter((b) => b.placed.some((p) => p.nature === pg?.nature && p.page !== pg?.id)));
  async function placeEquipment(inst: string, at: [number, number]) {
    const b = idx?.bom.find((x) => x.instance_id === inst); if (!b || !pg) return;
    let dmx: Patch | null = null;
    if (pg.nature === 'lighting' && (b.snapshot.dmx_channels || b.snapshot.dmx_modes.length)) {
      // Proposition : tient compte des projecteurs déjà posés ailleurs et sur cette page.
      await flush();
      try { dmx = await api.post<Patch>(`/projects/${pid}/drawings/dmx/suggest`, { instance_id: inst }); } catch { dmx = null; }
      const here = pg.elements.filter((e) => e.type === 'equipment' && e.dmx?.address);
      if (dmx && here.some((e) => e.dmx.universe === dmx!.universe && e.dmx.address <= dmx!.address + (b.snapshot.dmx_channels ?? 1) - 1 && dmx!.address <= e.dmx.address + chans(e) - 1)) dmx = null;
    }
    const w = pg.nature === 'schematic' ? 40 : 24, h = pg.nature === 'schematic' ? 18 : 20;
    add({ id: uid(), type: 'equipment', instance_id: inst, x: at[0] - w / 2, y: at[1] - h / 2, w, h, rot: 0, label: null, image: 'pictogram', show: pg.nature === 'lighting' ? ['name', 'dmx'] : ['name', 'tag'], frame: null, snapshot: b.snapshot, dmx });
    placing = null; tool = 'select'; refreshIndex();
  }

  // ----- Images -----
  let imgInput = $state<HTMLInputElement>(null!);
  let imgBox: { x: number; y: number; w: number; h: number } | null = null;
  function pickImage(b: { x: number; y: number; w: number; h: number }) { imgBox = b; imgInput.click(); }
  async function onImage(f: File | undefined) {
    if (!f || !imgBox) return;
    try {
      const r = await api.upload<{ file: string }>(`/projects/${pid}/drawings/files`, f, f.type);
      add({ id: uid(), type: 'image', ...imgBox, rot: 0, file: r.file });
    } catch (e) { fail(e); }
    imgBox = null;
  }

  // ----- DMX : adressage d'ensemble -----
  async function dmxAuto(mode: 'missing' | 'all') {
    if (mode === 'all' && !(await ask({ title: 'Réadresser tous les projecteurs ?', text: 'Les adresses DMX de tous les projecteurs de tous les plans de feu sont recalculées.', confirmLabel: 'Réadresser' }))) return;
    await flush();
    try { const r = await api.post<{ assigned: number }>(`/projects/${pid}/drawings/dmx/auto`, { mode }); toast(`${r.assigned} projecteur${r.assigned > 1 ? 's' : ''} adressé${r.assigned > 1 ? 's' : ''}.`); await loadPage(); await refreshIndex(); } catch (e) { fail(e); }
  }
  async function suggestFor(e: El) { try { const p = await api.post<Patch>(`/projects/${pid}/drawings/dmx/suggest`, { instance_id: e.instance_id, mode: e.dmx?.mode ?? null }); patchSel({ dmx: { ...p, mode: e.dmx?.mode ?? null } }); } catch (x) { fail(x); } }

  // ----- Légende -----
  const legend = $derived.by(() => {
    if (!pg) return [] as [string, string][];
    const m = new Map<string, string>();
    for (const e of pg.elements) if (e.type === 'link' && e.signal_id) m.set(refName('signals', e.signal_id), linkColor(e));
    for (const e of pg.elements) if (e.type === 'zone' && e.label) m.set(e.label, e.color ?? '#3A7BD5');
    if (pg.nature === 'lighting') {
      const c = new Map<string, number>();
      for (const e of pg.elements) if (e.type === 'equipment') c.set(e.snapshot.name, (c.get(e.snapshot.name) ?? 0) + 1);
      for (const [n, v] of c) m.set(`${v} × ${n}`, '');
    }
    return [...m.entries()];
  });

  // ----- Export PDF -----
  let exporting = $state(false);
  async function exportPdf() {
    if (!pg || !svgEl) return;
    exporting = true;
    try {
      const page = pg; const [w, h] = size;
      const blob = await withLightTheme(async () => {
        // Copie autonome de la feuille seule : sans grille, poignées ni tracés en cours.
        const s = standalone(svgEl, [0, 0, w, h], (el) => el.classList?.contains('ui-only'));
        (s.querySelector(':scope > g') as SVGGElement | null)?.removeAttribute('transform');
        const site = (await api.get<{ name: string }>('/site')).name;
        return toPdf(s, w, h, page.paper, page.landscape, { title: page.name, project: view.project.name, client: view.project.client ?? '', view: NATURES.find((n) => n[0] === page.nature)![2], revision: `r${page.rev}`, site, date: new Date().toLocaleDateString('fr-FR') });
      });
      const safe = `${view.project.name}`.replace(/[^\p{L}\p{N}-]+/gu, '_');
      saveFile(blob, `${page.name.replace(/[^\p{L}\p{N}-]+/gu, '_')}_${safe}_${new Date().toISOString().slice(0, 10)}.pdf`);
    } catch (e) { fail(e); } finally { exporting = false; }
  }
  const isSel = (id: string) => sel.has(id);
  const pageName = (id: string) => idx?.pages.find((p) => p.id === id)?.name ?? '';
</script>

<svelte:window onkeydown={key} onbeforeunload={(e) => { if (dirty || saving) { e.preventDefault(); void persist(); } }} />
<input type="file" accept="image/png,image/jpeg,image/svg+xml" hidden bind:this={imgInput} onchange={(e) => { onImage((e.target as HTMLInputElement).files?.[0]); (e.target as HTMLInputElement).value = ''; }}>

{#if !idx}<span class="spin"></span>
{:else}
  <div class="dw">
    <nav class="card dw-pages" aria-label="Pages">
      {#each NATURES as [n, title]}
        <div class="nat">
          <div class="nat-h"><span>{title}</span>{#if canDesign}<button class="icon-btn sm" aria-label="Nouvelle page : {title}" onclick={() => addPage(n)}><Icon name="plus" size="sm" /></button>{/if}</div>
          {#each idx.pages.filter((p) => p.nature === n) as p, i (p.id)}
            <div class="pg" class:on={p.id === pageId}>
              <button class="pg-b" onclick={() => openPage(p.id)}><span>{p.name}</span><span class="muted small">{p.count} élément{p.count > 1 ? 's' : ''}</span></button>
              {#if canDesign}<span class="pg-m"><button class="icon-btn xs" aria-label="Monter" disabled={i === 0} onclick={() => movePage(p, -1)}>↑</button><button class="icon-btn xs" aria-label="Descendre" disabled={i === idx.pages.filter((x) => x.nature === n).length - 1} onclick={() => movePage(p, 1)}>↓</button></span>{/if}
            </div>
          {:else}<span class="muted small none">Aucune page</span>{/each}
        </div>
      {/each}
    </nav>

    {#if !pg}
      <section class="card empty-card"><h3>Dessins et plan de feu</h3><p class="muted">Schémas de principe, implantations vidéo et audio, plans de feu avec adressage DMX. Créez une page avec le bouton + de sa nature.</p></section>
    {:else}
      <div class="card dw-main" style="padding:0;overflow:hidden">
        {#if !mine || requested}<div class="lockbar" class:mine class:other={!mine}>
          {#if mine}
        <span class="lk"><span class="key-i"><Icon name="lock" /></span><span><b>{requested}</b> demande la main.</span></span>
        <span class="sp"><button class="btn sm" onclick={async () => { requested = null; await flush(); await release(); await loadPage(); }}>Céder la main</button></span>
          {:else if pg.lock}
            <span class="lk"><span class="key-i"><Icon name="lock" /></span><span><b>{pg.lock.account_name}</b> a la main depuis {relative(pg.lock.taken_at)}.</span></span>
            <span class="sp">{#if canDesign}<button class="btn sm" onclick={async () => { try { await api.post(`/projects/${pid}/drawings/${pageId}/lock/request`); toast('Demande envoyée.', 'info'); } catch (e) { fail(e); } }}>Demander la main</button>{/if}</span>
          {:else}
            <span class="lk"><span class="key-i"><Icon name="unlock" /></span><span>Personne ne modifie cette page.</span></span>
            <span class="sp">{#if canDesign}<button class="btn sm primary" onclick={take}>Prendre la main</button>{:else}<span class="muted small">Lecture seule.</span>{/if}</span>
          {/if}
        </div>{/if}
        <div class="dtool">
          {#if mine}
            {#if conflict}<span class="save-st conflict" role="alert">Modifiée ailleurs entre-temps · <button class="linkbtn" onclick={keepMine}>Garder ma version</button> · <button class="linkbtn" onclick={takeTheirs}>Reprendre l’autre</button></span>
            {:else if failed}<span class="save-st conflict" role="alert">Non enregistré (réseau) · <button class="linkbtn" onclick={() => persist()}>Réessayer</button></span>
            {:else}<span class="save-st muted small" aria-live="polite">{saving ? 'Enregistrement…' : dirty ? 'Modifié' : 'Enregistré'}</span>{/if}
            <div class="seg" role="toolbar" aria-label="Outils">
              {#each [['select', 'Sélection', 'hand'], ['text', 'Texte', 'pen'], ['rect', 'Rectangle', 'box'], ['ellipse', 'Ellipse', 'radio'], ['line', 'Ligne', 'arrowr'], ['zone', 'Zone', 'grid'], ['link', 'Lien', 'syn'], ['image', 'Image', 'file']] as [t, l, ic]}
                <button class:on={tool === t && !placing} title={l} aria-label={l} onclick={() => setTool(t as Tool)}><Icon name={ic} size="sm" /></button>
              {/each}
            </div>
            <button class="icon-btn sm" aria-label="Annuler (Ctrl+Z)" title="Annuler (Ctrl+Z)" disabled={!undoStack.length} onclick={undo}><Icon name="undo" size="sm" /></button>
            <button class="icon-btn sm" aria-label="Rétablir (Ctrl+Maj+Z)" title="Rétablir (Ctrl+Maj+Z)" disabled={!redoStack.length} onclick={redo}><span style="display:inline-flex;transform:scaleX(-1)"><Icon name="undo" size="sm" /></span></button>
            <button class="btn sm ghost" onclick={sync} title="Met à jour les blocs depuis la nomenclature et retire les orphelins"><Icon name="refresh" size="sm" />Synchroniser</button>
          {/if}
          <span style="flex:1"></span>
          <span class="muted small">{Math.round(zoom * 100)} %</span>
          <button class="icon-btn sm" aria-label="Recadrer" onclick={() => { zoom = 1; panX = 0; panY = 0; }}><Icon name="fit" /></button>
          <button class="btn sm ghost" disabled={exporting} onclick={exportPdf}><Icon name="download" size="sm" />PDF</button>
          {#if mine}<button class="icon-btn sm" aria-label="Supprimer la page" onclick={delPage}><Icon name="trash" size="sm" /></button>{/if}
        </div>
        {#if tool === 'line'}<div class="hint">Ligne : cliquez les points ; double-clic ou Entrée pour terminer. <button class="btn sm primary" disabled={lineDraft.length < 2} onclick={finishLine}>Terminer</button></div>
        {:else if tool === 'link'}<div class="hint">Lien : cliquez l’origine puis la cible (un élément ou un point libre). {#if linkFrom}<b>Origine choisie.</b>{/if}</div>
        {:else if placing}<div class="hint">Cliquez sur la page pour poser <b>{idx.bom.find((b) => b.instance_id === placing)?.snapshot.name}</b>. <button class="btn sm ghost" onclick={() => (placing = null)}>Annuler</button></div>{/if}

        <div class="dw-body">
          <div class="dw-canvas" bind:clientWidth={wrapW} bind:clientHeight={wrapH}>
            <svg bind:this={svgEl} class="dw-svg" class:cross={tool !== 'select' || !!placing} role="application" aria-label="Page de dessin" onwheel={wheel} onpointerdown={down} onpointermove={move} onpointerup={up} ondblclick={() => { if (tool === 'line') finishLine(); }}>
              <defs><pattern id="dw-grid" width={pg.grid_mm || 5} height={pg.grid_mm || 5} patternUnits="userSpaceOnUse"><path d="M {pg.grid_mm || 5} 0 L 0 0 0 {pg.grid_mm || 5}" fill="none" class="gl" /></pattern></defs>
              <g class="paper" transform="translate({ox} {oy}) scale({k})">
                <rect width={size[0]} height={size[1]} fill={pg.background ?? '#ffffff'} class="sheet" data-sheet="1" />
                {#if pg.grid_mm > 0}<rect width={size[0]} height={size[1]} fill="url(#dw-grid)" class="ui-only" pointer-events="none" />{/if}
                {#each pg.elements.filter((e) => e.type === 'zone') as e (e.id)}
                  <g data-el={e.id} class="el" class:sel={isSel(e.id)}>
                    <rect x={e.x} y={e.y} width={e.w} height={e.h} rx="2" fill={e.color ?? '#3A7BD5'} fill-opacity=".1" stroke={e.color ?? '#3A7BD5'} stroke-width=".4" stroke-dasharray="2 1.2" />
                    <text x={e.x + 2} y={e.y + 4.5} font-size="3.4" font-weight="700" fill={e.color ?? '#3A7BD5'}>{e.label}</text>
                  </g>
                {/each}
                {#each pg.elements.filter((e) => e.type !== 'zone' && e.type !== 'link') as e (e.id)}
                  <g data-el={e.id} class="el" class:sel={isSel(e.id)} class:rsel={rsel.has(e.id)} style="--rs:{rsel.get(e.id) ?? 'transparent'}" class:bad={e.type === 'equipment' && conflictKeys.has(`${pg.id}:${e.id}`)}>
                    {#if e.type === 'text'}
                      <text x={e.x} y={e.y} font-size={e.size} font-weight={e.bold ? 700 : 400} fill={e.color ?? '#15171A'}>{#each String(e.text).split('\n') as ln, i}<tspan x={e.x} dy={i ? e.size * 1.25 : 0}>{ln}</tspan>{/each}</text>
                    {:else if e.type === 'rect'}
                      <rect x={e.x} y={e.y} width={e.w} height={e.h} rx={e.radius} fill={e.fill ?? 'none'} stroke={e.stroke ?? '#15171A'} stroke-width={e.width} pointer-events="all" />
                    {:else if e.type === 'ellipse'}
                      <ellipse cx={e.x + e.w / 2} cy={e.y + e.h / 2} rx={e.w / 2} ry={e.h / 2} fill={e.fill ?? 'none'} stroke={e.stroke ?? '#15171A'} stroke-width={e.width} pointer-events="all" />
                    {:else if e.type === 'line'}
                      <polyline points={e.points.map((p: number[]) => p.join(',')).join(' ')} fill="none" stroke={e.stroke ?? '#15171A'} stroke-width={e.width} stroke-dasharray={e.dash ? '2 1.2' : undefined} />
                      <polyline points={e.points.map((p: number[]) => p.join(',')).join(' ')} fill="none" stroke="transparent" stroke-width="3" class="ui-only" />
                      {#if e.arrow_end}{@const n = e.points.length}<polygon points={arrow(e.points[n - 2][0], e.points[n - 2][1], e.points[n - 1][0], e.points[n - 1][1], e.width)} fill={e.stroke ?? '#15171A'} />{/if}
                      {#if e.arrow_start}<polygon points={arrow(e.points[1][0], e.points[1][1], e.points[0][0], e.points[0][1], e.width)} fill={e.stroke ?? '#15171A'} />{/if}
                    {:else if e.type === 'image'}
                      <image href="/api/projects/{pid}/plans/files/{e.file}" x={e.x} y={e.y} width={e.w} height={e.h} preserveAspectRatio="xMidYMid meet" />
                      <rect x={e.x} y={e.y} width={e.w} height={e.h} fill="transparent" class="ui-only" />
                    {:else if e.type === 'equipment'}
                      {@const fc = e.frame ?? (e.snapshot.function ? fnColor(e.snapshot.function) : '#15171A')}
                      {@const ls = lines(e)}
                      <rect x={e.x} y={e.y} width={e.w} height={e.h} rx="1.5" fill="#ffffff" stroke={fc} stroke-width=".5" />
                      {#if e.image === 'pictogram'}<circle cx={e.x + 4.5} cy={e.y + 4.5} r="2.6" fill={fc} /><text x={e.x + 4.5} y={e.y + 5.6} font-size="3" text-anchor="middle" font-weight="700" fill="#ffffff">{e.snapshot.function ?? '·'}</text>
                      {:else if e.image?.startsWith('photo') && e.snapshot.images[0]}<image href={e.snapshot.images[0]} x={e.x + 1} y={e.y + 1} width={Math.min(e.w, e.h) - 2} height={Math.min(e.w, e.h) - 2} preserveAspectRatio="xMidYMid meet" />{/if}
                      {#each ls as t, i}<text x={e.x + (e.image && e.image !== 'none' ? 8.5 : 2)} y={e.y + 4.6 + i * 3.6} font-size={i ? 2.6 : 3} font-weight={i ? 400 : 700} fill={i ? '#5B636D' : '#15171A'} class:mono={t.startsWith('U') || t.startsWith('Ch ') || t.startsWith('#')}>{t.length > Math.max(6, (e.w - 8) / 1.6) ? t.slice(0, Math.max(5, Math.floor((e.w - 8) / 1.6))) + '…' : t}</text>{/each}
                    {/if}
                    {#if isSel(e.id) && mine && 'w' in e && e.type !== 'text'}<rect class="rz ui-only" x={e.x + e.w - 1.5} y={e.y + e.h - 1.5} width="3" height="3" />{/if}
                  </g>
                {/each}
                {#each pg.elements.filter((e) => e.type === 'link') as e (e.id)}
                  {@const c = linkColor(e)}{@const en = linkEnds(e)}
                  <g data-el={e.id} class="el" class:sel={isSel(e.id)}>
                    <path d={linkPath(e)} fill="none" stroke="transparent" stroke-width="3" class="ui-only" />
                    <path d={linkPath(e)} fill="none" stroke={c} stroke-width={e.width} />
                    {#if e.arrow_end}<polygon points={arrow(...en.e, e.width)} fill={c} />{/if}
                    {#if e.arrow_start}<polygon points={arrow(...en.s, e.width)} fill={c} />{/if}
                    {#if e.label}{@const a = anchorPt(e.from)}{@const b = anchorPt(e.to)}<text x={(a[0] + b[0]) / 2} y={(a[1] + b[1]) / 2 - 1.2} font-size="2.8" text-anchor="middle" fill={c} class="halo">{e.label}</text>{/if}
                  </g>
                {/each}
                {#if pg.legend && legend.length}
                  {@const lh = 6 + legend.length * 4}
                  <g transform="translate({size[0] - 72} {size[1] - lh - 8})">
                    <rect width="64" height={lh} fill="#ffffff" stroke="#15171A" stroke-width=".3" />
                    <text x="3" y="4.4" font-size="2.8" font-weight="700" fill="#15171A">LÉGENDE</text>
                    {#each legend as [n, col], i}{#if col}<rect x="3" y={6.6 + i * 4} width="6" height="1.4" fill={col} />{/if}<text x={col ? 11 : 3} y={7.9 + i * 4} font-size="2.6" fill="#15171A">{n.length > 34 ? n.slice(0, 33) + '…' : n}</text>{/each}
                  </g>
                {/if}
                {#if pg.scale}<text x="8" y={size[1] - 6} font-size="3" fill="#5B636D">Échelle 1:{pg.scale}</text>{/if}
                {#if draft}<rect x={Math.min(draft.x0, draft.x1)} y={Math.min(draft.y0, draft.y1)} width={Math.abs(draft.x1 - draft.x0)} height={Math.abs(draft.y1 - draft.y0)} class="draft ui-only" />{/if}
                {#if lineDraft.length}<polyline points={[...lineDraft, ...(hover ? [hover] : [])].map((p) => p.join(',')).join(' ')} class="draft ui-only" fill="none" />{/if}
                {#if linkFrom && hover}{@const a = anchorPt(linkFrom)}<line x1={a[0]} y1={a[1]} x2={hover[0]} y2={hover[1]} class="draft ui-only" />{/if}
              </g>
            </svg>
            {#if pageId}<RemoteCursors room={`project:${pid}/draw:${pageId}`} toScreen={(x, y) => [ox + x * k, oy + y * k]} bind:selections={remoteSel} />{/if}
          </div>

          <aside class="dw-side">
            <div class="seg side-tabs" role="tablist" aria-label="Panneau">
              {#each [['place', `À placer ${toPlace.length}`], ['props', 'Propriétés'], ['page', 'Document'], ...(pg.nature === 'lighting' ? [['dmx', 'DMX']] : [])] as [s, l]}<button role="tab" aria-selected={side === s} class:on={side === s} onclick={() => (side = s as typeof side)}>{l}</button>{/each}
            </div>
            {#if side === 'place'}
              <label class="search"><Icon name="search" /><input class="input sm" bind:value={q} placeholder="Équipement, ID…" aria-label="Filtrer la nomenclature"></label>
              {#if !toPlace.length}<p class="muted small">Rien à placer pour cette nature de page.</p>{/if}
              {#each toPlace as b (b.instance_id)}
                <button class="li" class:on={placing === b.instance_id} disabled={!mine} onclick={() => { setTool('select'); placing = b.instance_id; }}><span class="nm"><i class="dot" style="--c:{fnColor(b.snapshot.function)}"></i><b>{b.snapshot.name}</b> <span class="mono muted">{b.snapshot.tag}</span></span>{#if b.snapshot.dmx_channels}<span class="muted small">{b.snapshot.dmx_channels} canaux DMX</span>{/if}</button>
              {/each}
              <h4>Placés sur cette page · {placedHere.length}</h4>
              {#each placedHere as e (e.id)}<button class="li" onclick={() => { sel = new Set([e.id]); side = 'props'; }}><span class="nm"><b>{e.label ?? e.snapshot.name}</b> <span class="mono muted">{e.snapshot.tag}</span></span>{#if pg.nature === 'lighting'}<span class="small mono" class:warn-t={conflictKeys.has(`${pg.id}:${e.id}`)}>{dmxText(e)}</span>{/if}</button>{/each}
              {#if placedElsewhere.length}<h4>Sur d’autres pages</h4>{#each placedElsewhere as b (b.instance_id)}<span class="small muted">{b.snapshot.name} {b.snapshot.tag} · {pageName(b.placed.find((p) => p.nature === pg!.nature)!.page)}</span>{/each}{/if}
            {:else if side === 'props'}
              {@const ro = !mine}
              {#if !selList.length}<p class="muted small">Choisissez un élément (Maj + clic pour en choisir plusieurs).</p>
              {:else if selList.length > 1 && selList.every((e) => e.type === 'link')}
                <h4>{selList.length} liens</h4>
                <label class="field">Tracé<select class="input sm" disabled={ro} onchange={(e) => patchSel({ style: (e.target as HTMLSelectElement).value })}><option value="">—</option><option value="straight">Droit</option><option value="orthogonal">Orthogonal</option><option value="curve">Courbe</option></select></label>
                <label class="field">Signal<select class="input sm" disabled={ro} onchange={(e) => patchSel({ signal_id: (e.target as HTMLSelectElement).value || null })}><option value="">—</option>{#each refList('signals', true) as s}<option value={s.id}>{s.name}</option>{/each}</select></label>
                <div class="fg2"><label class="field">Couleur<input type="color" disabled={ro} onchange={(e) => patchSel({ color: (e.target as HTMLInputElement).value })}></label><label class="field">Épaisseur<input class="input sm" type="number" step="0.1" min="0.1" disabled={ro} onchange={(e) => patchSel({ width: Number((e.target as HTMLInputElement).value) })}></label></div>
                <label class="toggle-row small"><input type="checkbox" disabled={ro} onchange={(e) => patchSel({ arrow_end: (e.target as HTMLInputElement).checked })}>Flèche à l’arrivée</label>
                <button class="btn sm ghost danger" disabled={ro} onclick={removeSel}><Icon name="trash" size="sm" />Supprimer</button>
              {:else if selList.length > 1}
                <p class="small">{selList.length} éléments choisis.</p>
                <div class="row-btns"><button class="btn sm" disabled={ro} onclick={duplicateSel}>Dupliquer</button><button class="btn sm ghost danger" disabled={ro} onclick={removeSel}>Supprimer</button></div>
              {:else}
                {@const e = selList[0]!}
                {#if e.type === 'equipment'}
                  <h4>{e.snapshot.name} <span class="mono">{e.snapshot.tag}</span></h4>
                  <p class="small muted">{[e.snapshot.reference, e.snapshot.category].filter(Boolean).join(' · ')}</p>
                  <label class="field">Libellé propre<input class="input sm" value={e.label ?? ''} placeholder={e.snapshot.name} maxlength="80" disabled={ro} onchange={(x) => patchSel({ label: (x.target as HTMLInputElement).value.trim() || null })}></label>
                  <label class="field">Image<select class="input sm" value={e.image ?? 'none'} disabled={ro} onchange={(x) => patchSel({ image: (x.target as HTMLSelectElement).value })}><option value="pictogram">Pictogramme</option>{#if e.snapshot.images.length}<option value="photo">Image du catalogue</option>{/if}<option value="none">Aucune</option></select></label>
                  <span class="flabel">Afficher (au moins un)</span>
                  <div class="chips">{#each [['label', 'Libellé'], ['name', 'Nom'], ['reference', 'Référence'], ['tag', 'ID'], ['category', 'Catégorie'], ...(pg.nature === 'lighting' ? [['dmx', 'DMX']] : [])] as [kk, l]}<label class="chk"><input type="checkbox" checked={e.show.includes(kk)} disabled={ro || (e.show.length === 1 && e.show.includes(kk))} onchange={(x) => patchSel({ show: (x.target as HTMLInputElement).checked ? [...e.show, kk] : e.show.filter((s: string) => s !== kk) })}>{l}</label>{/each}</div>
                  <div class="fg2"><label class="field">Cadre<input type="color" value={e.frame ?? fnColor(e.snapshot.function)} disabled={ro} onchange={(x) => patchSel({ frame: (x.target as HTMLInputElement).value })}></label><label class="field">Rotation<select class="input sm" value={e.rot} disabled={ro} onchange={(x) => patchSel({ rot: Number((x.target as HTMLSelectElement).value) })}>{#each [0, 90, 180, 270] as r}<option value={r}>{r}°</option>{/each}</select></label></div>
                  {#if pg.nature === 'lighting'}
                    <h4>DMX</h4>
                    {@const d = e.dmx ?? { console: 0, universe: 0, address: 0, mode: null }}
                    <div class="fg2">
                      <label class="field">N° console<input class="input sm mono" type="number" min="1" value={d.console || ''} disabled={ro} onchange={(x) => patchSel({ dmx: { ...d, console: Number((x.target as HTMLInputElement).value) } })}></label>
                      <label class="field">Univers<input class="input sm mono" type="number" min="1" max="65535" value={d.universe || ''} disabled={ro} onchange={(x) => patchSel({ dmx: { ...d, universe: Number((x.target as HTMLInputElement).value) } })}></label>
                      <label class="field">Adresse<input class="input sm mono" type="number" min="1" max="512" value={d.address || ''} disabled={ro} onchange={(x) => patchSel({ dmx: { ...d, address: Number((x.target as HTMLInputElement).value) } })}></label>
                      <label class="field">Mode<select class="input sm" value={d.mode ?? ''} disabled={ro} onchange={(x) => patchSel({ dmx: { ...d, mode: (x.target as HTMLSelectElement).value || null } })}><option value="">Défaut ({e.snapshot.dmx_channels ?? '?'} c.)</option>{#each e.snapshot.dmx_modes as [n, c]}<option value={n}>{n} ({c} c.)</option>{/each}</select></label>
                    </div>
                    <p class="small">{chans(e)} canaux{d.address ? ` · ${d.address}–${d.address + chans(e) - 1}` : ''}{#if conflictKeys.has(`${pg.id}:${e.id}`)} · <b class="warn-t">chevauchement</b>{/if}</p>
                    {#if !ro}<button class="btn sm" onclick={() => suggestFor(e)}>Proposer une adresse</button>{/if}
                  {/if}
                {:else if e.type === 'link'}
                  <label class="field">Tracé<select class="input sm" value={e.style} disabled={ro} onchange={(x) => patchSel({ style: (x.target as HTMLSelectElement).value })}><option value="straight">Droit</option><option value="orthogonal">Orthogonal</option><option value="curve">Courbe</option></select></label>
                  <label class="field">Signal<select class="input sm" value={e.signal_id ?? ''} disabled={ro} onchange={(x) => patchSel({ signal_id: (x.target as HTMLSelectElement).value || null })}><option value="">—</option>{#each refList('signals', true) as s}<option value={s.id}>{s.name}</option>{/each}</select></label>
                  <label class="field">Libellé<input class="input sm" value={e.label ?? ''} maxlength="80" disabled={ro} onchange={(x) => patchSel({ label: (x.target as HTMLInputElement).value.trim() || null })}></label>
                  <div class="fg2"><label class="field">Couleur<input type="color" value={linkColor(e)} disabled={ro} onchange={(x) => patchSel({ color: (x.target as HTMLInputElement).value })}></label><label class="field">Épaisseur<input class="input sm" type="number" step="0.1" min="0.1" max="20" value={e.width} disabled={ro} onchange={(x) => patchSel({ width: Number((x.target as HTMLInputElement).value) })}></label></div>
                  <label class="toggle-row small"><input type="checkbox" checked={e.arrow_start} disabled={ro} onchange={(x) => patchSel({ arrow_start: (x.target as HTMLInputElement).checked })}>Flèche au départ</label>
                  <label class="toggle-row small"><input type="checkbox" checked={e.arrow_end} disabled={ro} onchange={(x) => patchSel({ arrow_end: (x.target as HTMLInputElement).checked })}>Flèche à l’arrivée</label>
                {:else if e.type === 'text'}
                  <label class="field">Texte<textarea class="input" rows="3" disabled={ro} onchange={(x) => patchSel({ text: (x.target as HTMLTextAreaElement).value })}>{e.text}</textarea></label>
                  <div class="fg2"><label class="field">Taille (mm)<input class="input sm" type="number" min="1" max="200" step="0.5" value={e.size} disabled={ro} onchange={(x) => patchSel({ size: Number((x.target as HTMLInputElement).value) })}></label><label class="field">Couleur<input type="color" value={e.color ?? '#15171A'} disabled={ro} onchange={(x) => patchSel({ color: (x.target as HTMLInputElement).value })}></label></div>
                  <label class="toggle-row small"><input type="checkbox" checked={e.bold} disabled={ro} onchange={(x) => patchSel({ bold: (x.target as HTMLInputElement).checked })}>Gras</label>
                {:else if e.type === 'zone'}
                  <label class="field">Nom<input class="input sm" value={e.label} maxlength="80" disabled={ro} onchange={(x) => patchSel({ label: (x.target as HTMLInputElement).value })}></label>
                  <label class="field">Couleur<input type="color" value={e.color ?? '#3A7BD5'} disabled={ro} onchange={(x) => patchSel({ color: (x.target as HTMLInputElement).value })}></label>
                {:else if e.type === 'rect' || e.type === 'ellipse' || e.type === 'line'}
                  <div class="fg2"><label class="field">Trait<input type="color" value={e.stroke ?? '#15171A'} disabled={ro} onchange={(x) => patchSel({ stroke: (x.target as HTMLInputElement).value })}></label><label class="field">Épaisseur<input class="input sm" type="number" step="0.1" min="0" max="20" value={e.width} disabled={ro} onchange={(x) => patchSel({ width: Number((x.target as HTMLInputElement).value) })}></label></div>
                  {#if e.type !== 'line'}<label class="toggle-row small"><input type="checkbox" checked={!!e.fill} disabled={ro} onchange={(x) => patchSel({ fill: (x.target as HTMLInputElement).checked ? '#E8ECF2' : null })}>Remplissage</label>{#if e.fill}<input type="color" value={e.fill} disabled={ro} onchange={(x) => patchSel({ fill: (x.target as HTMLInputElement).value })}>{/if}{/if}
                  {#if e.type === 'line'}<label class="toggle-row small"><input type="checkbox" checked={e.dash} disabled={ro} onchange={(x) => patchSel({ dash: (x.target as HTMLInputElement).checked })}>Pointillés</label><label class="toggle-row small"><input type="checkbox" checked={e.arrow_end} disabled={ro} onchange={(x) => patchSel({ arrow_end: (x.target as HTMLInputElement).checked })}>Flèche</label>{/if}
                {/if}
                {#if !ro}<div class="row-btns" style="margin-top:8px">{#if e.type !== 'equipment' && e.type !== 'link'}<button class="btn sm" onclick={duplicateSel}>Dupliquer</button>{/if}<button class="btn sm ghost danger" onclick={removeSel}><Icon name="trash" size="sm" />{e.type === 'equipment' ? 'Retirer de la page' : 'Supprimer'}</button></div>{/if}
              {/if}
            {:else if side === 'page'}
              {@const ro = !mine}
              <label class="field">Nom<input class="input sm" value={pg.name} maxlength="80" disabled={ro} onchange={(x) => { pg!.name = (x.target as HTMLInputElement).value; changed(); }}></label>
              <div class="fg2">
                <label class="field">Format<select class="input sm" value={pg.paper} disabled={ro} onchange={(x) => { pg!.paper = (x.target as HTMLSelectElement).value as Paper; changed(); }}>{#each Object.keys(SIZES) as p}<option value={p}>{p}</option>{/each}</select></label>
                <label class="field">Orientation<select class="input sm" value={pg.landscape ? 'l' : 'p'} disabled={ro} onchange={(x) => { pg!.landscape = (x.target as HTMLSelectElement).value === 'l'; changed(); }}><option value="l">Paysage</option><option value="p">Portrait</option></select></label>
                <label class="field">Grille (mm)<input class="input sm" type="number" min="0" max="50" value={pg.grid_mm} disabled={ro} onchange={(x) => { pg!.grid_mm = Number((x.target as HTMLInputElement).value); changed(); }}></label>
                <label class="field">Échelle 1:<input class="input sm" type="number" min="1" value={pg.scale ?? ''} placeholder="—" disabled={ro} onchange={(x) => { const v = Number((x.target as HTMLInputElement).value); pg!.scale = v > 0 ? v : null; changed(); }}></label>
              </div>
              <label class="toggle-row small"><input type="checkbox" checked={pg.snap} disabled={ro} onchange={(x) => { pg!.snap = (x.target as HTMLInputElement).checked; changed(); }}>Aimantation à la grille</label>
              <label class="toggle-row small"><input type="checkbox" checked={pg.legend} disabled={ro} onchange={(x) => { pg!.legend = (x.target as HTMLInputElement).checked; changed(); }}>Légende imprimée</label>
              <label class="field">Fond<input type="color" value={pg.background ?? '#ffffff'} disabled={ro} onchange={(x) => { pg!.background = (x.target as HTMLInputElement).value; changed(); }}></label>
              {#if pg.nature === 'lighting'}<label class="field">Afficher sur les blocs<select class="input sm" value={pg.dmx_display} disabled={ro} onchange={(x) => { pg!.dmx_display = (x.target as HTMLSelectElement).value as PageData['dmx_display']; changed(); }}><option value="address">Univers et adresse</option><option value="channel">Canal absolu</option><option value="number">N° console</option></select></label>{/if}
            {:else}
              <p class="small">{idx.dmx.fixtures.length} projecteur{idx.dmx.fixtures.length > 1 ? 's' : ''} sur les plans de feu · {idx.dmx.fixtures.filter((f) => f.patch?.address).length} adressé{idx.dmx.fixtures.filter((f) => f.patch?.address).length > 1 ? 's' : ''}</p>
              {#if idx.dmx.conflicts.length}<p class="small warn-t"><b>{idx.dmx.conflicts.length} chevauchement{idx.dmx.conflicts.length > 1 ? 's' : ''}</b> dans un même univers.</p>{/if}
              {#if canDesign}<div class="row-btns"><button class="btn sm primary" onclick={() => dmxAuto('missing')}>Adresser les manquants</button><button class="btn sm" onclick={() => dmxAuto('all')}>Tout réadresser</button></div>{/if}
              <table class="tbl dmx-t"><thead><tr><th>Appareil</th><th>#</th><th>U</th><th>Adr.</th><th>Can.</th></tr></thead><tbody>
                {#each placedHere.slice().sort((a, b) => (a.dmx?.universe ?? 99999) - (b.dmx?.universe ?? 99999) || (a.dmx?.address ?? 0) - (b.dmx?.address ?? 0)) as e (e.id)}
                  <tr class:bad={conflictKeys.has(`${pg.id}:${e.id}`)} onclick={() => { sel = new Set([e.id]); side = 'props'; }}><td><button type="button" class="rowlink" onclick={(ev) => { ev.stopPropagation(); sel = new Set([e.id]); side = 'props'; }}>{e.label ?? e.snapshot.name}</button> <span class="mono muted">{e.snapshot.tag}</span></td><td class="mono">{e.dmx?.console || '—'}</td><td class="mono">{e.dmx?.universe || '—'}</td><td class="mono">{e.dmx?.address || '—'}</td><td class="mono">{chans(e)}</td></tr>
                {/each}
              </tbody></table>
            {/if}
          </aside>
        </div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .dw { display: grid; grid-template-columns: 230px minmax(0, 1fr); gap: 12px; align-items: start; }
  .dw-pages { padding: 10px; display: grid; gap: 10px; position: sticky; top: 80px; }
  .nat-h { display: flex; justify-content: space-between; align-items: center; font: 650 12px var(--font); text-transform: uppercase; letter-spacing: .04em; color: var(--fg-2); }
  .pg { display: flex; align-items: center; border-radius: 10px; }
  .pg.on { background: var(--primary-bg); color: var(--primary-fg); }
  .pg.on .muted { color: inherit; opacity: .7; }
  .pg-b { flex: 1; display: flex; flex-direction: column; align-items: flex-start; gap: 0; background: none; border: 0; font: inherit; color: inherit; padding: 6px 8px; cursor: pointer; text-align: left; min-width: 0; }
  .pg-b span:first-child { font-weight: 600; font-size: 14px; }
  .pg-m { display: flex; flex-direction: column; }
  .icon-btn.xs { width: 20px; height: 18px; font-size: 11px; color: inherit; }
  .none { padding-left: 8px; }
  .dtool { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; padding: 8px 14px; border-top: 1px solid var(--line); }
  .hint { padding: 8px 16px; background: color-mix(in srgb, var(--accent, #4F6BFF) 8%, var(--surface)); border-top: 1px solid var(--line); display: flex; gap: 10px; align-items: center; flex-wrap: wrap; font-size: 14px; }
  .dw-body { display: grid; grid-template-columns: minmax(0, 1fr) 290px; border-top: 1px solid var(--line); height: calc(100vh - 240px); min-height: 520px; }
  .dw-canvas { position: relative; overflow: hidden; background: var(--surface-2); }
  .dw-svg { width: 100%; height: 100%; display: block; touch-action: none; user-select: none; cursor: default; }
  .dw-svg.cross { cursor: crosshair; }
  .sheet { filter: drop-shadow(0 2px 6px rgba(0, 0, 0, .18)); }
  .gl { stroke: #E3E6EA; stroke-width: .15; }
  .el { cursor: pointer; }
  .el.sel > :first-child, .el.sel rect:first-of-type { filter: drop-shadow(0 0 1.2px #4F6BFF) drop-shadow(0 0 1.2px #4F6BFF); }
  .el.rsel > :first-child { filter: drop-shadow(0 0 1px var(--rs)) drop-shadow(0 0 1.5px var(--rs)); }
  .el.bad rect:first-of-type { stroke: #CF3A2F; stroke-width: .9; }
  .rz { fill: #4F6BFF; cursor: nwse-resize; }
  .draft { stroke: #4F6BFF; stroke-width: .4; stroke-dasharray: 1.5 1; fill: rgba(79, 107, 255, .06); }
  .halo { paint-order: stroke; stroke: #ffffff; stroke-width: .8px; }
  .mono { font-family: var(--font-mono, monospace); }
  .dw-side { border-left: 1px solid var(--line); padding: 12px; overflow: auto; display: flex; flex-direction: column; gap: 6px; background: var(--surface); }
  .side-tabs { flex-wrap: wrap; margin-bottom: 6px; }
  .side-tabs button { font-size: 12.5px; padding: 6px 8px; }
  .dw-side h4 { margin: 12px 0 2px; font-size: 13px; text-transform: uppercase; letter-spacing: .04em; color: var(--fg-2); }
  .li { display: flex; flex-direction: column; gap: 2px; text-align: left; padding: 7px 10px; border-radius: 10px; border: 1px solid var(--line); background: var(--surface); font: inherit; color: inherit; cursor: pointer; }
  .li:hover:not(:disabled) { border-color: var(--fg-3); }
  .li.on { border-color: var(--accent, #4F6BFF); box-shadow: 0 0 0 1.5px color-mix(in srgb, var(--accent, #4F6BFF) 25%, transparent); }
  .nm { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
  .dot { display: inline-block; width: 10px; height: 10px; border-radius: 50%; background: var(--c); flex: none; }
  .field { display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
  .input.sm { height: 34px; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chk { display: inline-flex; gap: 4px; align-items: center; font-size: 13px; padding: 3px 8px; border: 1px solid var(--line); border-radius: 99px; }
  .row-btns { display: flex; gap: 8px; flex-wrap: wrap; }
  .warn-t { color: var(--err); }
  .dmx-t { font-size: 13px; margin-top: 8px; }
  .dmx-t tr { cursor: pointer; }
  .dmx-t tr.bad td { color: var(--err); font-weight: 600; }
  .search { display: flex; align-items: center; gap: 6px; }
  .empty-card { padding: 28px; text-align: center; }
  @media (max-width: 1100px) { .dw { grid-template-columns: 1fr; } .dw-pages { position: static; } .dw-body { grid-template-columns: 1fr; height: auto; } .dw-canvas { height: 64vh; } .dw-side { border-left: 0; border-top: 1px solid var(--line); } }
  .save-st { order: 99; margin-left: auto; }
  .save-st.conflict { font-size: 13px; color: var(--warn, #D97706); }
  .linkbtn { border: 0; background: none; padding: 0; color: var(--accent); font: inherit; font-weight: 600; cursor: pointer; text-decoration: underline; }
</style>
