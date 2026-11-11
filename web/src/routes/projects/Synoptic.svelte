<script lang="ts">
  // Synoptique (section 10) : nœuds = exemplaires d'équipements, rangées de panels et unités
  // d'adaptateurs ; entrées à gauche, sorties à droite (panel : arrière à gauche, avant à
  // droite). On câble en cliquant un port puis un autre ; Maj + clic arme plusieurs ports
  // sources, posés sur des ports consécutifs de la cible. Écran de conception : une personne
  // à la fois.
  import { untrack, onDestroy } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import NumberingDialog from './NumberingDialog.svelte';
  import ConnGlyph from '../../components/ConnGlyph.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { route, navigate } from '../../lib/router.svelte';
  import { join } from '../../lib/live.svelte';
  import { relative, nf } from '../../lib/format';
  import { loadRefs, refById, fnColor } from '../../lib/catalog.svelte';
  import { family } from '../../lib/panel';
  import type { ProjectView } from '../../lib/projects';

  interface Port { ref: string; label: string; connector: string; signal: string | null; family: string | null; dir: 'in' | 'out' | 'bidir' | 'through'; gender: string | null; variant: string | null; side: 'l' | 'r' }
  interface SNode { key: string; kind: 'equipment' | 'panel' | 'adapter'; label: string; sub: string; function: string | null; container_id: string | null; location_id: string; ports: Port[] }
  interface Far { node: string | null; label: string; sub: string; port: string; connector: string; external?: boolean; location_id?: string | null; resolved?: boolean | null }
  interface Fields { number: string | null; color: string | null; style: string; waypoints: [number, number][]; notes: string | null; tags: string[]; cable_model_id: string | null; harness_id: string | null; length_m: number | null; attach?: boolean; external?: { location_id: string | null; label: string | null } | null }
  interface CableV { id: string; a: string; b: string; rev: number; fields: Fields; a_in_view: boolean; b_in_view: boolean; a_info: Far | null; b_info: Far | null; warnings: string[]; estimate_m: number | null }
  interface Harness { id: string; name: string; color: string; rev: number }
  interface OvLoc { id: string; name: string; code: string; elements: number; weight_kg: number; power_w: number; outside: number }
  interface Overview { locations: OvLoc[]; links: { a: string; b: string; cables: string[]; external: number }[]; totals: { weight_kg: number; power_w: number } }
  interface Syn {
    view: string; nodes: SNode[]; cables: CableV[]; layout: Record<string, { x: number; y: number }>;
    lock: { account_id: string; account_name: string; taken_at: string; mine: boolean } | null;
    overview: Overview | null; harnesses: Harness[];
    views: { containers: { id: string; name: string; location_id: string; kind: string }[]; locations: { id: string; name: string; code: string }[] };
  }

  let { view: project }: { view: ProjectView } = $props();
  loadRefs();
  const pid = $derived(project.project.id);
  const canDesign = $derived(can('conception', 'ecriture'));
  const seg = $derived(route.path.split('/'));
  const wanted = $derived(seg[4] === 'generale' ? 'overview' : seg[4] && seg[5] ? `${seg[4] === 'lieu' ? 'location' : 'container'}:${seg[5]}` : null);

  let syn = $state<Syn | null>(null);
  const mine = $derived(!!syn?.lock?.mine);
  let sel = $state<string | null>(null);
  /** Câbles choisis (Ctrl ou Maj + clic pour en ajouter). */
  let multi = $state<Set<string>>(new Set());
  let numbering = $state(false);
  let tab = $state<'cable' | 'issues'>('cable');
  interface Issue { code: number; severity: 'error' | 'warning' | 'info'; title: string; detail: string; cables: string[]; nodes: string[]; view: string | null }
  let issues = $state<{ counts: { error: number; warning: number; info: number }; issues: Issue[] } | null>(null);
  let hl = $state<{ cables: Set<string>; nodes: Set<string> }>({ cables: new Set(), nodes: new Set() });
  let pendingHl: Issue | null = null;
  async function validate() { try { issues = await api.get(`/projects/${pid}/synoptic/validate`); } catch (e) { fail(e); } }
  function pickCable(id: string, e: MouseEvent) {
    e.stopPropagation();
    if (e.ctrlKey || e.metaKey || e.shiftKey) { const m = new Set(multi); if (m.has(id)) m.delete(id); else m.add(id); multi = m; }
    else multi = new Set([id]);
    sel = id; tab = 'cable';
  }
  function goIssue(i: Issue) {
    hl = { cables: new Set(i.cables), nodes: new Set(i.nodes) };
    if (i.view && syn && i.view !== syn.view) { pendingHl = i; navigate(viewUrl(i.view)); return; }
    if (i.cables[0]) { sel = i.cables[0]; multi = new Set(i.cables); }
  }
  let armed = $state<Port[]>([]);
  let mouse = $state<[number, number] | null>(null);
  let confirmReplace = $state<{ body: object; n: number } | null>(null);
  let confirmDelete = $state(false);
  let requested = $state<string | null>(null);
  let pos = $state<Record<string, { x: number; y: number }>>({});
  let showEmpty = $state(false);
  const nodes = $derived(syn ? syn.nodes.filter((n) => showEmpty || n.ports.length) : []);
  const hidden = $derived((syn?.nodes.length ?? 0) - nodes.length);

  async function load() {
    try {
      const q = wanted ? `?view=${encodeURIComponent(wanted)}` : '';
      const s = await api.get<Syn>(`/projects/${pid}/synoptic${q}`);
      syn = s;
      pos = s.overview ? { ...ovLayout(s.overview), ...s.layout } : { ...autoLayout(s.nodes.filter((n) => showEmpty || n.ports.length)), ...s.layout };
      if (sel && !s.cables.some((c) => c.id === sel)) sel = null;
      multi = new Set([...multi].filter((id) => s.cables.some((c) => c.id === id)));
      if (pendingHl && pendingHl.view === s.view) { const i = pendingHl; pendingHl = null; if (i.cables[0]) { sel = i.cables[0]; multi = new Set(i.cables); } }
      if (tab === 'issues') validate();
      if (!wanted && s.view) navigate(viewUrl(s.view), true);
    } catch (e) { fail(e); }
  }
  const viewUrl = (v: string) => { if (v === 'overview') return `/projets/${pid}/synoptique/generale`; const [k, id] = v.split(':'); return `/projets/${pid}/synoptique/${k === 'location' ? 'lieu' : 'element'}/${id}`; };

  // ----- Verrou -----
  let beat: ReturnType<typeof setInterval> | null = null;
  let lastAction = Date.now();
  async function take() {
    if (!canDesign) return;
    try { const r = await api.post<{ mine: boolean }>(`/projects/${pid}/synoptic/lock`); if (r.mine && !beat) beat = setInterval(heartbeat, 20_000); await load(); } catch (e) { fail(e); }
  }
  async function heartbeat() {
    if (Date.now() - lastAction > 10 * 60_000) { await release(); toast('Main cédée après 10 minutes sans action.', 'info'); return; }
    try { const r = await api.post<{ mine: boolean }>(`/projects/${pid}/synoptic/lock`); if (!r.mine) { stopBeat(); await load(); } } catch { /* prochain battement */ }
  }
  function stopBeat() { if (beat) { clearInterval(beat); beat = null; } }
  async function release() { stopBeat(); try { await api.del(`/projects/${pid}/synoptic/lock`); } catch { /* déjà libre */ } await load(); }
  async function requestHand() { try { await api.post(`/projects/${pid}/synoptic/lock/request`); toast(`Demande envoyée à ${syn?.lock?.account_name}.`, 'info'); } catch (e) { fail(e); } }

  let started = false;
  $effect(() => { wanted; untrack(async () => { armed = []; sel = null; await load(); if (!started) { started = true; await take(); } }); });
  $effect(() => join(`project:${pid}`, (c) => {
    if (c.kind === 'lock' && c.id === `syn:${pid}`) {
      if (c.action === 'request' && mine) requested = c.actor;
      if (c.action === 'taken') { if (mine) stopBeat(); load(); }
      if (c.action === 'released' && !mine) load();
      return;
    }
    if (!mine && !drag) load();
  }));
  onDestroy(() => { stopBeat(); if (started) fetch(`/api/projects/${pid}/synoptic/lock`, { method: 'DELETE', headers: { 'X-Strata-Request': '1' }, keepalive: true }); });

  // ----- Géométrie -----
  const NW = 270, HH = 52, PS = 28, TAG_GAP = 12;
  const left = (n: SNode) => n.ports.filter((p) => p.side === 'l');
  const right = (n: SNode) => n.ports.filter((p) => p.side === 'r');
  const nodeH = (n: SNode) => HH + Math.max(left(n).length, right(n).length, 1) * PS + 12;
  function autoLayout(nodes: SNode[]) {
    const col = (n: SNode) => (n.kind !== 'equipment' ? 1 : left(n).length === 0 ? 0 : right(n).length === 0 ? 2 : left(n).length > right(n).length ? 2 : 0);
    // Trois familles de colonnes (sources, panels et adaptateurs, destinations), chacune
    // repliée en sous-colonnes au-delà de 1 400 px de haut.
    const MAXH = 1400, COLW = 480;
    const groups: SNode[][] = [[], [], []];
    for (const n of [...nodes].sort((a, b) => a.sub.localeCompare(b.sub, 'fr') || a.label.localeCompare(b.label, 'fr'))) groups[col(n)]!.push(n);
    const out: Record<string, { x: number; y: number }> = {};
    let x = 40;
    for (const g of groups) {
      if (!g.length) continue;
      let y = 40;
      for (const n of g) {
        if (y > 40 && y + nodeH(n) > MAXH) { x += COLW; y = 40; }
        out[n.key] = { x, y };
        y += nodeH(n) + 44;
      }
      x += COLW;
    }
    return out;
  }
  const OVW = 300, OVH = 150;
  function ovLayout(o: Overview) {
    const out: Record<string, { x: number; y: number }> = {};
    o.locations.forEach((l, i) => { out[`loc:${l.id}`] = { x: 60 + (i % 3) * (OVW + 180), y: 60 + Math.floor(i / 3) * (OVH + 140) }; });
    return out;
  }
  const ovCenter = (id: string) => { const p = pos[`loc:${id}`] ?? { x: 0, y: 0 }; return [p.x + OVW / 2, p.y + OVH / 2] as [number, number]; };
  const nodeOf = (ref: string) => syn?.nodes.find((n) => n.ports.some((p) => p.ref === ref));
  function portXY(ref: string): [number, number, 'l' | 'r'] | null {
    const n = nodeOf(ref); if (!n) return null;
    const p = n.ports.find((x) => x.ref === ref)!;
    const list = p.side === 'l' ? left(n) : right(n);
    const i = list.indexOf(p);
    const at = pos[n.key] ?? { x: 0, y: 0 };
    return [p.side === 'l' ? at.x : at.x + NW, at.y + HH + i * PS + PS / 2 + 4, p.side];
  }
  const tagW = (c: CableV) => (c.fields.number ? c.fields.number.length * 6.9 + 19 : 22);
  const boxes = $derived(syn ? nodes.map((n) => { const p = pos[n.key] ?? { x: 0, y: 0 }; return [p.x - 6, p.y - 8, p.x + NW + 6, p.y + nodeH(n) + 8]; }) : []);
  const laneFree = (xa: number, xb: number, y: number) => { const a = Math.min(xa, xb), b = Math.max(xa, xb); return !boxes.some(([x0, y0, x1, y1]) => y > y0! && y < y1! && b > x0! && a < x1!); };
  const colFree = (x: number, ya: number, yb: number) => { const a = Math.min(ya, yb), b = Math.max(ya, yb); return !boxes.some(([x0, y0, x1, y1]) => x > x0! && x < x1! && b > y0! && a < y1!); };
  function findLane(xa: number, xb: number, y: number, dir = 0) {
    if (laneFree(xa, xb, y)) return y;
    for (let k = 1; k < 80; k++) for (const s of dir ? [dir] : [1, -1]) { const yy = y + s * k * 12; if (laneFree(xa, xb, yy)) return yy; }
    return y;
  }
  const jit = (id: string) => ((id.charCodeAt(id.length - 1) + id.charCodeAt(id.length - 2)) % 5 - 2) * 7;
  function cPath(c: CableV) {
    const A = portXY(c.a), B = portXY(c.b);
    if (!A || !B) return null;
    const [x1, y1, s1] = A, [x2, y2, s2] = B, w = tagW(c), run = w + TAG_GAP * 2, j = jit(c.id);
    const tagAt = (s: 'l' | 'r', x: number, y: number) => (s === 'r' ? [x + TAG_GAP, y] : [x - TAG_GAP - w, y]) as [number, number];
    const t1 = tagAt(s1, x1, y1), t2 = tagAt(s2, x2, y2);
    // Sortie à droite vers une entrée plus à droite : couloirs entre les colonnes.
    const [ax, ay, as, bx, by, bs] = s1 === 'l' && s2 === 'r' ? [x2, y2, s2, x1, y1, s1] : [x1, y1, s1, x2, y2, s2];
    if (as === 'r' && bs === 'l' && bx - ax > run * 2 + 20) {
      const m1 = ax + run + 12 + j, m2 = bx - run - 12 + j;
      const mx = Math.round(m2 - m1 < 40 ? (m1 + m2) / 2 : m1);
      if (colFree(mx, ay, by) && laneFree(ax, mx, ay) && laneFree(mx, bx, by)) return { d: `M${ax} ${ay}H${mx}V${by}H${bx}`, t1, t2 };
      const ly = findLane(m1, m2, (ay + by) / 2);
      return { d: `M${ax} ${ay}H${m1}V${ly}H${m2}V${by}H${bx}`, t1, t2 };
    }
    const d1 = s1 === 'r' ? run + 12 + j : -(run + 12 + j), d2 = s2 === 'r' ? run + 12 - j : -(run + 12 - j);
    const ym = findLane(x1 + d1, x2 + d2, Math.max(y1, y2) + 30 + Math.abs(j), 1);
    return { d: `M${x1} ${y1}H${x1 + d1}V${ym}H${x2 + d2}V${y2}H${x2}`, t1, t2 };
  }
  function stub(c: CableV) {
    const inside = c.a_in_view ? c.a : c.b;
    const P = portXY(inside); if (!P) return null;
    const [x, y, s] = P, far = c.a_in_view ? c.b_info : c.a_info;
    const dx = s === 'r' ? 1 : -1, w = tagW(c);
    const end = x + dx * (w + TAG_GAP * 2 + 60);
    return { d: `M${x} ${y}H${end}`, t: (s === 'r' ? [x + TAG_GAP, y] : [x - TAG_GAP - w, y]) as [number, number], ex: end, ey: y, dx, far };
  }
  const portColor = (p: Port | undefined) => (p?.family ? fnColor(p.family) : refById('connectors', p?.connector)?.data?.color ?? '#8A919B');
  const portByRef = (ref: string) => nodeOf(ref)?.ports.find((p) => p.ref === ref);
  const cableColor = (c: CableV) => c.fields.color ?? portColor(portByRef(c.a) ?? portByRef(c.b));
  const used = $derived(new Set(syn?.cables.flatMap((c) => [c.a, c.b]) ?? []));
  const short = (c: string) => { const n = refById('connectors', c)?.name ?? c; const s = n.replace(/\s*\/.*$/, ''); return s.length > 9 ? s.split(/\s+/)[0]!.slice(0, 9) : s; };
  const warnCount = $derived(syn?.cables.filter((c) => c.warnings.length).length ?? 0);
  const selected = $derived(syn?.cables.find((c) => c.id === sel) ?? null);

  // ----- Vue : déplacement et zoom -----
  let svgEl = $state<SVGSVGElement>(null!);
  let k = $state(1), tx = $state(20), ty = $state(20);
  function toWorld(e: { clientX: number; clientY: number }): [number, number] {
    const r = svgEl.getBoundingClientRect();
    return [(e.clientX - r.left - tx) / k, (e.clientY - r.top - ty) / k];
  }
  function wheel(e: WheelEvent) {
    e.preventDefault();
    const r = svgEl.getBoundingClientRect(), mx = e.clientX - r.left, my = e.clientY - r.top;
    const nk = Math.min(2.5, Math.max(0.2, k * (e.deltaY < 0 ? 1.12 : 1 / 1.12)));
    tx = mx - ((mx - tx) * nk) / k; ty = my - ((my - ty) * nk) / k; k = nk;
  }
  function fit() {
    if (syn?.overview && svgEl) {
      const ls = syn.overview.locations; if (!ls.length) return;
      const xs = ls.flatMap((l) => [pos[`loc:${l.id}`]!.x - 60, pos[`loc:${l.id}`]!.x + OVW + 60]);
      const ys = ls.flatMap((l) => [pos[`loc:${l.id}`]!.y - 60, pos[`loc:${l.id}`]!.y + OVH + 60]);
      const r = svgEl.getBoundingClientRect(), w = Math.max(...xs) - Math.min(...xs), h = Math.max(...ys) - Math.min(...ys);
      k = Math.min(1.2, Math.max(0.3, Math.min(r.width / w, r.height / h)));
      tx = -Math.min(...xs) * k + (r.width - w * k) / 2; ty = -Math.min(...ys) * k + 10;
      return;
    }
    if (!nodes.length || !svgEl) return;
    const xs = nodes.flatMap((n) => [pos[n.key]!.x - 120, pos[n.key]!.x + NW + 120]);
    const ys = nodes.flatMap((n) => [pos[n.key]!.y - 40, pos[n.key]!.y + nodeH(n) + 60]);
    const r = svgEl.getBoundingClientRect();
    const w = Math.max(...xs) - Math.min(...xs), h = Math.max(...ys) - Math.min(...ys);
    k = Math.min(1.2, Math.max(0.2, Math.min(r.width / w, r.height / h)));
    tx = -Math.min(...xs) * k + (r.width - w * k) / 2; ty = -Math.min(...ys) * k + 10;
  }
  let fitted = '';
  function start() {
    fit();
    if (k < 0.6) { k = 0.75; tx = 10; ty = 10; }
  }
  $effect(() => { if (syn && svgEl && fitted !== syn.view) { fitted = syn.view; untrack(start); } });

  let drag: { kind: 'pan' | 'node'; key?: string; ox: number; oy: number; moved: boolean } | null = null;
  function down(e: PointerEvent) {
    const t = e.target as Element;
    if (t.closest('.port') || t.closest('.cable') || t.closest('.lbl')) return;
    const nodeEl = t.closest<SVGGElement>('[data-node]');
    if (nodeEl && mine && t.closest('.drag')) {
      const key = nodeEl.dataset.node!, [wx, wy] = toWorld(e);
      drag = { kind: 'node', key, ox: wx - pos[key]!.x, oy: wy - pos[key]!.y, moved: false };
    } else if (!nodeEl) {
      drag = { kind: 'pan', ox: e.clientX - tx, oy: e.clientY - ty, moved: false };
      sel = null; multi = new Set();
    }
    if (drag) { (e.currentTarget as Element).setPointerCapture(e.pointerId); }
  }
  function move(e: PointerEvent) {
    if (armed.length) mouse = toWorld(e);
    if (!drag) return;
    drag.moved = true;
    if (drag.kind === 'pan') { tx = e.clientX - drag.ox; ty = e.clientY - drag.oy; return; }
    const [wx, wy] = toWorld(e);
    pos[drag.key!] = { x: Math.round((wx - drag.ox) / 10) * 10, y: Math.round((wy - drag.oy) / 10) * 10 };
  }
  async function up() {
    const d = drag; drag = null;
    if (d?.kind === 'node' && d.moved && syn) {
      lastAction = Date.now();
      try { await api.put(`/projects/${pid}/synoptic/layout`, { view: syn.view, positions: [{ node: d.key, ...pos[d.key!] }] }); } catch (e) { fail(e); await load(); }
    }
  }

  // ----- Câblage -----
  const isAdIn = (p: Port) => p.ref.startsWith('a:') && p.ref.endsWith(':in');
  let extForm = $state<{ port: Port; location_id: string; label: string } | null>(null);
  async function attachTo(adapter: Port, port: Port, replace = false) {
    try { await api.post(`/projects/${pid}/attach`, { adapter: adapter.ref, port: port.ref, replace }); toast('Adaptateur attaché.'); await load(); }
    catch (e) { if (e instanceof ApiError && e.code === 'occupied' && confirm('Ce port porte déjà un câble. Le remplacer par l’attache ?')) await attachTo(adapter, port, true); else if (!(e instanceof ApiError && e.code === 'occupied')) { fail(e); await load(); } }
  }
  async function markExternal() {
    if (!extForm) return;
    const body = { port: extForm.port.ref, location_id: extForm.location_id || null, label: extForm.label.trim() || null };
    try { await api.post(`/projects/${pid}/external`, body); toast('Port externe marqué.'); extForm = null; armed = []; await load(); } catch (e) { fail(e); }
  }
  function clickPort(p: Port, n: SNode, e: MouseEvent) {
    e.stopPropagation();
    if (!mine) { const c = syn?.cables.find((x) => x.a === p.ref || x.b === p.ref); if (c) sel = c.id; return; }
    lastAction = Date.now();
    if (!armed.length) { armed = [p]; mouse = null; extForm = null; return; }
    if (armed.some((a) => a.ref === p.ref)) { armed = armed.filter((a) => a.ref !== p.ref); return; }
    // Entrée d'adaptateur sur un port du même connecteur (ou l'inverse) : attache, sans câble.
    if (armed.length === 1 && n.kind !== 'adapter' && isAdIn(armed[0]!) && armed[0]!.connector === p.connector) { const a = armed[0]!; armed = []; mouse = null; attachTo(a, p); return; }
    if (armed.length === 1 && isAdIn(p) && nodeOf(armed[0]!.ref)?.kind !== 'adapter' && armed[0]!.connector === p.connector) { const t = armed[0]!; armed = []; mouse = null; attachTo(p, t); return; }
    const first = nodeOf(armed[0]!.ref);
    if (e.shiftKey && first?.key === n.key) { armed = [...armed, p]; return; }
    // Cible : ports consécutifs du même côté, à partir du port cliqué.
    const list = p.side === 'l' ? left(n) : right(n);
    const start = list.indexOf(p);
    const srcs = [...armed].sort((a, b) => (portXY(a.ref)?.[1] ?? 0) - (portXY(b.ref)?.[1] ?? 0));
    const targets = list.slice(start, start + srcs.length);
    if (targets.length < srcs.length) { toast(`Pas assez de ports à partir de « ${p.label} » : ${srcs.length} demandés.`, 'warn'); return; }
    const body = { cables: srcs.map((s, i) => ({ a: s.ref, b: targets[i]!.ref })) };
    armed = []; mouse = null;
    place(body, false);
  }
  async function place(body: { cables: { a: string; b: string }[] }, replace: boolean) {
    try {
      const r = await api.post<{ created: { id: string }[]; warnings: string[][] }>(`/projects/${pid}/cables`, { ...body, replace });
      const w = r.warnings.flat();
      if (w.length) toast(`${r.created.length} câble(s) posé(s) · ${[...new Set(w)].join(' ')}`, 'warn');
      await load();
      if (r.created.length === 1) sel = r.created[0]!.id;
    } catch (e) {
      if (e instanceof ApiError && e.status === 409 && e.code === 'occupied') confirmReplace = { body, n: body.cables.length };
      else { fail(e); await load(); }
    }
  }
  let newHarness = $state<{ name: string; color: string } | null>(null);
  const harnessOf = (c: CableV) => syn?.harnesses.find((h) => h.id === c.fields.harness_id) ?? null;
  async function createHarness() {
    if (!newHarness) return;
    try { const h = await api.post<Harness>(`/projects/${pid}/harnesses`, newHarness); const ids = multi.size ? [...multi] : sel ? [sel] : []; if (ids.length) await api.post(`/projects/${pid}/harnesses/assign`, { ids, harness_id: h.id }); newHarness = null; await load(); } catch (e) { fail(e); }
  }
  async function assign(id: string | null) {
    const ids = multi.size ? [...multi] : sel ? [sel] : [];
    if (!ids.length) return;
    try { await api.post(`/projects/${pid}/harnesses/assign`, { ids, harness_id: id }); await load(); } catch (e) { fail(e); }
  }
  async function dropHarness(h: Harness) { if (!confirm(`Supprimer le faisceau « ${h.name} » ? Ses câbles restent.`)) return; try { await api.del(`/harnesses/${h.id}`); await load(); } catch (e) { fail(e); } }
  function showHarness(h: Harness) { const ids = syn?.cables.filter((c) => c.fields.harness_id === h.id).map((c) => c.id) ?? []; hl = { cables: new Set(ids), nodes: new Set() }; multi = new Set(ids); sel = ids[0] ?? null; }
  async function patchSel(f: Partial<Fields>) {
    const c = selected; if (!c) return;
    lastAction = Date.now();
    try { await api.patch(`/cables/${c.id}`, { rev: c.rev, ...c.fields, ...f }); await load(); } catch (e) { fail(e); await load(); }
  }
  async function deleteSel() {
    const ids = multi.size > 1 ? [...multi] : selected ? [selected.id] : [];
    if (!ids.length) return;
    try { await api.post(`/projects/${pid}/cables/delete`, { ids }); sel = null; multi = new Set(); confirmDelete = false; await load(); } catch (e) { fail(e); }
  }
  function key(e: KeyboardEvent) {
    if ((e.target as Element).closest('input,textarea,select')) return;
    if (e.key === 'Escape') { armed = []; mouse = null; sel = null; multi = new Set(); hl = { cables: new Set(), nodes: new Set() }; }
    if ((e.key === 'Delete' || e.key === 'Backspace') && selected && mine) { e.preventDefault(); confirmDelete = true; }
  }
  const armedPreview = $derived.by(() => {
    if (!armed.length || !mouse) return '';
    return armed.map((a) => { const P = portXY(a.ref); if (!P) return ''; const [x, y, s] = P; const mx = x + (s === 'r' ? 30 : -30); return `M${x} ${y}H${mx}L${mouse![0]} ${mouse![1]}`; }).join(' ');
  });
  const endName = (ref: string, info: Far | null) => { const n = nodeOf(ref); const p = n?.ports.find((x) => x.ref === ref); return n && p ? `${n.label} › ${p.label}` : info ? `${info.label} › ${info.port}` : 'Extrémité manquante'; };
