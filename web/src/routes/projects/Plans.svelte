<script lang="ts">
  // Cartographie 2D et 3D (section 11.1). Positions en mm réels dans le repère commun de la
  // carte (calibration : premier point = origine, second = axe X). Une main par carte.
  import { untrack, onDestroy } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { join, sendCursor, sendSelection, type PresenceMember } from '../../lib/live.svelte';
  import RemoteCursors from '../../components/RemoteCursors.svelte';
  import { relative } from '../../lib/format';
  import { loadRefs, refList, refById, refName } from '../../lib/catalog.svelte';
  import type { ProjectView } from '../../lib/projects';

  type Kind = 'multi_floor' | 'flat' | 'outdoor';
  interface MapSum { id: string; name: string; kind: Kind; floors: { id: string; level: number; name: string }[]; containers: number }
  interface Index { maps: MapSum[]; stats: { containers: number; placed: number; liaisons: number; traced: number } }
  interface Calib { a: [number, number]; b: [number, number]; dist_mm: number }
  interface Floor { id: string; map_id: string; level: number; name: string; height_mm: number; ceiling_mm: number; calibration: Calib | null; background: string | null; bg_type: string | null; bg_w: number | null; bg_h: number | null; bg_opacity: number; base_mm: number | null }
  interface Obj { id: string; map_id: string; floor_id: string; kind: 'container' | 'point' | 'text' | 'measure'; [k: string]: any }
  interface Link { id: string; point_id: string; target_id: string; label: string | null }
  interface WP { x: number; y: number; level: number; h?: number | null }
  interface Liaison { key: string; a: { id: string; name: string | null }; b: { id: string; name: string | null }; count: number; numbers: string[]; family: string | null; color: string | null; state: 'ready' | 'other_map' | 'unplaced'; waypoints: WP[] | null; points3d: [number, number, number][] | null; length_m: number | null }
  interface Show { map: { id: string; name: string; kind: Kind }; floors: Floor[]; objects: Obj[]; links: Link[]; liaisons: Liaison[]; unplaced: { id: string; name: string; kind: string; dims: [number, number, number] }[]; lock: { account_name: string; taken_at: string; mine: boolean } | null }

  let { view }: { view: ProjectView } = $props();
  const pid = $derived(view.project.id);
  const canDesign = $derived(can('conception', 'ecriture'));
  const KINDS: Record<Kind, string> = { multi_floor: 'Multi-étages', flat: 'Plat', outdoor: 'Extérieur' };

  let idx = $state<Index | null>(null);
  let mapId = $state<string | null>(null);
  let d = $state<Show | null>(null);
  let floorId = $state<string | null>(null);
  let mode = $state<'2d' | '3d'>('2d');
  let side = $state<'place' | 'trace' | 'layers' | 'sel'>('place');
  const mine = $derived(!!d?.lock?.mine);
  const floor = $derived(d?.floors.find((f) => f.id === floorId) ?? d?.floors[0] ?? null);
  const objs = $derived((d?.objects ?? []).filter((o) => o.floor_id === floor?.id));
  const byId = $derived(new Map((d?.objects ?? []).map((o) => [o.id, o])));
  const levelOf = (fid: string) => d?.floors.find((f) => f.id === fid)?.level ?? 0;

  const LS = 'strata.plans.map';
  async function loadIndex() {
    try {
      idx = await api.get(`/projects/${pid}/plans`);
      if (!idx) return;
      let saved: string | null = null; try { saved = localStorage.getItem(`${LS}.${pid}`); } catch { /* sans stockage */ }
      if (!mapId || !idx.maps.some((m) => m.id === mapId)) await openMap(idx.maps.find((m) => m.id === saved)?.id ?? idx.maps[0]?.id ?? null);
    } catch (e) { fail(e); }
  }
  async function refreshIndex() { try { idx = await api.get(`/projects/${pid}/plans`); } catch { /* prochain chargement */ } }
  async function load() {
    if (!mapId) { d = null; return; }
    try {
      d = await api.get(`/projects/${pid}/plans/${mapId}`);
      if (d && !d.floors.some((f) => f.id === floorId)) floorId = d.floors.find((f) => f.level === 0)?.id ?? d.floors[0]?.id ?? null;
      if (sel && !d?.objects.some((o) => o.id === sel)) sel = null;
    } catch (e) { fail(e); }
  }
  async function openMap(id: string | null) {
    if (mapId && mapId !== id) await release();
    mapId = id; sel = null; tool = 'select'; trace = null; fitted = '';
    try { if (id) localStorage.setItem(`${LS}.${pid}`, id); } catch { /* sans stockage */ }
    await load();
    if (id && canDesign) await take();
  }
  $effect(() => { view; untrack(() => { loadRefs(); if (!idx) loadIndex(); else if (!mine) { loadIndex(); load(); } }); });

  // ----- Main -----
  let beat: ReturnType<typeof setInterval> | null = null;
  let lastAction = Date.now();
  let requested = $state<string | null>(null);
  async function take() {
    if (!canDesign || !mapId) return;
    try { const r = await api.post<{ mine: boolean }>(`/projects/${pid}/plans/${mapId}/lock`); if (r.mine && !beat) beat = setInterval(heartbeat, 20_000); await load(); } catch (e) { fail(e); }
  }
  async function heartbeat() {
    if (Date.now() - lastAction > 10 * 60_000) { await release(); await load(); toast('Main cédée après 10 minutes sans action.', 'info'); return; }
    try { const r = await api.post<{ mine: boolean }>(`/projects/${pid}/plans/${mapId}/lock`); if (!r.mine) { stopBeat(); await load(); } } catch { /* prochain battement */ }
  }
  function stopBeat() { if (beat) { clearInterval(beat); beat = null; } }
  async function release() { stopBeat(); if (!mapId || !d?.lock?.mine) return; try { await api.del(`/projects/${pid}/plans/${mapId}/lock`); } catch { /* déjà libre */ } }
  async function requestHand() { try { await api.post(`/projects/${pid}/plans/${mapId}/lock/request`); toast(`Demande envoyée à ${d?.lock?.account_name}.`, 'info'); } catch (e) { fail(e); } }
  $effect(() => join(`project:${pid}`, (c) => {
    if (c.kind === 'lock' && mapId && c.id === `map:${mapId}`) {
      if (c.action === 'request' && mine) requested = c.actor;
      if (c.action === 'taken') { if (mine) stopBeat(); load(); }
      if (c.action === 'released' && !mine) load();
      return;
    }
    if (c.kind === 'plan' && !mine && !drag) { loadIndex(); load(); }
  }));
  onDestroy(() => { stopBeat(); if (mapId && d?.lock?.mine) fetch(`/api/projects/${pid}/plans/${mapId}/lock`, { method: 'DELETE', headers: { 'X-Strata-Request': '1' }, keepalive: true }); });

  async function write<T>(f: () => Promise<T>): Promise<T | undefined> {
    lastAction = Date.now();
    try { const r = await f(); await Promise.all([load(), refreshIndex()]); return r; } catch (e) { fail(e); await load(); return undefined; }
  }

  // ----- Assistant de carte -----
  let wiz = $state<{ name: string; kind: Kind; from: number; to: number } | null>(null);
  let wizErr = $state<ApiError | null>(null);
  async function createMap() {
    if (!wiz) return;
    const levels = wiz.kind === 'multi_floor' ? Array.from({ length: Math.abs(wiz.to - wiz.from) + 1 }, (_, i) => Math.min(wiz!.from, wiz!.to) + i) : [0];
    try {
      const r = await api.post<{ id: string }>(`/projects/${pid}/plans`, { name: wiz.name, kind: wiz.kind, levels });
      wiz = null; await loadIndex(); await openMap(r.id); toast('Carte créée.');
    } catch (e) { if (e instanceof ApiError && e.fields.length) wizErr = e; else fail(e); }
  }
  let mapEdit = $state<{ name: string; kind: Kind } | null>(null);
  async function saveMap() { if (!mapEdit) return; await write(() => api.patch(`/projects/${pid}/plans/${mapId}`, mapEdit)); mapEdit = null; await loadIndex(); }
  async function deleteMap() {
    if (!d || !confirm(`Supprimer la carte « ${d.map.name} », ses étages, fonds et tracés ?`)) return;
    try { await api.post(`/projects/${pid}/plans/${mapId}/delete`); stopBeat(); mapId = null; d = null; await loadIndex(); } catch (e) { fail(e); }
  }

  // ----- Étages -----
  let floorEdit = $state<Floor | null>(null);
  let newLevel = $state<number | null>(null);
  const freeLevels = $derived(Array.from({ length: 13 }, (_, i) => i - 2).filter((l) => !d?.floors.some((f) => f.level === l)));
  async function saveFloor() {
    if (!floorEdit) return;
    const { id, name, height_mm, ceiling_mm, calibration, bg_opacity, level } = floorEdit;
    await write(() => api.patch(`/projects/${pid}/plans/${mapId}/floors`, { id, level, name, height_mm: Number(height_mm), ceiling_mm: Number(ceiling_mm), calibration, bg_opacity: Number(bg_opacity) }));
    floorEdit = null;
  }
  async function addFloor() {
    if (newLevel == null) return;
    const r = await write(() => api.post<{ id: string }>(`/projects/${pid}/plans/${mapId}/floors`, { level: newLevel }));
    if (r) { floorId = r.id; newLevel = null; await loadIndex(); }
  }
  async function deleteFloor(f: Floor) {
    if (!confirm(`Supprimer l’étage « ${f.name} » et tout ce qui y est posé ?`)) return;
    await write(() => api.post(`/projects/${pid}/plans/${mapId}/floors/delete`, { id: f.id })); floorEdit = null; await loadIndex();
  }
  async function uploadBg(f: Floor, file: File) {
    if (!/^image\/(png|jpeg|svg\+xml)$/.test(file.type)) { toast('Fond de plan : une image PNG, JPG ou SVG.', 'err'); return; }
    if (file.size > 15 * 1024 * 1024) { toast('Fond de plan de 15 Mo au plus.', 'err'); return; }
    const url = URL.createObjectURL(file);
    const size = await new Promise<[number, number]>((res, rej) => { const im = new Image(); im.onload = () => res([im.naturalWidth || 1000, im.naturalHeight || 1000]); im.onerror = rej; im.src = url; }).catch(() => [1000, 1000] as [number, number]);
    URL.revokeObjectURL(url);
    await write(() => api.putRaw(`/projects/${pid}/plans/${mapId}/floors/${f.id}/background?w=${size[0]}&h=${size[1]}`, file, file.type));
    floorEdit = d?.floors.find((x) => x.id === f.id) ? { ...d!.floors.find((x) => x.id === f.id)! } : null;
    fitted = '';
    toast('Fond de plan posé. Calibrez-le : deux points et leur distance réelle.', 'info');
  }
  async function removeBg(f: Floor) { await write(() => api.del(`/projects/${pid}/plans/${mapId}/floors/${f.id}/background`)); floorEdit = null; }

  // ----- Repère : image → monde -----
  function bgMatrix(f: Floor): string {
    const c = f.calibration;
    if (!c) return 'matrix(10 0 0 10 0 0)';
    const dx = c.b[0] - c.a[0], dy = c.b[1] - c.a[1], s = c.dist_mm / Math.hypot(dx, dy), ang = Math.atan2(dy, dx);
    const co = Math.cos(ang) * s, si = Math.sin(ang) * s;
    const e = -(co * c.a[0] + si * c.a[1]), f2 = -(-si * c.a[0] + co * c.a[1]);
    return `matrix(${co} ${-si} ${si} ${co} ${e} ${f2})`;
  }
  /** Monde → pixel du fond (inverse de la calibration). */
  function worldToImg(f: Floor, p: [number, number]): [number, number] {
    const c = f.calibration;
    if (!c) return [p[0] / 10, p[1] / 10];
    const dx = c.b[0] - c.a[0], dy = c.b[1] - c.a[1], s = c.dist_mm / Math.hypot(dx, dy), ang = Math.atan2(dy, dx);
    const co = Math.cos(ang), si = Math.sin(ang), x = p[0] / s, y = p[1] / s;
    return [c.a[0] + x * co - y * si, c.a[1] + x * si + y * co];
  }

  // ----- Vue 2D -----
  let svgEl = $state<SVGSVGElement>(null!);
  let wrapW = $state(800), wrapH = $state(600);
  let k = $state(0.05), tx = $state(40), ty = $state(40);
  const toWorld = (e: { clientX: number; clientY: number }): [number, number] => { const r = svgEl.getBoundingClientRect(); return [(e.clientX - r.left - tx) / k, (e.clientY - r.top - ty) / k]; };
  function wheel(e: WheelEvent) {
    e.preventDefault();
    const r = svgEl.getBoundingClientRect(), mx = e.clientX - r.left, my = e.clientY - r.top;
    const nk = Math.min(2, Math.max(0.002, k * (e.deltaY < 0 ? 1.15 : 1 / 1.15)));
    tx = mx - ((mx - tx) * nk) / k; ty = my - ((my - ty) * nk) / k; k = nk;
  }
  function bounds(): [number, number, number, number] {
    const xs: number[] = [], ys: number[] = [];
    if (floor?.background && floor.bg_w && floor.bg_h) {
      for (const p of [[0, 0], [floor.bg_w, 0], [0, floor.bg_h], [floor.bg_w, floor.bg_h]] as [number, number][]) {
        const c = floor.calibration;
        let w: [number, number];
        if (c) { const dx = c.b[0] - c.a[0], dy = c.b[1] - c.a[1], s = c.dist_mm / Math.hypot(dx, dy), ang = Math.atan2(dy, dx), px = p[0] - c.a[0], py = p[1] - c.a[1]; w = [(px * Math.cos(ang) + py * Math.sin(ang)) * s, (-px * Math.sin(ang) + py * Math.cos(ang)) * s]; }
        else w = [p[0] * 10, p[1] * 10];
        xs.push(w[0]); ys.push(w[1]);
      }
    }
    for (const o of d?.objects ?? []) {
      if (o.kind === 'measure') for (const p of o.points) { xs.push(p[0]); ys.push(p[1]); }
      else { xs.push(o.x); ys.push(o.y); }
    }
    if (!xs.length) return [-5000, -5000, 20000, 15000];
    return [Math.min(...xs) - 2000, Math.min(...ys) - 2000, Math.max(...xs) + 2000, Math.max(...ys) + 2000];
  }
  function fit() {
    if (!svgEl) return;
    const [x0, y0, x1, y1] = bounds(), r = svgEl.getBoundingClientRect();
    k = Math.min(2, Math.max(0.002, Math.min((r.width - 60) / (x1 - x0), (r.height - 60) / (y1 - y0))));
    tx = -x0 * k + (r.width - (x1 - x0) * k) / 2; ty = -y0 * k + (r.height - (y1 - y0) * k) / 2;
  }
  let fitted = '';
  $effect(() => { if (d && svgEl && mode === '2d' && fitted !== `${d.map.id}:${floor?.id}`) { fitted = `${d.map.id}:${floor?.id}`; untrack(() => requestAnimationFrame(fit)); } });

  // Grille et règles : pas « rond » pour ~80 px à l'écran.
  const step = $derived.by(() => { const target = 80 / k; for (const s of [100, 200, 500, 1000, 2000, 5000, 10000, 20000, 50000, 100000]) if (s >= target) return s; return 200000; });
  const vis = $derived({ x0: -tx / k, y0: -ty / k, x1: (wrapW - tx) / k, y1: (wrapH - ty) / k });
  const ticksX = $derived.by(() => { const o: number[] = []; for (let x = Math.ceil(vis.x0 / step) * step; x <= vis.x1; x += step) o.push(x); return o.slice(0, 400); });
  const ticksY = $derived.by(() => { const o: number[] = []; for (let y = Math.ceil(vis.y0 / step) * step; y <= vis.y1; y += step) o.push(y); return o.slice(0, 400); });
  const fmtM = (mm: number) => { const m = mm / 1000; return `${Number.isInteger(m) ? m : m.toFixed(1).replace('.', ',')} m`; };
  const scaleBar = $derived.by(() => { for (const s of [500, 1000, 2000, 5000, 10000, 20000, 50000, 100000]) if (s * k >= 90) return s; return 200000; });

  // Calques
  let layers = $state({ bg: true, grid: true, containers: true, points: true, routes: true, texts: true, measures: true });
  let hidden = $state(new Set<string>());
  const toggleHidden = (id: string) => { const s = new Set(hidden); if (s.has(id)) s.delete(id); else s.add(id); hidden = s; };

  // ----- Outils -----
  type Tool = 'select' | 'place' | 'point' | 'text' | 'measure' | 'calib' | 'route';
  let tool = $state<Tool>('select');
  let placing = $state<string | null>(null);
  let sel = $state<string | null>(null);
  let draft = $state<[number, number][]>([]);
  let calibPts = $state<[number, number][]>([]);
  let calibDist = $state<number | null>(null);
  let trace = $state<{ key: string; pts: WP[] } | null>(null);
  let hover = $state<[number, number] | null>(null);
  let pan: { x: number; y: number; tx: number; ty: number } | null = null;
  let drag = $state<{ id: string; dx: number; dy: number; x: number; y: number; moved: boolean } | null>(null);
  const selected = $derived(sel ? byId.get(sel) ?? null : null);
  let remoteSel = $state<Record<string, { member: PresenceMember; ids: string[] }>>({});
  const rsel = $derived(new Map(Object.values(remoteSel).flatMap((r) => r.ids.map((id) => [id, r.member.color] as const))));
  $effect(() => { const ids = sel ? [sel] : []; if (mapId && floor) sendSelection(`project:${pid}/map:${mapId}:${floor.id}`, ids); });

  function setTool(t: Tool) { tool = t; draft = []; calibPts = []; if (t !== 'route') trace = null; if (t !== 'place') placing = null; }
  /** Aimantation orthogonale : à moins de 8° d'un axe, on s'aligne sur le point précédent. */
  function ortho(p: [number, number], prev: [number, number] | null): [number, number] {
    if (!prev) return p;
    const dx = p[0] - prev[0], dy = p[1] - prev[1];
    const a = Math.abs(Math.atan2(dy, dx) * 180 / Math.PI);
    if (a < 8 || a > 172) return [p[0], prev[1]];
    if (Math.abs(a - 90) < 8) return [prev[0], p[1]];
    return p;
  }
  function ends(l: Liaison): { a: WP | null; b: WP | null } {
    const f = (cid: string) => { const o = d?.objects.find((x) => x.kind === 'container' && x.container_id === cid); return o ? { x: o.x, y: o.y, level: levelOf(o.floor_id) } : null; };
    return { a: f(l.a.id), b: f(l.b.id) };
  }
  function prevPoint(): [number, number] | null {
    if (tool === 'measure') return draft.at(-1) ?? null;
    if (tool === 'route' && trace) {
      const last = trace.pts.at(-1);
      if (last && last.level === floor?.level) return [last.x, last.y];
      if (!last) { const l = d?.liaisons.find((x) => x.key === trace!.key); const e = l && ends(l).a; if (e && e.level === floor?.level) return [e.x, e.y]; }
    }
    return null;
  }

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    const w = toWorld(e);
    if (tool === 'select' || !mine) {
      pan = { x: e.clientX, y: e.clientY, tx, ty }; svgEl.setPointerCapture(e.pointerId);
      if (e.target === svgEl || (e.target as Element).classList.contains('bgimg')) sel = null;
      return;
    }
    if (!floor) return;
    lastAction = Date.now();
    if (tool === 'place' && placing) {
      const cid = placing;
      write(() => api.post<{ id: string }>(`/projects/${pid}/plans/${mapId}/objects`, { floor_id: floor!.id, kind: 'container', container_id: cid, x: Math.round(w[0]), y: Math.round(w[1]), z: 0, rot: 0 })).then((r) => { if (r) { sel = r.id; side = 'sel'; } });
      placing = null; tool = 'select';
    } else if (tool === 'point') {
      write(() => api.post<{ id: string }>(`/projects/${pid}/plans/${mapId}/objects`, { floor_id: floor!.id, kind: 'point', x: Math.round(w[0]), y: Math.round(w[1]), z: 300, label: `Point ${(d?.objects.filter((o) => o.kind === 'point').length ?? 0) + 1}`, ports: 4, signal_id: null, connector_id: null })).then((r) => { if (r) { sel = r.id; side = 'sel'; } });
      tool = 'select';
    } else if (tool === 'text') {
      const t = prompt('Texte à poser :'); tool = 'select';
      if (t?.trim()) write(() => api.post<{ id: string }>(`/projects/${pid}/plans/${mapId}/objects`, { floor_id: floor!.id, kind: 'text', x: Math.round(w[0]), y: Math.round(w[1]), text: t, size: Math.round(Math.max(100, 14 / k)) }));
    } else if (tool === 'measure') {
      draft = [...draft, ortho(w, draft.at(-1) ?? null)];
    } else if (tool === 'calib') {
      if (!floor.background) { toast('Posez d’abord un fond de plan sur cet étage.', 'warn'); return; }
      calibPts = [...calibPts, worldToImg(floor, w)];
      if (calibPts.length === 2) calibDist = null;
    } else if (tool === 'route' && trace) {
      const p = ortho(w, prevPoint());
      trace = { ...trace, pts: [...trace.pts, { x: Math.round(p[0]), y: Math.round(p[1]), level: floor.level }] };
    }
  }
  function move(e: PointerEvent) {
    const w = toWorld(e);
    if (mapId && floor) sendCursor(`project:${pid}/map:${mapId}:${floor.id}`, w[0], w[1]);
    hover = tool === 'measure' || tool === 'route' ? ortho(w, prevPoint()) : w;
    if (pan) { tx = pan.tx + e.clientX - pan.x; ty = pan.ty + e.clientY - pan.y; return; }
    if (drag) { const nx = Math.round(w[0] - drag.dx), ny = Math.round(w[1] - drag.dy); drag = { ...drag, x: nx, y: ny, moved: drag.moved || Math.abs(nx - drag.x) + Math.abs(ny - drag.y) > 0 }; }
  }
  function up() {
    pan = null;
    if (drag) {
      const g = drag; drag = null;
      const o = byId.get(g.id);
      if (o && g.moved && mine) {
        const body = { ...o, x: g.x, y: g.y }; delete (body as any).name; d!.objects = d!.objects.map((x) => (x.id === g.id ? { ...x, x: g.x, y: g.y } : x));
        write(() => api.patch(`/projects/${pid}/plans/${mapId}/objects`, clean(body)));
      }
    }
  }
  function objDown(e: PointerEvent, o: Obj) {
    if (tool !== 'select') return;
    e.stopPropagation(); sel = o.id; side = 'sel';
    if (!mine || o.kind === 'measure') return;
    const w = toWorld(e);
    drag = { id: o.id, dx: w[0] - o.x, dy: w[1] - o.y, x: o.x, y: o.y, moved: false };
    svgEl.setPointerCapture(e.pointerId);
  }
  const posOf = (o: Obj) => (drag?.id === o.id ? { x: drag.x, y: drag.y } : { x: o.x, y: o.y });
  /** Corps d'écriture d'un objet : sans les champs calculés par le serveur. */
  function clean(o: Obj): Record<string, unknown> {
    const { name, element_kind, dims, location_id, used, free, color, length_mm, map_id, ...rest } = o as any;
    void name; void element_kind; void dims; void location_id; void used; void free; void color; void length_mm; void map_id;
    return rest;
  }
  async function finishMeasure() {
    if (draft.length >= 2 && floor) await write(() => api.post(`/projects/${pid}/plans/${mapId}/objects`, { floor_id: floor!.id, kind: 'measure', points: draft.map((p) => [Math.round(p[0]), Math.round(p[1])]) }));
    draft = []; tool = 'select';
  }
  async function finishCalib() {
    if (!floor || calibPts.length !== 2 || !calibDist || calibDist <= 0) return;
    const f = floor;
    await write(() => api.patch(`/projects/${pid}/plans/${mapId}/floors`, { id: f.id, level: f.level, name: f.name, height_mm: f.height_mm, ceiling_mm: f.ceiling_mm, bg_opacity: f.bg_opacity, calibration: { a: calibPts[0], b: calibPts[1], dist_mm: calibDist } }));
    calibPts = []; tool = 'select'; fitted = ''; toast('Échelle calibrée.');
  }
  function startTrace(l: Liaison) {
    if (l.state !== 'ready') return;
    tool = 'route'; trace = { key: l.key, pts: [] }; sel = null;
    const e = ends(l).a; if (e) { const f = d?.floors.find((x) => x.level === e.level); if (f) floorId = f.id; }
    toast('Cliquez les points de passage ; changez d’étage pour passer d’un niveau à l’autre. Entrée pour terminer.', 'info');
  }
  async function finishTrace() {
    if (!trace) return;
    const t = trace;
    await write(() => api.put(`/projects/${pid}/plans/${mapId}/routes`, { key: t.key, waypoints: t.pts }));
    trace = null; tool = 'select';
  }
  async function clearRoute(l: Liaison) { await write(() => api.post(`/projects/${pid}/plans/${mapId}/routes/delete`, { key: l.key })); }
  function key(e: KeyboardEvent) {
    if ((e.target as HTMLElement)?.closest('input,textarea,select')) return;
    if (e.key === 'Escape') { setTool('select'); sel = null; }
    if (e.key === 'Enter') { if (tool === 'measure') finishMeasure(); if (tool === 'route') finishTrace(); }
    if ((e.key === 'Delete' || e.key === 'Backspace') && sel && mine) removeSel();
    if (e.key === 'Backspace' && tool === 'route' && trace) trace = { ...trace, pts: trace.pts.slice(0, -1) };
  }
  async function removeSel() {
    if (!selected) return;
    if (!confirm(selected.kind === 'container' ? `Retirer « ${selected.name} » de la carte ?` : 'Supprimer cet objet ?')) return;
    await write(() => api.post(`/projects/${pid}/plans/${mapId}/objects/delete`, { ids: [selected!.id] })); sel = null;
  }
  async function saveSel(patch: Record<string, unknown>) {
    if (!selected) return;
    await write(() => api.patch(`/projects/${pid}/plans/${mapId}/objects`, { ...clean(selected!), ...patch }));
  }
  let linkTarget = $state('');
  async function addLink() { if (!selected || !linkTarget) return; await write(() => api.post(`/projects/${pid}/plans/${mapId}/links`, { point_id: selected!.id, target_id: linkTarget })); linkTarget = ''; }
  async function delLink(l: Link) { await write(() => api.post(`/projects/${pid}/plans/${mapId}/links/delete`, { id: l.id })); }

  // Tracés sur l'étage courant : segments, et montées aux changements de niveau.
  function routeSegs(l: Liaison, pts: WP[]): { segs: [number, number, number, number][]; risers: [number, number, string][] } {
    const e = ends(l); if (!e.a || !e.b || !floor) return { segs: [], risers: [] };
    const all = [e.a, ...pts, e.b];
    const segs: [number, number, number, number][] = [], risers: [number, number, string][] = [];
    for (let i = 0; i < all.length - 1; i++) {
      const p = all[i]!, q = all[i + 1]!;
      if (p.level === floor.level && q.level === floor.level) segs.push([p.x, p.y, q.x, q.y]);
      else if (p.level === floor.level) risers.push([p.x, p.y, q.level > p.level ? '▲' : '▼']);
      else if (q.level === floor.level) { risers.push([p.x, p.y, q.level > p.level ? '▼' : '▲']); segs.push([p.x, p.y, q.x, q.y]); }
    }
    return { segs, risers };
  }
  const lcolor = (l: Liaison) => l.color ?? '#5B636D';
  const legend = $derived.by(() => {
    const m = new Map<string, string>();
    for (const l of d?.liaisons ?? []) if (l.waypoints && l.family) m.set(refName('functions', l.family) || l.family, lcolor(l));
    for (const o of d?.objects ?? []) if (o.kind === 'point' && o.signal_id) m.set(refName('signals', o.signal_id), o.color ?? '#5B636D');
    return [...m.entries()];
  });
  const dimsOf = (o: Obj): [number, number] => { const [w, p] = (o.dims ?? [600, 600, 1000]) as number[]; return o.rot === 90 || o.rot === 270 ? [p!, w!] : [w!, p!]; };

  // ----- Vue 3D (lecture) -----
  let az = $state(-35), el = $state(32);
  let rot3: { x: number; y: number; az: number; el: number } | null = null;
  const proj = (p: [number, number, number]) => {
    const t = (az * Math.PI) / 180, f = (el * Math.PI) / 180;
    const u = p[0] * Math.cos(t) - p[1] * Math.sin(t), v = p[0] * Math.sin(t) + p[1] * Math.cos(t);
    return { x: u, y: v * Math.sin(f) - p[2] * Math.cos(f), z: v * Math.cos(f) + p[2] * Math.sin(f) };
  };
  const scene = $derived.by(() => {
    if (!d || mode !== '3d') return null;
    const base = (fid: string) => d!.floors.find((f) => f.id === fid)?.base_mm ?? 0;
    const [x0, y0, x1, y1] = bounds();
    const polys: { pts: string; z: number; fill: string; stroke: string; op: number; label?: string; lx?: number; ly?: number; start?: boolean }[] = [];
    const raw: { p: { x: number; y: number; z: number }[]; fill: string; stroke: string; op: number; label?: string; corner?: boolean }[] = [];
    for (const f of d.floors) {
      if (f.base_mm == null) continue;
      const z = f.base_mm;
      raw.push({ p: [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]].map((q) => proj(q as [number, number, number])), fill: 'var(--surface)', stroke: 'var(--fg-3)', op: 0.5, label: `${f.name}`, corner: true });
    }
    for (const o of d.objects.filter((o) => o.kind === 'container')) {
      const [w, p] = dimsOf(o), h = (o.dims?.[2] ?? 1000) as number, z0 = base(o.floor_id) + (o.z ?? 0), z1 = z0 + h;
      const c = [[o.x - w / 2, o.y - p / 2], [o.x + w / 2, o.y - p / 2], [o.x + w / 2, o.y + p / 2], [o.x - w / 2, o.y + p / 2]];
      const faces = [c.map(([x, y]) => [x, y, z1]), ...[0, 1, 2, 3].map((i) => [[...c[i]!, z0], [...c[(i + 1) % 4]!, z0], [...c[(i + 1) % 4]!, z1], [...c[i]!, z1]])];
      faces.forEach((fc, i) => raw.push({ p: fc.map((q) => proj(q as [number, number, number])), fill: i === 0 ? 'var(--accent, #4F6BFF)' : 'var(--fg-2)', stroke: 'var(--fg)', op: i === 0 ? 0.55 : 0.28, label: i === 0 ? o.name : undefined }));
    }
    const all = raw.flatMap((r) => r.p);
    for (const l of d.liaisons) if (l.points3d && !hidden.has(l.key)) all.push(...l.points3d.map(proj));
    if (!all.length) return null;
    const minX = Math.min(...all.map((p) => p.x)), maxX = Math.max(...all.map((p) => p.x)), minY = Math.min(...all.map((p) => p.y)), maxY = Math.max(...all.map((p) => p.y));
    const W = 1000, H = 640, s = Math.min((W - 80) / Math.max(maxX - minX, 1), (H - 80) / Math.max(maxY - minY, 1));
    const X = (p: { x: number }) => 40 + (p.x - minX) * s + ((W - 80) - (maxX - minX) * s) / 2, Y = (p: { y: number }) => 40 + (p.y - minY) * s + ((H - 80) - (maxY - minY) * s) / 2;
    for (const r of raw) {
      const cz = r.p.reduce((a, p) => a + p.z, 0) / r.p.length;
      const cx = r.p.reduce((a, p) => a + X(p), 0) / r.p.length, cy = r.p.reduce((a, p) => a + Y(p), 0) / r.p.length;
      const lp = r.corner ? r.p.reduce((m, p) => (X(p) < X(m) ? p : m)) : null;
      polys.push({ pts: r.p.map((p) => `${X(p).toFixed(1)},${Y(p).toFixed(1)}`).join(' '), z: cz - (r.corner ? 1e9 : 0), fill: r.fill, stroke: r.stroke, op: r.op, label: r.label, lx: lp ? X(lp) + 8 : cx, ly: lp ? Y(lp) - 6 : cy, start: !!lp });
    }
    polys.sort((a, b) => a.z - b.z);
    const tubes = d.liaisons.filter((l) => l.points3d && !hidden.has(l.key)).map((l) => ({ key: l.key, color: lcolor(l), pts: l.points3d!.map(proj).map((p) => `${X(p).toFixed(1)},${Y(p).toFixed(1)}`).join(' '), len: l.length_m, name: `${l.a.name} ↔ ${l.b.name}` }));
    return { polys, tubes, W, H };
  });
  function down3(e: PointerEvent) { rot3 = { x: e.clientX, y: e.clientY, az, el }; (e.currentTarget as Element).setPointerCapture(e.pointerId); }
  function move3(e: PointerEvent) { if (!rot3) return; az = rot3.az + (e.clientX - rot3.x) * 0.4; el = Math.min(85, Math.max(5, rot3.el + (e.clientY - rot3.y) * 0.3)); }

  const traceable = $derived((d?.liaisons ?? []).filter((l) => l.state === 'ready'));
  const nf1 = (n: number | null) => (n == null ? '—' : `${n.toLocaleString('fr-FR', { minimumFractionDigits: 1, maximumFractionDigits: 1 })} m`);