</script>

<svelte:window onkeydown={key} />

{#if !syn}<span class="spin"></span>
{:else}
  <div class="syn card" style="padding:0;overflow:hidden">
    <div class="lockbar" class:mine class:other={!mine}>
      {#if mine}
        <span class="lk"><span class="key-i"><Icon name="lock" /></span><span><b>Vous avez la main</b> sur le synoptique du projet.</span></span>
        <span class="sp">{#if requested}<span class="chip warn">{requested} demande la main</span>{/if}<button class="btn sm" onclick={() => { requested = null; release(); }}>Céder la main</button></span>
      {:else if syn.lock}
        <span class="lk"><span class="key-i"><Icon name="lock" /></span><span><b>{syn.lock.account_name}</b> a la main depuis {relative(syn.lock.taken_at)}. Vous voyez la dernière version enregistrée.</span></span>
        <span class="sp">{#if canDesign}<button class="btn sm" onclick={requestHand}>Demander la main</button>{/if}</span>
      {:else}
        <span class="lk"><span class="key-i"><Icon name="unlock" /></span><span>Personne ne modifie le synoptique.</span></span>
        <span class="sp">{#if canDesign}<button class="btn sm primary" onclick={take}>Prendre la main</button>{:else}<span class="muted small">Lecture seule : votre rôle ne permet pas de concevoir.</span>{/if}</span>
      {/if}
    </div>
    <div class="dtool">
      <select class="input" aria-label="Vue du synoptique" value={syn.view} onchange={(e) => navigate(viewUrl((e.target as HTMLSelectElement).value))}>
        <option value="overview">Vue générale (liaisons entre lieux)</option>
        <optgroup label="Éléments">{#each syn.views.containers as c}<option value="container:{c.id}">{c.name}</option>{/each}</optgroup>
        <optgroup label="Lieux">{#each syn.views.locations as l}<option value="location:{l.id}">{l.name} (tout le lieu)</option>{/each}</optgroup>
      </select>
{#if !syn.overview}      <label class="toggle-row small"><input type="checkbox" bind:checked={showEmpty} onchange={() => { if (syn) pos = { ...autoLayout(syn.nodes.filter((n) => showEmpty || n.ports.length)), ...syn.layout }; }}>Sans port{hidden ? ` (${hidden})` : ''}</label>
      <span class="muted small">{nodes.length} nœud{nodes.length > 1 ? 's' : ''} · {syn.cables.length} câble{syn.cables.length > 1 ? 's' : ''}{warnCount ? ` · ${warnCount} à vérifier` : ''}</span>{:else}<span class="muted small">{syn.overview.locations.length} lieu{syn.overview.locations.length > 1 ? 'x' : ''} · {nf(syn.overview.totals.weight_kg, 1)} kg · {nf(syn.overview.totals.power_w)} W · double-clic sur un lieu pour l’ouvrir</span>{/if}
      <span style="flex:1"></span>
      {#if armed.length}<span class="chip accent">{armed.length} port{armed.length > 1 ? 's' : ''} armé{armed.length > 1 ? 's' : ''} · cliquez la cible · Échap pour annuler</span>{/if}
      <button class="icon-btn sm" title="Zoom arrière" aria-label="Zoom arrière" onclick={() => (k = Math.max(0.2, k / 1.2))}><Icon name="zout" size="sm" /></button>
      <span class="mono small">{Math.round(k * 100)} %</span>
      <button class="icon-btn sm" title="Zoom avant" aria-label="Zoom avant" onclick={() => (k = Math.min(2.5, k * 1.2))}><Icon name="zin" size="sm" /></button>
      <button class="btn sm" onclick={fit}><Icon name="fit" size="sm" />Tout afficher</button>
      <button class="btn sm" class:on={tab === 'issues'} onclick={() => { tab = tab === 'issues' ? 'cable' : 'issues'; if (tab === 'issues') validate(); }}><Icon name="check" size="sm" />Anomalies{#if issues} <span class="mono">{issues.counts.error + issues.counts.warning}</span>{/if}</button>
      {#if mine}<button class="btn sm primary" onclick={() => (numbering = true)}><Icon name="hash" size="sm" />Numéroter</button>{/if}
    </div>
    <div class="syn-body">
      <svg bind:this={svgEl} class="syn-canvas" class:arming={armed.length > 0} role="application" aria-label="Synoptique"
        onwheel={wheel} onpointerdown={down} onpointermove={move} onpointerup={up}>
        <defs><pattern id="syn-grid" width="20" height="20" patternUnits="userSpaceOnUse" patternTransform="translate({tx} {ty}) scale({k})"><circle cx="1" cy="1" r="1" fill="var(--grid-dot, rgba(120,130,140,.25))" /></pattern></defs>
        <rect width="100%" height="100%" fill="url(#syn-grid)" />
        <g transform="translate({tx} {ty}) scale({k})">
          {#if syn.overview}
            {@const ov = syn.overview}
            <g class="ov-links">
              {#each ov.links as l}
                {@const [x1, y1] = ovCenter(l.a)}
                {@const [x2, y2] = ovCenter(l.b)}
                <line class="ov-line" class:ext={l.external > 0} x1={x1} y1={y1} x2={x2} y2={y2} style="stroke-width:{Math.min(14, 2 + l.cables.length)}" />
                <g class="ov-count" transform="translate({(x1 + x2) / 2} {(y1 + y2) / 2})"><rect x="-38" y="-14" width="76" height="28" rx="14" /><text y="5" text-anchor="middle">{l.cables.length} câble{l.cables.length > 1 ? 's' : ''}</text></g>
              {/each}
            </g>
            {#each ov.locations as l (l.id)}
              {@const at = pos[`loc:${l.id}`] ?? { x: 0, y: 0 }}
              <g class="ov-loc" data-node="loc:{l.id}" transform="translate({at.x} {at.y})" role="button" tabindex="-1" ondblclick={() => navigate(viewUrl(`location:${l.id}`))} onkeydown={() => {}}>
                <rect class="box" width={OVW} height={OVH} rx="14" />
                <rect class="drag" width={OVW} height="56" fill="transparent" />
                <text class="ov-code" x="18" y="28">{l.code}</text>
                <text class="ov-name" x="18" y="50">{l.name.slice(0, 28)}</text>
                <text class="ov-k" x="18" y="86">{l.elements} élément{l.elements > 1 ? 's' : ''}</text>
                <text class="ov-k" x="18" y="108">{nf(l.weight_kg, 1)} kg · {nf(l.power_w)} W</text>
                {#if l.outside}<text class="ov-out" x="18" y="132">↗ {l.outside} port{l.outside > 1 ? 's' : ''} hors projet</text>{/if}
                <text class="ov-open" x={OVW - 16} y="132" text-anchor="end">Double-clic : ouvrir</text>
              </g>
            {/each}
          {/if}
          <g class="cables">
            {#each syn.cables as c (c.id)}
              {#if c.a_in_view && c.b_in_view}
                {@const g = cPath(c)}
                {#if g}
                  <g class="cable" class:attach={c.fields.attach} class:harnessed={!!harnessOf(c)} class:sel={multi.has(c.id) || sel === c.id} class:hl={hl.cables.has(c.id)} class:warn={c.warnings.length > 0} style="--c:{cableColor(c)};--h:{harnessOf(c)?.color ?? 'transparent'}" role="button" tabindex="-1" onclick={(e) => pickCable(c.id, e)} onkeydown={() => {}}>
                    <title>{c.fields.number ?? 'Sans numéro'} · {endName(c.a, c.a_info)} → {endName(c.b, c.b_info)}</title>
                    <path class="case" d={g.d} /><path class="vis" d={g.d} /><path class="hit" d={g.d} />
                  </g>
                {/if}
              {:else}
                {@const s = stub(c)}
                {#if s}
                  <g class="cable stub" class:sel={multi.has(c.id) || sel === c.id} class:hl={hl.cables.has(c.id)} class:warn={c.warnings.length > 0} style="--c:{cableColor(c)}" role="button" tabindex="-1" onclick={(e) => pickCable(c.id, e)} onkeydown={() => {}}>
                    <path class="case" d={s.d} /><path class="vis" d={s.d} /><path class="hit" d={s.d} />
                    <g class="far" transform="translate({s.dx > 0 ? s.ex + 4 : s.ex - 4} {s.ey - 11})"><rect x={s.dx > 0 ? 0 : -190} width="190" height="22" rx="6" /><text x={s.dx > 0 ? 8 : -182} y="15">{s.far?.external ? '↗' : '→'} {(s.far ? (s.far.external ? s.far.label : `${s.far.label} › ${s.far.port}`) : 'Extrémité manquante').slice(0, 30)}</text></g>
                  </g>
                {/if}
              {/if}
            {/each}
          </g>
          <g class="nodes">
            {#each nodes as n (n.key)}
              {@const at = pos[n.key] ?? { x: 0, y: 0 }}
              {@const h = nodeH(n)}
              <g class="node {n.kind}" class:hl={hl.nodes.has(n.key)} data-node={n.key} transform="translate({at.x} {at.y})" style="--c:{n.function ? fnColor(n.function) : n.kind === 'panel' ? '#5B636D' : '#8A919B'}">
                <rect class="box" width={NW} height={h} rx="10" />
                <path class="hdr" d="M0 10a10 10 0 0 1 10-10h{NW - 20}a10 10 0 0 1 10 10v{HH - 10}H0z" />
                <rect class="band" x="0" y="0" width={NW} height="4" rx="2" />
                <line class="sep" x1="0" x2={NW} y1={HH} y2={HH} />
                <circle class="nled" cx="16" cy={HH / 2 + 1} r="4.5" />
                <text class="nt" x="30" y="25">{n.label.slice(0, 30)}</text>
                <text class="ns" x="30" y="42">{n.sub.slice(0, 40)}</text>
                <rect class="drag" width={NW} height={HH} fill="transparent" />
                {#each [left(n), right(n)] as list, si}
                  {#each list as p, i (p.ref)}
                    {@const out = si === 1}
                    {@const x = out ? NW : 0}
                    {@const y = HH + i * PS + PS / 2 + 4}
                    {@const s = short(p.connector)}
                    {@const cw = s.length * 6.3 + 12}
                    {@const cx0 = out ? NW - 16 - cw : 16}
                    {@const room = (si === 0 ? right(n) : left(n)).length > i ? 9 : 20}
                    <g class="port" class:used={used.has(p.ref)} class:armed={armed.some((a) => a.ref === p.ref)} style="--c:{portColor(p)}" role="button" tabindex="-1"
                      onclick={(e) => clickPort(p, n, e)} onkeydown={() => {}}>
                      <title>{p.label} · {refById('connectors', p.connector)?.name ?? p.connector}{p.variant ? ` ${p.variant}` : ''}{p.gender ? ` · ${p.gender === 'M' ? 'mâle' : p.gender === 'F' ? 'femelle' : 'neutre'}` : ''}{p.signal ? ` · ${refById('signals', p.signal)?.name ?? p.signal}` : ''}</title>
                      <circle class="ph" cx={x} cy={y} r="13" />
                      <g class="glyph"><ConnGlyph f={family(p.connector)} {x} cy={y} /></g>
                      <g class="cchip"><rect x={cx0} y={y - 8.5} width={cw} height="17" rx="4" /><text x={cx0 + cw / 2} y={y + 3.6} text-anchor="middle">{s}</text></g>
                      <text class="pn" x={out ? cx0 - 7 : cx0 + cw + 7} y={y + 4.5} text-anchor={out ? 'end' : 'start'}>{p.label.length > room ? p.label.slice(0, room - 1) + '…' : p.label}{p.dir === 'bidir' ? ' ↔' : ''}</text>
                    </g>
                  {/each}
                {/each}
                {#if !n.ports.length}<text class="ns" x={NW / 2} y={HH + 26} text-anchor="middle">Aucun port dans la fiche</text>{/if}
              </g>
            {/each}
          </g>
          <g class="tags">
            {#each syn.cables.filter((x) => !x.fields.attach) as c (c.id)}
              {@const both = c.a_in_view && c.b_in_view}
              {@const g = both ? cPath(c) : null}
              {@const s = both ? null : stub(c)}
              {#each (g ? [g.t1, g.t2] : s ? [s.t] : []) as t}
                <g class="lbl" class:none={!c.fields.number} class:sel={multi.has(c.id) || sel === c.id} class:hl={hl.cables.has(c.id)} style="--c:{cableColor(c)}" transform="translate({t[0]} {t[1] - 9})" role="button" tabindex="-1" onclick={(e) => pickCable(c.id, e)} onkeydown={() => {}}>
                  <rect class="tag" width={tagW(c)} height="18" rx="4" />
                  {#if c.fields.number}<rect class="bandc" x="3.5" y="3.5" width="3" height="11" rx="1" /><text x="10.5" y="13">{c.fields.number}</text>{:else}<text x="7.5" y="13.5">?</text>{/if}
                </g>
              {/each}
            {/each}
          </g>
          {#if armedPreview}<path class="preview" d={armedPreview} />{/if}
        </g>
      </svg>

      <aside class="syn-side">
        {#if tab === 'issues'}
          <section>
            <span class="lbl-t">Validation</span>
            {#if !issues}<span class="spin"></span>
            {:else}
              <div class="counts"><span class="cnt err"><b>{issues.counts.error}</b> erreur{issues.counts.error > 1 ? 's' : ''}</span><span class="cnt warn"><b>{issues.counts.warning}</b> avertissement{issues.counts.warning > 1 ? 's' : ''}</span><span class="cnt info"><b>{issues.counts.info}</b> info{issues.counts.info > 1 ? 's' : ''}</span></div>
              {#each issues.issues as i, k (k)}
                <button class="issue {i.severity}" onclick={() => goIssue(i)}><Icon name={i.severity === 'info' ? 'info' : 'alert'} size="sm" /><span><b>{i.title}</b><span class="muted small">{i.detail}</span></span></button>
              {:else}<p class="muted">Aucune anomalie : le câblage est cohérent.</p>{/each}
              <button class="btn sm ghost" onclick={validate}><Icon name="refresh" size="sm" />Relancer</button>
            {/if}
          </section>
        {:else if multi.size > 1}
          <section>
            <span class="lbl-t">{multi.size} câbles choisis</span>
            <p class="muted small" style="margin:0">Ctrl ou Maj + clic pour en ajouter ou en retirer.</p>
            {#if mine}
              <div class="field"><label for="mh">Faisceau</label><select class="input" id="mh" onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v === '+') newHarness = { name: '', color: '#2459E0' }; else assign(v || null); (e.target as HTMLSelectElement).value = '~'; }}><option value="~">Ranger dans…</option><option value="">Aucun faisceau</option>{#each syn.harnesses as h}<option value={h.id}>{h.name}</option>{/each}<option value="+">+ Nouveau faisceau…</option></select></div>
              <button class="btn sm primary" onclick={() => (numbering = true)}><Icon name="hash" size="sm" />Numéroter la sélection</button>
              <button class="btn sm ghost danger" onclick={() => (confirmDelete = true)}><Icon name="trash" size="sm" />Supprimer les {multi.size} câbles</button>{/if}
          </section>
        {:else if selected?.fields.attach}
          {@const c = selected}
          <section>
            <span class="lbl-t">Attache d’adaptateur</span>
            <div class="ends"><div><span class="flabel">Adaptateur</span><b>{endName(c.b, c.b_info)}</b></div><div><span class="flabel">Enfiché sur</span><b>{endName(c.a, c.a_info)}</b></div></div>
            {#each c.warnings as w}<div class="alert warn"><Icon name="alert" /><span>{w}</span></div>{/each}
            <p class="muted small" style="margin:0">Sans câble : l’entrée de l’adaptateur est enfichée directement sur le port.</p>
            {#if mine}<button class="btn sm ghost danger" onclick={() => (confirmDelete = true)}><Icon name="trash" size="sm" />Détacher</button>{/if}
          </section>
        {:else if selected}
          {@const c = selected}
          <section>
            <span class="lbl-t">{c.fields.external ? 'Port externe' : 'Câble'}</span>
            <div class="field"><label for="cn">Numéro</label><input class="input mono" id="cn" value={c.fields.number ?? ''} disabled={!mine} placeholder="Sans numéro" onchange={(e) => patchSel({ number: (e.target as HTMLInputElement).value || null })}></div>
            <div class="ends">
              <div><span class="flabel">De</span><b>{endName(c.a, c.a_info)}</b><span class="muted small">{refById('connectors', portByRef(c.a)?.connector ?? c.a_info?.connector)?.name ?? ''}</span></div>
              <div><span class="flabel">Vers</span><b>{endName(c.b, c.b_info)}</b><span class="muted small">{refById('connectors', portByRef(c.b)?.connector ?? c.b_info?.connector)?.name ?? ''}</span></div>
            </div>
            {#each c.warnings as w}<div class="alert warn"><Icon name="alert" /><span>{w}</span></div>{/each}
            <div class="sh2">
              <div class="field"><label for="cc">Couleur</label><input class="input" id="cc" type="color" style="padding:4px;height:40px" value={c.fields.color ?? cableColor(c)} disabled={!mine} onchange={(e) => patchSel({ color: (e.target as HTMLInputElement).value })}></div>
              <div class="field"><label for="cl">Longueur réelle (m)</label><input class="input mono" id="cl" type="number" min="0" step="0.5" value={c.fields.length_m ?? ''} disabled={!mine} placeholder={c.estimate_m ? `≈ ${nf(c.estimate_m, 1)}` : 'À saisir'} onchange={(e) => { const v = (e.target as HTMLInputElement).value; patchSel({ length_m: v === '' ? null : Number(v) }); }}></div>
            </div>
            <div class="field"><label for="ch">Faisceau</label><select class="input" id="ch" disabled={!mine} value={c.fields.harness_id ?? ''} onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v === '+') { newHarness = { name: '', color: '#2459E0' }; } else assign(v || null); }}><option value="">Aucun</option>{#each syn.harnesses as h}<option value={h.id}>{h.name}</option>{/each}{#if mine}<option value="+">+ Nouveau faisceau…</option>{/if}</select></div>
            <dl class="kv"><dt>Longueur estimée</dt><dd>{c.estimate_m ? `${nf(c.estimate_m, 1)} m` : '—'}</dd></dl>
            <div class="field"><label for="cno">Notes</label><textarea class="input" id="cno" rows="3" maxlength="2000" disabled={!mine} value={c.fields.notes ?? ''} onchange={(e) => patchSel({ notes: (e.target as HTMLTextAreaElement).value || null })}></textarea></div>
            {#if mine}<button class="btn sm ghost danger" onclick={() => (confirmDelete = true)}><Icon name="trash" size="sm" />Supprimer le câble</button>{/if}
          </section>
        {:else if armed.length === 1 && mine}
          {@const a = armed[0]!}
          <section>
            <span class="lbl-t">Port armé</span>
            <b>{nodeOf(a.ref)?.label} › {a.label}</b>
            <p class="muted small" style="margin:0">{isAdIn(a) ? 'Cliquez un port d’équipement ou de panel du même connecteur : l’adaptateur s’y attache sans câble. Sinon, cliquez un autre port pour poser un câble.' : 'Cliquez le port d’arrivée pour poser un câble, ou marquez ce port comme sortant de l’élément.'}</p>
            {#if extForm}
              <div class="field"><label for="xl">Vers un autre lieu</label><select class="input" id="xl" bind:value={extForm.location_id}><option value="">Hors projet</option>{#each syn.views.locations.filter((l) => l.id !== nodeOf(a.ref)?.location_id) as l}<option value={l.id}>{l.name}</option>{/each}</select></div>
              {#if !extForm.location_id}<div class="field"><label for="xt">Destination</label><input class="input" id="xt" bind:value={extForm.label} maxlength="120" placeholder="Car OB2, diffuseur, fibre opérateur…"></div>{/if}
              <div class="row-actions" style="justify-content:flex-start"><button class="btn sm primary" disabled={!extForm.location_id && !extForm.label.trim()} onclick={markExternal}>Marquer</button><button class="btn sm ghost" onclick={() => (extForm = null)}>Annuler</button></div>
            {:else}<button class="btn sm" onclick={() => (extForm = { port: a, location_id: '', label: '' })}><Icon name="arrowr" size="sm" />Port externe…</button>{/if}
          </section>
        {:else}
          <section>
            <span class="lbl-t">Câblage</span>
            <p class="muted small" style="margin:0">{mine ? 'Cliquez un port, puis le port d’arrivée. Maj + clic pour armer plusieurs ports d’un même équipement : ils se posent sur des ports consécutifs de la cible. Glissez un en-tête pour déplacer un nœud.' : 'Cliquez un câble ou un port câblé pour voir ses détails.'}</p>
            {#if warnCount}
              <span class="lbl-t" style="margin-top:8px">À vérifier</span>
              {#each syn.cables.filter((x) => x.warnings.length) as w (w.id)}
                <button class="warn-row" onclick={() => (sel = w.id)}><Icon name="alert" size="sm" /><span><b>{w.fields.number ?? 'Sans numéro'}</b><span class="muted small">{w.warnings.join(' ')}</span></span></button>
              {/each}
            {/if}
            <span class="lbl-t" style="margin-top:8px">Faisceaux</span>
            {#each syn.harnesses as h (h.id)}
              {@const n = syn.cables.filter((c) => c.fields.harness_id === h.id).length}
              <div class="hrow"><button class="hname" onclick={() => showHarness(h)}><i class="dot" style="--c:{h.color}"></i><b>{h.name}</b><span class="muted small">{n} câble{n > 1 ? 's' : ''} dans la vue</span></button>{#if mine}<button class="icon-btn sm" aria-label="Supprimer le faisceau {h.name}" onclick={() => dropHarness(h)}><Icon name="trash" size="sm" /></button>{/if}</div>
            {:else}<span class="muted small">Aucun faisceau. Choisissez des câbles puis « Ranger dans… ».</span>{/each}
            {#if mine}<button class="btn sm ghost" onclick={() => (newHarness = { name: '', color: '#2459E0' })}><Icon name="plus" size="sm" />Faisceau</button>{/if}
          </section>
        {/if}
      </aside>
    </div>
  </div>
{/if}

{#if newHarness}
  <Confirm title="Nouveau faisceau" confirmLabel="Créer" onconfirm={createHarness} onclose={() => (newHarness = null)}>
    <div class="sh2"><div class="field"><label for="hn">Nom</label><input class="input" id="hn" bind:value={newHarness.name} maxlength="120" placeholder="Multi vidéo plateau"></div><div class="field"><label for="hc">Couleur</label><input class="input" id="hc" type="color" style="padding:4px;height:40px" bind:value={newHarness.color}></div></div>
    <p class="muted small">{multi.size || sel ? 'Les câbles choisis y seront rangés.' : 'Rangez-y des câbles ensuite.'}</p>
  </Confirm>
{/if}
{#if numbering && syn}<NumberingDialog project={project} view={syn.view} selection={multi.size ? [...multi] : sel ? [sel] : []} onclose={() => (numbering = false)} ondone={() => { numbering = false; load(); }} />{/if}
{#if confirmReplace}
  <Confirm title="Remplacer des câbles ?" confirmLabel="Remplacer" onconfirm={async () => { const b = confirmReplace!.body; confirmReplace = null; await place(b as { cables: { a: string; b: string }[] }, true); }} onclose={() => (confirmReplace = null)}>
    <p>Un ou plusieurs ports visés portent déjà un câble. Un port ne porte qu’un câble : le câble existant sera supprimé et remplacé.</p>
  </Confirm>
{/if}
{#if confirmDelete && multi.size > 1}
  <Confirm title="Supprimer {multi.size} câbles ?" confirmLabel="Supprimer" danger onconfirm={deleteSel} onclose={() => (confirmDelete = false)}>
    <p>Les câbles choisis sont supprimés ; leurs numéros ne seront pas réattribués.</p>
  </Confirm>
{:else if confirmDelete && selected}
  <Confirm title="Supprimer le câble {selected.fields.number ?? ''} ?" confirmLabel="Supprimer" danger onconfirm={deleteSel} onclose={() => (confirmDelete = false)}>
    <p>{endName(selected.a, selected.a_info)} → {endName(selected.b, selected.b_info)}</p>
  </Confirm>
{/if}

<style>
  .dtool { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; padding: 10px 16px; border-top: 1px solid var(--line); }
  .dtool select { width: auto; min-width: 220px; height: 38px; }
  .syn-body { display: grid; grid-template-columns: minmax(0, 1fr) 320px; border-top: 1px solid var(--line); height: calc(100vh - 220px); min-height: 520px; }
  .syn-canvas { width: 100%; height: 100%; display: block; background: var(--surface-2); touch-action: none; cursor: grab; user-select: none; }
  .syn-canvas:active { cursor: grabbing; }
  .syn-canvas.arming { cursor: crosshair; }
  .syn-side { border-left: 1px solid var(--line); padding: 16px; overflow: auto; background: var(--surface); }
  .syn-side section { display: grid; gap: 10px; }
  .ends { display: grid; gap: 8px; }
  .ends > div { display: flex; flex-direction: column; padding: 10px; border-radius: 12px; background: var(--surface-2); }
  .sh2 { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  .warn-row { display: flex; gap: 8px; align-items: flex-start; padding: 8px; border-radius: 10px; border: 0; background: color-mix(in srgb, var(--warn, #B78400) 10%, transparent); text-align: left; cursor: pointer; font: inherit; color: inherit; }
  .warn-row span { display: flex; flex-direction: column; }
  /* Nœuds */
  .node .box { fill: var(--node-bg, var(--surface)); stroke: var(--node-edge, var(--line)); stroke-width: 1; filter: drop-shadow(0 2px 6px rgba(0, 0, 0, .12)); }
  .node .hdr { fill: color-mix(in srgb, var(--c) 9%, var(--node-bg, var(--surface))); }
  .node .band { fill: var(--c); }
  .node .sep { stroke: var(--line); }
  .node .nled { fill: var(--c); }
  .node .nt { font: 650 15px var(--font); fill: var(--fg); }
  .node .ns { font: 500 11px var(--mono); fill: var(--fg-3); }
  .node .drag { cursor: move; }
  .node.panel .box { stroke-dasharray: 5 3; }
  .port { cursor: pointer; }
  .port .ph { fill: transparent; }
  .port:hover .ph, .port.armed .ph { fill: color-mix(in srgb, var(--c) 22%, transparent); }
  .port.armed .ph { stroke: var(--c); stroke-width: 2; }
  .port :global(.cg-shell) { fill: var(--node-bg, var(--surface)); stroke: var(--c); stroke-width: 2.2; }
  .port.used :global(.cg-shell) { fill: color-mix(in srgb, var(--c) 30%, var(--node-bg, var(--surface))); }
  .port :global(.cg-ring) { fill: none; stroke: var(--c); stroke-width: 1.2; opacity: .75; }
  .port :global(.cg-pin), .port :global(.cg-key) { fill: var(--c); }
  .cchip rect { fill: color-mix(in srgb, var(--fg) 7%, transparent); stroke: color-mix(in srgb, var(--fg) 14%, transparent); stroke-width: .8; }
  .cchip text { font: 600 9.5px var(--mono); fill: var(--fg-3); }
  .pn { font: 500 13px var(--font); fill: var(--fg-2); }
  /* Câbles */
  .cable { cursor: pointer; }
  .cable .case { fill: none; stroke: var(--surface-2); stroke-width: 6; }
  .cable .vis { fill: none; stroke: var(--c); stroke-width: 2.2; stroke-linejoin: round; }
  .cable .hit { fill: none; stroke: transparent; stroke-width: 14; }
  .cable.warn .vis { stroke-dasharray: 7 4; }
  .cable.sel .vis { stroke-width: 3.6; filter: drop-shadow(0 0 4px var(--c)); }
  .cable.stub .vis { stroke-dasharray: 2 4; }
  .far rect { fill: var(--surface); stroke: var(--c); stroke-width: 1.2; }
  .far text { font: 600 11px var(--font); fill: var(--fg-2); }
  .lbl { cursor: pointer; }
  .lbl .tag { fill: #FFFFFF; stroke: color-mix(in srgb, var(--c) 70%, #000); stroke-width: 1; }
  .lbl .bandc { fill: var(--c); }
  .lbl text { font: 700 11px var(--mono); fill: #15171A; }
  .lbl.none .tag { fill: color-mix(in srgb, #B78400 18%, #fff); stroke-dasharray: 3 2; }
  .lbl.sel .tag { stroke-width: 2.2; }
  .cable.hl .vis { stroke-width: 4; filter: drop-shadow(0 0 6px var(--err, #D33)); }
  .node.hl .box { stroke: var(--err, #D33); stroke-width: 3; }
  .counts { display: flex; gap: 6px; flex-wrap: wrap; }
  .cnt { padding: 4px 10px; border-radius: 99px; font-size: 13px; background: var(--surface-2); }
  .cnt.err b { color: var(--err); } .cnt.warn b { color: var(--warn, #B78400); }
  .issue { display: flex; gap: 8px; align-items: flex-start; padding: 8px 10px; border-radius: 10px; border: 0; text-align: left; cursor: pointer; font: inherit; color: inherit; background: var(--surface-2); }
  .issue span { display: flex; flex-direction: column; min-width: 0; }
  .issue.error { background: color-mix(in srgb, var(--err) 10%, transparent); }
  .issue.warning { background: color-mix(in srgb, var(--warn, #B78400) 12%, transparent); }
  .cable.attach .vis { stroke-width: 6; stroke-linecap: round; stroke-dasharray: none; opacity: .85; }
  .cable.attach .case { stroke-width: 10; }
  .cable.harnessed .case { stroke: var(--h); stroke-width: 9; opacity: .55; }
  .hrow { display: flex; gap: 6px; align-items: center; }
  .hname { flex: 1; display: flex; gap: 8px; align-items: center; flex-wrap: wrap; padding: 6px 8px; border: 0; border-radius: 10px; background: var(--surface-2); cursor: pointer; font: inherit; color: inherit; text-align: left; }
  .dot { display: inline-block; width: 10px; height: 10px; border-radius: 50%; background: var(--c); }
  .ov-loc .box { fill: var(--node-bg, var(--surface)); stroke: var(--line); stroke-width: 1.2; filter: drop-shadow(0 4px 12px rgba(0, 0, 0, .14)); }
  .ov-loc { cursor: pointer; }
  .ov-loc .drag { cursor: move; }
  .ov-code { font: 700 12px var(--mono); fill: var(--fg-3); letter-spacing: .08em; }
  .ov-name { font: 700 19px var(--font); fill: var(--fg); }
  .ov-k { font: 500 13px var(--font); fill: var(--fg-2); }
  .ov-out { font: 600 12.5px var(--font); fill: var(--accent, #4F6BFF); }
  .ov-open { font: 500 11px var(--font); fill: var(--fg-3); }
  .ov-line { stroke: var(--fg-2); opacity: .55; stroke-linecap: round; }
  .ov-line.ext { stroke-dasharray: 10 6; }
  .ov-count rect { fill: var(--surface); stroke: var(--line); }
  .ov-count text { font: 700 12px var(--mono); fill: var(--fg); }
  .preview { fill: none; stroke: var(--accent, #4F6BFF); stroke-width: 2; stroke-dasharray: 6 4; pointer-events: none; }
  @media (max-width: 1000px) { .syn-body { grid-template-columns: 1fr; height: auto; } .syn-canvas { height: 70vh; } .syn-side { border-left: 0; border-top: 1px solid var(--line); } }
</style>