</script>

<svelte:window onkeydown={key} />

{#if !idx}<span class="spin"></span>
{:else if !idx.maps.length}
  <section class="card empty-card">
    <h3>Aucune carte</h3>
    <p class="muted">Une carte situe les contenants du projet sur un plan (bâtiment multi-étages, plateau, extérieur), trace les liaisons entre eux et en donne la longueur réelle.</p>
    {#if canDesign}<button class="btn primary" onclick={() => { wiz = { name: view.project.venue ?? 'Site', kind: 'multi_floor', from: 0, to: 0 }; wizErr = null; }}><Icon name="plus" size="sm" />Créer une carte</button>{/if}
  </section>
{:else}
  <section class="card pl-head">
    <div class="pl-maps">
      <select class="input" aria-label="Carte" value={mapId} onchange={(e) => openMap((e.target as HTMLSelectElement).value)}>{#each idx.maps as m}<option value={m.id}>{m.name} · {KINDS[m.kind]}</option>{/each}</select>
      {#if canDesign}<button class="btn sm" onclick={() => { wiz = { name: '', kind: 'multi_floor', from: 0, to: 0 }; wizErr = null; }}><Icon name="plus" size="sm" />Carte</button>{/if}
      {#if mine && d}<button class="icon-btn sm" aria-label="Renommer la carte" onclick={() => (mapEdit = { name: d!.map.name, kind: d!.map.kind })}><Icon name="pen" size="sm" /></button><button class="icon-btn sm" aria-label="Supprimer la carte" onclick={deleteMap}><Icon name="trash" size="sm" /></button>{/if}
    </div>
    <div class="pl-stats small">
      <span><b>{idx.stats.placed}</b>/{idx.stats.containers} éléments posés</span>
      <span><b>{idx.stats.traced}</b>/{idx.stats.liaisons} liaisons tracées</span>
    </div>
    <div class="seg" role="tablist" aria-label="Vue"><button class:on={mode === '2d'} onclick={() => (mode = '2d')}>Plan 2D</button><button class:on={mode === '3d'} onclick={() => (mode = '3d')}>Vue 3D</button></div>
  </section>

  {#if d}
    <div class="card pl" style="padding:0;overflow:hidden">
      <div class="lockbar" class:mine class:other={!mine}>
        {#if mine}
          <span class="lk"><span class="key-i"><Icon name="lock" /></span><span><b>Vous avez la main</b> sur cette carte.</span></span>
          <span class="sp">{#if requested}<span class="chip warn">{requested} demande la main</span>{/if}<button class="btn sm" onclick={async () => { requested = null; await release(); await load(); }}>Céder la main</button></span>
        {:else if d.lock}
          <span class="lk"><span class="key-i"><Icon name="lock" /></span><span><b>{d.lock.account_name}</b> a la main depuis {relative(d.lock.taken_at)}. Vous voyez la dernière version enregistrée.</span></span>
          <span class="sp">{#if canDesign}<button class="btn sm" onclick={requestHand}>Demander la main</button>{/if}</span>
        {:else}
          <span class="lk"><span class="key-i"><Icon name="unlock" /></span><span>Personne ne modifie cette carte.</span></span>
          <span class="sp">{#if canDesign}<button class="btn sm primary" onclick={take}>Prendre la main</button>{:else}<span class="muted small">Lecture seule : votre rôle ne permet pas de concevoir.</span>{/if}</span>
        {/if}
      </div>
      <div class="dtool">
        <div class="seg floors" role="tablist" aria-label="Étages">
          {#each [...d.floors].reverse() as f (f.id)}<button role="tab" aria-selected={floor?.id === f.id} class:on={floor?.id === f.id} onclick={() => (floorId = f.id)}><span class="mono lv">{f.level > 0 ? `+${f.level}` : f.level}</span>{f.name}</button>{/each}
        </div>
        {#if floor}<button class="btn sm ghost" onclick={() => (floorEdit = { ...floor! })}><Icon name="sliders" size="sm" />Étage</button>{/if}
        {#if mine && d.map.kind === 'multi_floor' && freeLevels.length}
          <select class="input sm" aria-label="Ajouter un étage" value={newLevel ?? ''} onchange={(e) => { const v = (e.target as HTMLSelectElement).value; newLevel = v === '' ? null : Number(v); addFloor(); }}><option value="">+ Étage…</option>{#each freeLevels as l}<option value={l}>Niveau {l > 0 ? `+${l}` : l}</option>{/each}</select>
        {/if}
        <span style="flex:1"></span>
        {#if mode === '2d' && mine}
          <div class="seg tools" role="toolbar" aria-label="Outils">
            {#each [['select', 'Sélection', 'hand'], ['point', 'Point de connexion', 'pin'], ['text', 'Texte', 'pen'], ['measure', 'Mesure', 'hash'], ['calib', 'Calibrer', 'scan']] as [t, l, ic]}
              <button class:on={tool === t} title={l} aria-label={l} onclick={() => setTool(t as Tool)}><Icon name={ic} size="sm" /><span class="tl">{l}</span></button>
            {/each}
          </div>
        {/if}
        {#if mode === '2d'}<button class="icon-btn sm" aria-label="Recadrer" onclick={fit}><Icon name="fit" /></button>{/if}
      </div>
      {#if tool === 'route' && trace}
        {@const l = d.liaisons.find((x) => x.key === trace!.key)}
        <div class="hint">Tracé <b>{l?.a.name} ↔ {l?.b.name}</b> · {trace.pts.length} point{trace.pts.length > 1 ? 's' : ''} · aimantation orthogonale · changez d’étage pour monter ou descendre. <button class="btn sm primary" onclick={finishTrace}>Terminer</button><button class="btn sm" disabled={!trace.pts.length} onclick={() => (trace = { ...trace!, pts: trace!.pts.slice(0, -1) })}>Retirer le dernier</button><button class="btn sm ghost" onclick={() => setTool('select')}>Annuler</button></div>
      {:else if tool === 'measure'}
        <div class="hint">Mesure : cliquez les points ; <b>{fmtM(draft.reduce((a, p, i) => (i ? a + Math.hypot(p[0] - draft[i - 1]![0], p[1] - draft[i - 1]![1]) : 0), 0))}</b>. <button class="btn sm primary" disabled={draft.length < 2} onclick={finishMeasure}>Terminer</button><button class="btn sm ghost" onclick={() => setTool('select')}>Annuler</button></div>
      {:else if tool === 'calib'}
        <div class="hint">Calibration : cliquez deux points du fond ({calibPts.length}/2), puis donnez leur distance réelle. Les mêmes deux points physiques servent de repère commun à tous les étages.
          {#if calibPts.length === 2}<label class="inl">Distance <input class="input sm mono" type="number" min="1" bind:value={calibDist} placeholder="mm" aria-label="Distance réelle en mm"> mm</label><button class="btn sm primary" disabled={!calibDist} onclick={finishCalib}>Calibrer</button>{/if}
          <button class="btn sm ghost" onclick={() => setTool('select')}>Annuler</button></div>
      {:else if tool === 'place' && placing}
        <div class="hint">Cliquez sur le plan pour poser <b>{d.unplaced.find((c) => c.id === placing)?.name}</b>. <button class="btn sm ghost" onclick={() => setTool('select')}>Annuler</button></div>
      {:else if floor && floor.background && !floor.calibration}
        <div class="hint warn">Fond non calibré : l’échelle est approximative. {#if mine}<button class="btn sm" onclick={() => setTool('calib')}>Calibrer</button>{/if}</div>
      {/if}

      <div class="pl-body">
        {#if mode === '2d'}
          <div class="pl-canvas" bind:clientWidth={wrapW} bind:clientHeight={wrapH}>
            <svg bind:this={svgEl} class="pl-svg" class:crosshair={tool !== 'select'} role="application" aria-label="Plan de l’étage" onwheel={wheel} onpointerdown={down} onpointermove={move} onpointerup={up} onpointerleave={() => (hover = null)} ondblclick={() => { if (tool === 'measure') finishMeasure(); else if (tool === 'route') finishTrace(); }}>
              <g transform="translate({tx} {ty}) scale({k})">
                {#if floor && layers.bg && floor.background}
                  <image class="bgimg" href="/api/projects/{pid}/plans/files/{floor.background}" width={floor.bg_w} height={floor.bg_h} transform={bgMatrix(floor)} opacity={floor.bg_opacity} preserveAspectRatio="none" />
                {/if}
                {#if layers.grid}
                  {#each ticksX as x}<line x1={x} y1={vis.y0} x2={x} y2={vis.y1} class="grid" class:axis={x === 0} vector-effect="non-scaling-stroke" />{/each}
                  {#each ticksY as y}<line x1={vis.x0} y1={y} x2={vis.x1} y2={y} class="grid" class:axis={y === 0} vector-effect="non-scaling-stroke" />{/each}
                {/if}
                {#if layers.measures}
                  {#each objs.filter((o) => o.kind === 'measure' && !hidden.has(o.id)) as o (o.id)}
                    <g class="obj" class:sel={sel === o.id} role="button" tabindex="-1" onpointerdown={(e) => objDown(e, o)}>
                      <polyline points={o.points.map((p: number[]) => p.join(',')).join(' ')} class="meas" vector-effect="non-scaling-stroke" />
                      {#each o.points as p}<circle cx={p[0]} cy={p[1]} r={3 / k} class="meas-pt" />{/each}
                      <text x={o.points.at(-1)[0] + 6 / k} y={o.points.at(-1)[1] - 6 / k} font-size={12 / k} class="meas-t">{fmtM(o.length_mm)}</text>
                    </g>
                  {/each}
                {/if}
                {#if layers.routes}
                  {#each d.liaisons.filter((l) => l.state === 'ready' && l.waypoints && !hidden.has(l.key) && trace?.key !== l.key) as l (l.key)}
                    {@const r = routeSegs(l, l.waypoints!)}
                    {#each r.segs as s}<line x1={s[0]} y1={s[1]} x2={s[2]} y2={s[3]} stroke={lcolor(l)} stroke-width={4 / k} stroke-linecap="round" opacity=".85"><title>{l.a.name} ↔ {l.b.name} · {nf1(l.length_m)}</title></line>{/each}
                    {#each r.risers as rs}<g transform="translate({rs[0]} {rs[1]})"><circle r={9 / k} fill="var(--surface)" stroke={lcolor(l)} stroke-width={2 / k} /><text font-size={10 / k} text-anchor="middle" dy={3.5 / k} fill={lcolor(l)}>{rs[2]}</text></g>{/each}
                  {/each}
                  {#if trace}
                    {@const l = d.liaisons.find((x) => x.key === trace!.key)}
                    {#if l}
                      {@const r = routeSegs(l, trace.pts)}
                      {#each r.segs as s}<line x1={s[0]} y1={s[1]} x2={s[2]} y2={s[3]} stroke={lcolor(l)} stroke-width={4 / k} stroke-dasharray="{10 / k} {6 / k}" />{/each}
                      {#each trace.pts.filter((p) => p.level === floor?.level) as p}<circle cx={p.x} cy={p.y} r={5 / k} fill={lcolor(l)} />{/each}
                      {@const pv = prevPoint()}
                      {#if pv && hover}<line x1={pv[0]} y1={pv[1]} x2={hover[0]} y2={hover[1]} stroke={lcolor(l)} stroke-width={2 / k} stroke-dasharray="{4 / k} {4 / k}" />{/if}
                    {/if}
                  {/if}
                {/if}
                {#if layers.points}
                  {#each d.links as ln (ln.id)}
                    {@const a = byId.get(ln.point_id)}{@const b = byId.get(ln.target_id)}
                    {#if a && b && a.floor_id === floor?.id && b.floor_id === floor?.id}<line x1={posOf(a).x} y1={posOf(a).y} x2={posOf(b).x} y2={posOf(b).y} stroke={a.color ?? 'var(--fg-2)'} stroke-width={2 / k} stroke-dasharray="{6 / k} {4 / k}" />{/if}
                  {/each}
                {/if}
                {#if layers.containers}
                  {#each objs.filter((o) => o.kind === 'container' && !hidden.has(o.container_id)) as o (o.id)}
                    {@const p = posOf(o)}{@const [w, h] = dimsOf(o)}
                    <g class="obj ct" class:sel={sel === o.id} class:rsel={rsel.has(o.id)} style="--rs:{rsel.get(o.id) ?? 'transparent'}" transform="translate({p.x} {p.y})" role="button" tabindex="-1" onpointerdown={(e) => objDown(e, o)}>
                      <rect x={-w / 2} y={-h / 2} width={w} height={h} rx={Math.min(w, h) * 0.06} class="ctr" vector-effect="non-scaling-stroke" />
                      <text y={h / 2 + 14 / k} font-size={12 / k} text-anchor="middle" class="ct-t">{o.name}</text>
                    </g>
                  {/each}
                {/if}
                {#if layers.points}
                  {#each objs.filter((o) => o.kind === 'point' && !hidden.has(o.id)) as o (o.id)}
                    {@const p = posOf(o)}
                    <g class="obj" class:sel={sel === o.id} transform="translate({p.x} {p.y})" role="button" tabindex="-1" onpointerdown={(e) => objDown(e, o)}>
                      <rect x={-7 / k} y={-7 / k} width={14 / k} height={14 / k} rx={3 / k} fill={o.color ?? 'var(--fg-2)'} stroke="var(--surface)" stroke-width={2 / k} />
                      <text x={10 / k} y={4 / k} font-size={11 / k} class="pt-t">{o.label} <tspan class="muted">{o.free}/{o.ports}</tspan></text>
                    </g>
                  {/each}
                {/if}
                {#if layers.texts}
                  {#each objs.filter((o) => o.kind === 'text' && !hidden.has(o.id)) as o (o.id)}
                    {@const p = posOf(o)}
                    <text x={p.x} y={p.y} font-size={o.size} class="obj tx" class:sel={sel === o.id} role="button" tabindex="-1" onpointerdown={(e) => objDown(e, o)}>{o.text}</text>
                  {/each}
                {/if}
                {#if tool === 'measure' && draft.length}
                  <polyline points={[...draft, ...(hover ? [hover] : [])].map((p) => p.join(',')).join(' ')} class="meas draft" vector-effect="non-scaling-stroke" />
                {/if}
                {#if tool === 'calib' && floor}
                  {#each calibPts as c}{@const w = (() => { const f = floor!; if (!f.calibration) return [c[0] * 10, c[1] * 10]; const cc = f.calibration, dx = cc.b[0] - cc.a[0], dy = cc.b[1] - cc.a[1], s = cc.dist_mm / Math.hypot(dx, dy), an = Math.atan2(dy, dx), px = c[0] - cc.a[0], py = c[1] - cc.a[1]; return [(px * Math.cos(an) + py * Math.sin(an)) * s, (-px * Math.sin(an) + py * Math.cos(an)) * s]; })()}<circle cx={w[0]} cy={w[1]} r={6 / k} class="calib" />{/each}
                {/if}
              </g>
            </svg>
            <!-- Règles graduées en mètres -->
            {#if mapId && floor}<RemoteCursors room={`project:${pid}/map:${mapId}:${floor.id}`} toScreen={(x, y) => [tx + x * k, ty + y * k]} bind:selections={remoteSel} />{/if}
            <svg class="ruler top" aria-hidden="true"><rect width="100%" height="18" class="rbg" />{#each ticksX as x}<line x1={tx + x * k} x2={tx + x * k} y1="10" y2="18" class="rt" /><text x={tx + x * k + 3} y="11" class="rl">{fmtM(x)}</text>{/each}</svg>
            <svg class="ruler left" aria-hidden="true"><rect width="18" height="100%" class="rbg" />{#each ticksY as y}<line y1={ty + y * k} y2={ty + y * k} x1="10" x2="18" class="rt" /><text transform="translate(11 {ty + y * k - 3}) rotate(-90)" class="rl">{fmtM(y)}</text>{/each}</svg>
            <div class="scalebar"><i style="width:{scaleBar * k}px"></i><span>{fmtM(scaleBar)}</span></div>
            {#if legend.length}<div class="legend">{#each legend as [n, c]}<span><i style="background:{c}"></i>{n}</span>{/each}</div>{/if}
          </div>
        {:else}
          <div class="pl-canvas">
            {#if scene}
              <svg viewBox="0 0 {scene.W} {scene.H}" class="pl-3d" role="img" aria-label="Vue 3D des étages, contenants et tracés" onpointerdown={down3} onpointermove={move3} onpointerup={() => (rot3 = null)}>
                {#each scene.polys as p}<polygon points={p.pts} fill={p.fill} fill-opacity={p.op} stroke={p.stroke} stroke-opacity=".8" stroke-width={p.start ? 1.5 : 1} />{#if p.label}<text x={p.lx} y={p.ly} text-anchor={p.start ? 'start' : 'middle'} class="l3" class:fl={p.start}>{p.label}</text>{/if}{/each}
                {#each scene.tubes as t (t.key)}<polyline points={t.pts} fill="none" stroke={t.color} stroke-width="5" stroke-linecap="round" stroke-linejoin="round" opacity=".9"><title>{t.name} · {nf1(t.len)}</title></polyline>{/each}
              </svg>
              <div class="hint3 small muted">Glissez pour tourner · lecture seule</div>
            {:else}<p class="muted" style="padding:24px">Rien à montrer : posez des contenants et vérifiez que les étages se suivent.</p>{/if}
          </div>
        {/if}

        <aside class="pl-side">
          <div class="seg side-tabs" role="tablist" aria-label="Panneau">
            {#each [['place', `À placer ${d.unplaced.length}`], ['trace', `À tracer ${traceable.filter((l) => !l.waypoints).length}`], ['layers', 'Calques'], ['sel', 'Sélection']] as [s, l]}<button role="tab" aria-selected={side === s} class:on={side === s} onclick={() => (side = s as typeof side)}>{l}</button>{/each}
          </div>
          {#if side === 'place'}
            {#if !d.unplaced.length}<p class="muted small">Tous les éléments du projet sont posés.</p>{/if}
            {#each d.unplaced as c (c.id)}
              <button class="li" class:on={placing === c.id} disabled={!mine || mode === '3d'} onclick={() => { setTool('place'); placing = c.id; }}><b>{c.name}</b><span class="muted small">{Math.round(c.dims[0])} × {Math.round(c.dims[1])} × {Math.round(c.dims[2])} mm</span></button>
            {/each}
            {#if d.objects.some((o) => o.kind === 'container')}
              <h4>Posés sur cette carte</h4>
              {#each d.objects.filter((o) => o.kind === 'container') as o (o.id)}
                <button class="li" onclick={() => { floorId = o.floor_id; sel = o.id; side = 'sel'; }}><b>{o.name}</b><span class="muted small">{d.floors.find((f) => f.id === o.floor_id)?.name}</span></button>
              {/each}
            {/if}
          {:else if side === 'trace'}
            {#if !d.liaisons.length}<p class="muted small">Aucune liaison entre contenants posés sur cette carte. Les liaisons viennent des câbles du synoptique.</p>{/if}
            {#each d.liaisons as l (l.key)}
              <div class="li tr">
                <span class="nm"><i class="dot" style="--c:{lcolor(l)}"></i><b>{l.a.name ?? '?'} ↔ {l.b.name ?? '?'}</b></span>
                <span class="muted small">{l.count} câble{l.count > 1 ? 's' : ''}{l.numbers.length ? ` · ${l.numbers.slice(0, 3).join(', ')}${l.numbers.length > 3 ? '…' : ''}` : ''}</span>
                {#if l.state === 'ready'}
                  <span class="small">{l.waypoints ? `Tracé · ${nf1(l.length_m)}` : 'À tracer'}</span>
                  {#if mine && mode === '2d'}<span class="ra"><button class="btn sm" onclick={() => startTrace(l)}>{l.waypoints ? 'Retracer' : 'Tracer'}</button>{#if l.waypoints}<button class="btn sm ghost" onclick={() => clearRoute(l)}>Effacer</button>{/if}</span>{/if}
                {:else}
                  <span class="small warn-t">{l.state === 'other_map' ? 'Contenants sur deux cartes' : 'Un contenant n’est pas posé'}</span>
                {/if}
              </div>
            {/each}
          {:else if side === 'layers'}
            {#each [['bg', 'Fond de plan'], ['grid', 'Grille'], ['containers', 'Contenants'], ['points', 'Points de connexion et liens'], ['routes', 'Tracés'], ['texts', 'Textes'], ['measures', 'Mesures']] as [kk, l]}
              <label class="toggle-row small"><input type="checkbox" bind:checked={(layers as any)[kk]}>{l}</label>
            {/each}
            {#if floor?.background}<label class="field small">Plan estompé<input type="range" min="0.05" max="1" step="0.05" value={floor.bg_opacity} disabled={!mine} onchange={(e) => write(() => api.patch(`/projects/${pid}/plans/${mapId}/floors`, { ...floor, bg_opacity: Number((e.target as HTMLInputElement).value) }))}></label>{/if}
            <h4>Par contenant</h4>
            {#each d.objects.filter((o) => o.kind === 'container') as o (o.id)}<label class="toggle-row small"><input type="checkbox" checked={!hidden.has(o.container_id)} onchange={() => toggleHidden(o.container_id)}>{o.name}</label>{/each}
            <h4>Par liaison</h4>
            {#each d.liaisons.filter((l) => l.waypoints) as l (l.key)}<label class="toggle-row small"><input type="checkbox" checked={!hidden.has(l.key)} onchange={() => toggleHidden(l.key)}><i class="dot" style="--c:{lcolor(l)}"></i>{l.a.name} ↔ {l.b.name}</label>{/each}
          {:else}
            {#if !selected}<p class="muted small">Choisissez un objet sur le plan.</p>
            {:else}
              {@const ro = !mine}
              {#if selected.kind === 'container'}
                <h4>{selected.name}</h4>
                <div class="fg2">
                  <label class="field">X (mm)<input class="input sm mono" type="number" value={Math.round(selected.x)} disabled={ro} onchange={(e) => saveSel({ x: Number((e.target as HTMLInputElement).value) })}></label>
                  <label class="field">Y (mm)<input class="input sm mono" type="number" value={Math.round(selected.y)} disabled={ro} onchange={(e) => saveSel({ y: Number((e.target as HTMLInputElement).value) })}></label>
                  <label class="field">Hauteur au sol (mm)<input class="input sm mono" type="number" min="0" value={selected.z} disabled={ro} onchange={(e) => saveSel({ z: Number((e.target as HTMLInputElement).value) })}></label>
                  <label class="field">Rotation<select class="input sm" value={selected.rot} disabled={ro} onchange={(e) => saveSel({ rot: Number((e.target as HTMLSelectElement).value) })}>{#each [0, 90, 180, 270] as r}<option value={r}>{r}°</option>{/each}</select></label>
                </div>
                <label class="field">Étage<select class="input sm" value={selected.floor_id} disabled={ro} onchange={(e) => { const f = (e.target as HTMLSelectElement).value; saveSel({ floor_id: f }); floorId = f; }}>{#each d.floors as f}<option value={f.id}>{f.name}</option>{/each}</select></label>
              {:else if selected.kind === 'point'}
                <label class="field">Libellé<input class="input sm" value={selected.label} disabled={ro} maxlength="60" onchange={(e) => saveSel({ label: (e.target as HTMLInputElement).value })}></label>
                <label class="field">Signal (verrouille les liens)<select class="input sm" value={selected.signal_id ?? ''} disabled={ro} onchange={(e) => saveSel({ signal_id: (e.target as HTMLSelectElement).value || null })}><option value="">—</option>{#each refList('signals', true) as s}<option value={s.id}>{s.name}</option>{/each}</select></label>
                <label class="field">Connecteur<select class="input sm" value={selected.connector_id ?? ''} disabled={ro} onchange={(e) => saveSel({ connector_id: (e.target as HTMLSelectElement).value || null })}><option value="">—</option>{#each refList('connectors', true).filter((c) => !selected!.signal_id || (refById('signals', selected!.signal_id)?.data?.connectors ?? []).includes(c.id)) as c}<option value={c.id}>{c.name}</option>{/each}</select></label>
                <div class="fg2">
                  <label class="field">Ports<input class="input sm mono" type="number" min="1" max="256" value={selected.ports} disabled={ro} onchange={(e) => saveSel({ ports: Number((e.target as HTMLInputElement).value) })}></label>
                  <label class="field">Hauteur (mm)<input class="input sm mono" type="number" min="0" value={selected.z} disabled={ro} onchange={(e) => saveSel({ z: Number((e.target as HTMLInputElement).value) })}></label>
                </div>
                <p class="small">Capacité libre : <b>{selected.free}</b> sur {selected.ports}</p>
                {#each d.links.filter((l) => l.point_id === selected!.id) as ln (ln.id)}
                  {@const t = byId.get(ln.target_id)}
                  <div class="lnk small"><span>→ {t?.kind === 'container' ? t.name : t?.label}</span>{#if !ro}<button class="icon-btn sm" aria-label="Retirer le lien" onclick={() => delLink(ln)}><Icon name="x" size="sm" /></button>{/if}</div>
                {/each}
                {#if !ro && selected.free > 0}
                  <div class="lnk"><select class="input sm" bind:value={linkTarget} aria-label="Cible du lien"><option value="">Lier à…</option>{#each d.objects.filter((o) => (o.kind === 'container' || o.kind === 'point') && o.id !== selected!.id) as o}<option value={o.id}>{o.kind === 'container' ? o.name : `Point ${o.label}`}</option>{/each}</select><button class="btn sm" disabled={!linkTarget} onclick={addLink}>Lier</button></div>
                {/if}
              {:else if selected.kind === 'text'}
                <label class="field">Texte<textarea class="input" rows="3" disabled={ro} onchange={(e) => saveSel({ text: (e.target as HTMLTextAreaElement).value })}>{selected.text}</textarea></label>
                <label class="field">Taille (mm)<input class="input sm mono" type="number" min="20" value={selected.size} disabled={ro} onchange={(e) => saveSel({ size: Number((e.target as HTMLInputElement).value) })}></label>
              {:else}
                <h4>Mesure</h4><p><b>{fmtM(selected.length_mm)}</b> · {selected.points.length} points</p>
              {/if}
              {#if !ro}<button class="btn sm ghost danger" onclick={removeSel}><Icon name="trash" size="sm" />{selected.kind === 'container' ? 'Retirer de la carte' : 'Supprimer'}</button>{/if}
            {/if}
          {/if}
        </aside>
      </div>
    </div>
  {/if}
{/if}

{#if wiz}
  <Dialog title="Nouvelle carte" eyebrow="Cartographie" onclose={() => (wiz = null)}>
    <div class="field"><label for="w-name">Nom</label><input class="input" id="w-name" bind:value={wiz.name} maxlength="80" placeholder="Stade, bâtiment A, parvis…">{#if wizErr?.field('name')}<span class="ferr">{wizErr.field('name')}</span>{/if}</div>
    <div class="field"><span class="flabel">Type</span><div class="seg">{#each Object.entries(KINDS) as [kk, l]}<button type="button" class:on={wiz.kind === kk} onclick={() => (wiz!.kind = kk as Kind)}>{l}</button>{/each}</div></div>
    {#if wiz.kind === 'multi_floor'}
      <div class="fg2">
        <label class="field">Du niveau<select class="input" bind:value={wiz.from}>{#each Array.from({ length: 13 }, (_, i) => i - 2) as l}<option value={l}>{l > 0 ? `+${l}` : l}</option>{/each}</select></label>
        <label class="field">Au niveau<select class="input" bind:value={wiz.to}>{#each Array.from({ length: 13 }, (_, i) => i - 2) as l}<option value={l}>{l > 0 ? `+${l}` : l}</option>{/each}</select></label>
      </div>
      <p class="muted small">{Math.abs(wiz.to - wiz.from) + 1} étage{wiz.to !== wiz.from ? 's' : ''}, 3 m entre sols et faux-plafond à 2,5 m par défaut. Le niveau d’un étage ne change plus ensuite.</p>
    {:else}<p class="muted small">Un seul niveau ({wiz.kind === 'outdoor' ? 'extérieur' : 'plan unique'}).</p>{/if}
    {#snippet footer()}<button class="btn" onclick={() => (wiz = null)}>Annuler</button><button class="btn primary" disabled={!wiz?.name.trim()} onclick={createMap}>Créer la carte</button>{/snippet}
  </Dialog>
{/if}

{#if mapEdit}
  <Dialog title="Carte" eyebrow="Cartographie" onclose={() => (mapEdit = null)}>
    <div class="field"><label for="m-name">Nom</label><input class="input" id="m-name" bind:value={mapEdit.name} maxlength="80"></div>
    <div class="field"><span class="flabel">Type</span><div class="seg">{#each Object.entries(KINDS) as [kk, l]}<button type="button" class:on={mapEdit.kind === kk} onclick={() => (mapEdit!.kind = kk as Kind)}>{l}</button>{/each}</div></div>
    {#snippet footer()}<button class="btn" onclick={() => (mapEdit = null)}>Annuler</button><button class="btn primary" onclick={saveMap}>Enregistrer</button>{/snippet}
  </Dialog>
{/if}

{#if floorEdit}
  {@const ro = !mine}
  <Dialog title={floorEdit.name} eyebrow="Étage · niveau {floorEdit.level > 0 ? `+${floorEdit.level}` : floorEdit.level}" onclose={() => (floorEdit = null)}>
    <div class="field"><label for="f-name">Nom</label><input class="input" id="f-name" bind:value={floorEdit.name} maxlength="80" disabled={ro}></div>
    <div class="fg2">
      <label class="field">Hauteur entre sols (mm)<input class="input mono" type="number" min="1" max="20000" bind:value={floorEdit.height_mm} disabled={ro}></label>
      <label class="field">Faux-plafond ou chemin de câbles (mm)<input class="input mono" type="number" min="0" bind:value={floorEdit.ceiling_mm} disabled={ro}></label>
    </div>
    <div class="field">
      <span class="flabel">Fond de plan</span>
      {#if floorEdit.background}
        <p class="small">{floorEdit.bg_type?.replace('image/', '').toUpperCase()} · {floorEdit.bg_w} × {floorEdit.bg_h} px · {floorEdit.calibration ? `calibré (${(floorEdit.calibration.dist_mm / Math.hypot(floorEdit.calibration.b[0] - floorEdit.calibration.a[0], floorEdit.calibration.b[1] - floorEdit.calibration.a[1])).toFixed(1).replace('.', ',')} mm par pixel)` : 'non calibré'}</p>
      {:else}<p class="muted small">Aucun. PNG, JPG ou SVG, 15 Mo au plus.</p>{/if}
      {#if !ro}
        <div class="row-btns">
          <label class="btn sm"><Icon name="upload" size="sm" />{floorEdit.background ? 'Remplacer' : 'Choisir un fichier'}<input type="file" accept="image/png,image/jpeg,image/svg+xml" hidden onchange={(e) => { const f = (e.target as HTMLInputElement).files?.[0]; if (f) uploadBg(floorEdit!, f); }}></label>
          {#if floorEdit.background}<button class="btn sm" onclick={() => { const f = floorEdit!; floorEdit = null; floorId = f.id; mode = '2d'; setTool('calib'); }}>Calibrer</button><button class="btn sm ghost danger" onclick={() => removeBg(floorEdit!)}>Retirer</button>{/if}
        </div>
      {/if}
    </div>
    {#snippet footer()}
      {#if !ro && (d?.floors.length ?? 0) > 1}<button class="btn ghost danger" style="margin-right:auto" onclick={() => deleteFloor(floorEdit!)}>Supprimer l’étage</button>{/if}
      <button class="btn" onclick={() => (floorEdit = null)}>Fermer</button>{#if !ro}<button class="btn primary" onclick={saveFloor}>Enregistrer</button>{/if}
    {/snippet}
  </Dialog>
{/if}

<style>
  .pl-head { padding: 12px 16px; display: flex; gap: 16px; align-items: center; flex-wrap: wrap; margin-bottom: 12px; }
  .pl-maps { display: flex; gap: 8px; align-items: center; }
  .pl-maps select { width: auto; min-width: 220px; height: 40px; }
  .pl-stats { display: flex; gap: 16px; flex: 1; flex-wrap: wrap; color: var(--fg-2); }
  .pl-stats b { color: var(--fg); }
  .dtool { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; padding: 10px 16px; border-top: 1px solid var(--line); }
  .dtool select.sm { width: auto; height: 34px; }
  .floors { max-width: 100%; overflow-x: auto; }
  .floors button { white-space: nowrap; }
  .lv { font-size: 11px; opacity: .7; margin-right: 6px; }
  .tools button { display: inline-flex; gap: 6px; align-items: center; }
  @media (max-width: 1300px) { .tl { display: none; } }
  .hint { padding: 8px 16px; background: color-mix(in srgb, var(--accent, #4F6BFF) 8%, var(--surface)); border-top: 1px solid var(--line); display: flex; gap: 10px; align-items: center; flex-wrap: wrap; font-size: 14px; }
  .hint.warn { background: color-mix(in srgb, var(--warn, #B78400) 12%, var(--surface)); }
  .hint .inl { display: inline-flex; gap: 6px; align-items: center; }
  .hint .input.sm { width: 110px; height: 32px; }
  .pl-body { display: grid; grid-template-columns: minmax(0, 1fr) 300px; border-top: 1px solid var(--line); height: calc(100vh - 260px); min-height: 520px; }
  .pl-canvas { position: relative; overflow: hidden; background: var(--surface-2); }
  .pl-svg { width: 100%; height: 100%; display: block; touch-action: none; cursor: grab; user-select: none; }
  .pl-svg.crosshair { cursor: crosshair; }
  .grid { stroke: var(--line); stroke-width: 1; }
  .grid.axis { stroke: var(--fg-3); }
  .obj { cursor: pointer; }
  .ctr { fill: color-mix(in srgb, var(--accent, #4F6BFF) 16%, var(--surface)); stroke: var(--fg); stroke-width: 1.5; }
  .obj.rsel .ctr { stroke: var(--rs); stroke-width: 3; }
  .obj.sel .ctr { stroke: var(--accent, #4F6BFF); stroke-width: 3; }
  .ct-t, .pt-t { font-family: var(--font); font-weight: 600; fill: var(--fg); paint-order: stroke; stroke: var(--surface); stroke-width: 3px; stroke-linejoin: round; }
  .pt-t .muted { fill: var(--fg-2); font-weight: 500; }
  .obj.sel .pt-t { fill: var(--accent, #4F6BFF); }
  .tx { font-family: var(--font); fill: var(--fg); }
  .tx.sel { fill: var(--accent, #4F6BFF); }
  .meas { fill: none; stroke: #CF3A2F; stroke-width: 1.5; stroke-dasharray: 6 3; }
  .meas.draft { stroke-dasharray: 3 3; }
  .meas-pt { fill: #CF3A2F; }
  .meas-t { fill: #CF3A2F; font-family: var(--font); font-weight: 700; paint-order: stroke; stroke: var(--surface); stroke-width: 3px; }
  .calib { fill: none; stroke: #CF3A2F; stroke-width: 2; vector-effect: non-scaling-stroke; }
  .ruler { position: absolute; pointer-events: none; overflow: hidden; }
  .ruler.top { left: 0; top: 0; width: 100%; height: 18px; }
  .ruler.left { left: 0; top: 0; width: 18px; height: 100%; }
  .rbg { fill: color-mix(in srgb, var(--surface) 88%, transparent); }
  .rt { stroke: var(--fg-2); stroke-width: 1; }
  .rl { font: 600 9.5px var(--font-mono, monospace); fill: var(--fg-2); }
  .scalebar { position: absolute; left: 30px; bottom: 14px; display: flex; flex-direction: column; gap: 3px; font: 600 11px var(--font-mono, monospace); color: var(--fg-2); pointer-events: none; }
  .scalebar i { display: block; height: 6px; border: 2px solid var(--fg); border-top: 0; }
  .legend { position: absolute; right: 12px; bottom: 12px; background: var(--surface); border: 1px solid var(--line); border-radius: 10px; padding: 8px 10px; display: grid; gap: 4px; font-size: 12.5px; }
  .legend span { display: flex; gap: 8px; align-items: center; }
  .legend i { width: 18px; height: 4px; border-radius: 2px; }
  .pl-3d { width: 100%; height: 100%; display: block; cursor: grab; touch-action: none; }
  .l3.fl { font-size: 13px; fill: var(--fg-2); }
  .l3 { font: 600 11px var(--font); fill: var(--fg); paint-order: stroke; stroke: var(--surface); stroke-width: 3px; }
  .hint3 { position: absolute; left: 14px; bottom: 10px; }
  .pl-side { border-left: 1px solid var(--line); padding: 12px; overflow: auto; display: flex; flex-direction: column; gap: 6px; background: var(--surface); }
  .side-tabs { flex-wrap: wrap; margin-bottom: 6px; }
  .side-tabs button { font-size: 12.5px; padding: 6px 8px; }
  .pl-side h4 { margin: 12px 0 2px; font-size: 13px; text-transform: uppercase; letter-spacing: .04em; color: var(--fg-2); }
  .li { display: flex; flex-direction: column; gap: 2px; text-align: left; padding: 8px 10px; border-radius: 10px; border: 1px solid var(--line); background: var(--surface); font: inherit; color: inherit; cursor: pointer; }
  .li:hover:not(:disabled) { border-color: var(--fg-3); }
  .li.on { border-color: var(--accent, #4F6BFF); box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent, #4F6BFF) 25%, transparent); }
  .li.tr { cursor: default; }
  .li .nm { display: flex; gap: 6px; align-items: center; }
  .ra { display: flex; gap: 6px; margin-top: 4px; }
  .warn-t { color: var(--warn, #B78400); }
  .dot { display: inline-block; width: 10px; height: 10px; border-radius: 50%; background: var(--c); flex: none; }
  .lnk { display: flex; gap: 6px; align-items: center; justify-content: space-between; }
  .lnk select { flex: 1; }
  .field { display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
  .input.sm { height: 34px; }
  .row-btns { display: flex; gap: 8px; flex-wrap: wrap; }
  .ferr { color: var(--err); font-size: 13px; }
  .empty-card { padding: 28px; text-align: center; display: grid; gap: 10px; justify-items: center; }
  @media (max-width: 1000px) { .pl-body { grid-template-columns: 1fr; height: auto; } .pl-canvas { height: 64vh; } .pl-side { border-left: 0; border-top: 1px solid var(--line); } }
</style>
