<script lang="ts">
  // Synoptique (section 10) : nœuds = exemplaires d'équipements, rangées de panels et unités
  // d'adaptateurs ; entrées à gauche, sorties à droite (panel : arrière à gauche, avant à
  // droite). Fonctionnement calqué sur Arkyo : palette d'outils (Sélection, Main, Câble,
  // Texte, Zone, Gomme), câblage clic-clic ou glisser avec aimantation et ports compatibles
  // surlignés, plage de ports en mode Multiple, extrémités rebranchables, tronçons
  // déplaçables, menus contextuels. Écran de conception : une personne à la fois.
  import { untrack, onDestroy } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import NumberingDialog from './NumberingDialog.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import { withLightTheme, standalone, raster, pdf as toPdf, save } from '../../lib/svgexport';
  import { PAPERS, type Paper } from '../../lib/sheet';
  import RemoteCursors from '../../components/RemoteCursors.svelte';
  import { fitText, shareRow, textWidth } from '../../lib/fit';
  import { routeAll, rounded, simplify, orthoTo, offsetPath, type Routed, type Req as RouteReq } from '../../lib/cableroute';
  import ProjectLinks from '../../components/ProjectLinks.svelte';
  import ViewPicker from '../../components/ViewPicker.svelte';
  import AnomalyGroups from '../../components/AnomalyGroups.svelte';
  import type { Anomaly } from '../../lib/anomalies';
  import { sendCursor, sendSelection, type PresenceMember } from '../../lib/live.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { route, navigate } from '../../lib/router.svelte';
  import { join } from '../../lib/live.svelte';
  import { relative, nf } from '../../lib/format';
  import { loadRefs, refById, fnColor } from '../../lib/catalog.svelte';
  import type { ProjectView } from '../../lib/projects';

  interface Port { ref: string; label: string; connector: string; signal: string | null; family: string | null; dir: 'in' | 'out' | 'bidir' | 'through'; gender: string | null; variant: string | null; side: 'l' | 'r' }
  interface SNode { key: string; kind: 'equipment' | 'panel' | 'adapter'; label: string; sub: string; function: string | null; container_id: string | null; location_id: string; ports: Port[] }
  interface Far { node: string | null; label: string; sub: string; port: string; connector: string; external?: boolean; location_id?: string | null; resolved?: boolean | null }
  interface Fields { number: string | null; color: string | null; style: string; waypoints: [number, number][]; notes: string | null; tags: string[]; cable_model_id: string | null; harness_id: string | null; length_m: number | null; attach?: boolean; external?: { location_id: string | null; label: string | null } | null }
  interface CableV { id: string; a: string; b: string; rev: number; fields: Fields; a_in_view: boolean; b_in_view: boolean; a_info: Far | null; b_info: Far | null; warnings: string[]; estimate_m: number | null }
  interface Ann { id: string; view: string; kind: 'zone' | 'label'; data: any; rev: number }
  interface Harness { id: string; name: string; color: string; rev: number }
  interface OvLoc { id: string; name: string; code: string; elements: number; weight_kg: number; power_w: number; outside: number }
  interface Overview { locations: OvLoc[]; links: { a: string; b: string; cables: string[]; external: number }[]; totals: { weight_kg: number; power_w: number } }
  interface Syn {
    view: string; nodes: SNode[]; cables: CableV[]; layout: Record<string, { x: number; y: number }>;
    lock: { account_id: string; account_name: string; taken_at: string; mine: boolean } | null;
    overview: Overview | null; harnesses: Harness[]; annotations: Ann[];
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
  // Sélections des autres sur cette vue : câble → couleur de la personne.
  let remoteSel = $state<Record<string, { member: PresenceMember; ids: string[] }>>({});
  const rsel = $derived(new Map(Object.values(remoteSel).flatMap((r) => r.ids.map((id) => [id, r.member.color] as const))));
  $effect(() => { const ids = [...new Set([...(sel ? [sel] : []), ...multi])]; if (syn?.view) sendSelection(`project:${pid}/syn:${syn.view}`, ids); });
  let numbering = $state(false);
  let tab = $state<'cable' | 'issues' | 'layers' | 'harness'>('cable');
  interface Issue { code: number; severity: 'error' | 'warning' | 'info'; title: string; subject: string; place: string | null; detail: string; cables: string[]; nodes: string[]; view: string | null }
  let issues = $state<{ counts: { error: number; warning: number; info: number }; issues: Issue[] } | null>(null);
  let hl = $state<{ cables: Set<string>; nodes: Set<string> }>({ cables: new Set(), nodes: new Set() });
  let pendingHl: Issue | null = null;
  async function validate() { try { issues = await api.get(`/projects/${pid}/synoptic/validate`); } catch (e) { fail(e); } }
  function pickCable(id: string, e: MouseEvent) {
    e.stopPropagation();
    if (tool === 'eraser') { if (mine) eraseCables([id]); return; }
    if ((tool !== 'select' && tool !== 'cable') || armed.length) return;
    const cb = syn?.cables.find((x) => x.id === id);
    if (cb && lockedFam.has(famOf(cb))) { toast('Calque verrouillé : déverrouillez-le pour choisir ce câble.', 'info'); return; }
    if (e.ctrlKey || e.metaKey || e.shiftKey) { const m = new Set(multi); if (m.has(id)) m.delete(id); else m.add(id); multi = m; }
    else multi = new Set([id]);
    sel = id; tab = 'cable'; selNodes = new Set(); selAnn = null;
  }
  function goIssue(i: Issue) {
    hl = { cables: new Set(i.cables), nodes: new Set(i.nodes) };
    if (i.view && syn && i.view !== syn.view) { pendingHl = i; navigate(viewUrl(i.view)); return; }
    if (i.cables[0]) { sel = i.cables[0]; multi = new Set(i.cables); }
    centerOn(i.cables[0] ? `[data-cable="${i.cables[0]}"]` : i.nodes[0] ? `[data-node="${i.nodes[0]}"]` : null);
  }
  /** Centre la vue sur un câble ou un nœud (après son rendu). */
  function centerOn(selector: string | null) {
    if (!selector) return;
    requestAnimationFrame(() => {
      const el = svgEl?.querySelector<SVGGraphicsElement>(selector);
      if (!el) return;
      const b = el.getBBox(), r = svgEl.getBoundingClientRect();
      if (k < 0.6) k = 0.8;
      tx = r.width / 2 - (b.x + b.width / 2) * k; ty = r.height / 2 - (b.y + b.height / 2) * k;
    });
  }
  // Anomalies de la validation et câbles à vérifier, regroupés par type.
  const issueList = $derived<Anomaly<Issue>[]>((issues?.issues ?? []).map((i) => ({ group: i.title, severity: i.severity, subject: i.subject, place: i.place, detail: i.detail, src: i })));
  let armed = $state<Port[]>([]);
  // Barre de câblage : nombre de câbles, tracé automatique ou multipoint, style, câbles parallèles.
  type CabPrefs = { count: 'simple' | 'multiple'; mode: 'auto' | 'multipoint'; style: 'orthogonal' | 'rounded' | 'arrow'; parallel: 'stacked' | 'spread'; askModel: boolean };
  let cab = $state<CabPrefs>({ count: 'simple', mode: 'auto', style: 'rounded', parallel: 'stacked', askModel: true, ...(() => { try { return JSON.parse(localStorage.getItem('syn-cab') ?? '{}'); } catch { return {}; } })() });
  $effect(() => { const v = JSON.stringify(cab); try { localStorage.setItem('syn-cab', v); } catch { /* indisponible */ } });
  /** Points de passage posés pendant un câblage multipoint. */
  let wps = $state<[number, number][]>([]);
  const snap8 = (v: number) => Math.round(v / 8) * 8;
  /** Style par défaut des prochains câbles ; appliqué aussi aux câbles sélectionnés. */
  async function setStyle(st: CabPrefs['style']) {
    cab.style = st;
    const ids = [...new Set([...(sel ? [sel] : []), ...multi])];
    const list = (syn?.cables ?? []).filter((c) => ids.includes(c.id) && c.fields.style !== st);
    if (!list.length || !mine) return;
    try {
      for (const c of list) await api.patch(`/cables/${c.id}`, { rev: c.rev, ...c.fields, style: st });
      toast(`Style appliqué à ${list.length} câble${list.length > 1 ? 's' : ''}.`);
      await load();
    } catch (e) { fail(e); await load(); }
  }
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
      if (pendingHl && pendingHl.view === s.view) { const i = pendingHl; pendingHl = null; if (i.cables[0]) { sel = i.cables[0]; multi = new Set(i.cables); } setTimeout(() => centerOn(i.cables[0] ? `[data-cable="${i.cables[0]}"]` : i.nodes[0] ? `[data-node="${i.nodes[0]}"]` : null), 300); }
      validate();
      // La position de chaque nœud est enregistrée une fois : les zones et la validation en dépendent.
      if (s.lock?.mine && !s.overview) {
        const missing = s.nodes.filter((n) => !s.layout[n.key] && pos[n.key]).map((n) => ({ node: n.key, ...pos[n.key]! }));
        if (missing.length) api.put(`/projects/${pid}/synoptic/layout`, { view: s.view, positions: missing }).catch(() => {});
      }
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

  // Toute sélection ramène l'onglet Propriétés (sauf pendant la validation).
  $effect(() => { if (sel || selNodes.size || selAnn || armed.length) untrack(() => { if (tab === 'layers' || tab === 'harness') tab = 'cable'; }); });
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
  const NW = 270, HH = 52, PS = 28, TAG_GAP = 26;
  const left = (n: SNode) => n.ports.filter((p) => p.side === 'l');
  const right = (n: SNode) => n.ports.filter((p) => p.side === 'r');
  // Libellés des ports par ligne : l'entrée et la sortie se partagent la place (mesurée en pixels).
  const portChip = (p: SNode['ports'][number]) => short(p.connector) + (p.gender === 'M' ? ' M' : p.gender === 'F' ? ' F' : '');
  const portText = (p: SNode['ports'][number]) => p.label;
  const chipWidth = (s: string) => s.length * 6.3 + 12;
  const fitCache = new Map<string, { l: string; r: string }[]>();
  function fittedLabels(n: SNode): { l: string; r: string }[] {
    const L = left(n), R = right(n);
    const key = n.key + '|' + n.ports.map((p) => `${p.side}${p.connector}${p.gender}${p.dir}${p.label}`).join('|');
    const hit = fitCache.get(key);
    if (hit) return hit;
    const rows: { l: string; r: string }[] = [];
    for (let i = 0; i < Math.max(L.length, R.length); i++) {
      const a = L[i], b = R[i];
      const from = 24, to = NW - 24;
      const [wl, wr] = shareRow(a ? textWidth(portText(a)) : null, b ? textWidth(portText(b)) : null, to - from);
      rows.push({ l: a ? fitText(portText(a), wl) : '', r: b ? fitText(portText(b), wr) : '' });
    }
    if (fitCache.size > 500) fitCache.clear();
    fitCache.set(key, rows);
    return rows;
  }
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
  // Étiquette d'extrémité : connecteur près du port, puis numéro (« — » s'il manque).
  const MONO_S = "600 9.5px 'JetBrains Mono', monospace", MONO_N = "700 11px 'JetBrains Mono', monospace";
  const connOf = (c: CableV) => short(portByRef(c.a)?.connector ?? portByRef(c.b)?.connector ?? c.a_info?.connector ?? '');
  const tagW = (c: CableV) => textWidth(connOf(c), MONO_S) + 14 + (c.fields.number ? textWidth(c.fields.number, MONO_N) + 8 : 14);
  const boxes = $derived(syn ? nodes.map((n) => { const p = pos[n.key] ?? { x: 0, y: 0 }; return [p.x - 6, p.y - 8, p.x + NW + 6, p.y + nodeH(n) + 8]; }) : []);
  // Routage global : couloirs attribués à tous les câbles de la vue à la fois (aucun
  // chevauchement, écart constant, faisceaux parallèles, nœuds contournés).
  const routed = $derived.by(() => {
    if (!syn || syn.overview) return new Map<string, Routed>();
    const reqs: RouteReq[] = [], fixed: [number, number][][] = [];
    for (const c of syn.cables) {
      if (!c.a_in_view || !c.b_in_view || hiddenFam.has(famOf(c))) continue;
      const A = portXY(c.a), B = portXY(c.b); if (!A || !B) continue;
      if (liveWps(c).length) { const P = cablePts(c); if (P) fixed.push(P); continue; } // tracé posé à la main : obstacle
      reqs.push({ id: c.id, a: { x: A[0], y: A[1], side: A[2] }, b: { x: B[0], y: B[1], side: B[2] }, out: tagW(c) + TAG_GAP * 2 + 12 });
    }
    return routeAll(reqs, boxes as [number, number, number, number][], fixed);
  });
  /** Tracé en cours de modification (tronçon déplacé à la main). */
  let ovr = $state<Record<string, [number, number][]> | null>(null);
  const dedupe = (pts: [number, number][]) => pts.filter((q, i) => i === 0 || q[0] !== pts[i - 1]![0] || q[1] !== pts[i - 1]![1]);
  /** Nœuds en cours de déplacement : les tracés manuels suivent (deux bouts déplacés) ou
      repassent en automatique (un seul bout déplacé), sans coude ni croisement parasite. */
  let moving = $state<{ keys: Set<string>; dx: number; dy: number } | null>(null);
  function liveWps(c: CableV): [number, number][] {
    const wp = c.fields.waypoints ?? [];
    if (!wp.length || !moving) return wp;
    const a = moving.keys.has(nodeOf(c.a)?.key ?? ''), b = moving.keys.has(nodeOf(c.b)?.key ?? '');
    if (a && b) return wp.map(([x, y]) => [x + moving!.dx, y + moving!.dy] as [number, number]);
    if (!a && !b) return wp;
    return shiftEnd(c, wp, a ? 'a' : 'b', moving.dx, moving.dy);
  }
  /** Un seul bout déplacé : seul le coude voisin suit (sortie du port, puis le tronçon qui la
      prolonge) ; le reste du tracé ne bouge pas. Si la sortie se retrouve derrière le port,
      le câble repasse en automatique. */
  function shiftEnd(c: CableV, wp: [number, number][], end: 'a' | 'b', dx: number, dy: number): [number, number][] {
    const P = portXY(end === 'a' ? c.a : c.b); if (!P) return [];
    const w = (end === 'a' ? wp : [...wp].reverse()).map((q) => [...q] as [number, number]);
    const [ox, oy] = w[0]!;
    if (oy !== P[1] - dy) return []; // ancien tracé sans sortie horizontale
    w[0] = [ox + dx, oy + dy];
    if (w[1]) { if (w[1][0] === ox) w[1][0] += dx; else if (w[1][1] === oy) w[1][1] += dy; }
    if (P[2] === 'r' ? w[0][0] < P[0] + 20 : w[0][0] > P[0] - 20) return [];
    return end === 'a' ? w : w.reverse();
  }
  function cablePts(c: CableV): [number, number][] | null {
    const A = portXY(c.a), B = portXY(c.b);
    if (!A || !B) return null;
    if (ovr?.[c.id]) return ovr[c.id]!;
    const wp = liveWps(c);
    if (wp.length) {
      // Multipoint : sortie horizontale du port, points de passage, entrée horizontale.
      const out = TAG_GAP * 2 + tagW(c);
      const exA: [number, number] = [A[0] + (A[2] === 'r' ? out : -out), A[1]], exB: [number, number] = [B[0] + (B[2] === 'r' ? out : -out), B[1]];
      const raw: [number, number][] = [[A[0], A[1]], ...(wp[0]![1] === A[1] ? [] : [exA]), ...wp, ...(wp[wp.length - 1]![1] === B[1] ? [] : [exB]), [B[0], B[1]]];
      const pts: [number, number][] = [raw[0]!];
      for (let i = 1; i < raw.length; i++) {
        const p = pts[pts.length - 1]!, q = raw[i]!;
        if (p[0] !== q[0] && p[1] !== q[1]) pts.push([q[0], p[1]]);
        pts.push(q);
      }
      return dedupe(pts);
    }
    const r = routed.get(c.id);
    return r ? r.pts : [[A[0], A[1]], [B[0], B[1]]];
  }
  const arrowHead = (x2: number, y2: number, a: number) => {
    const L = 11, W = 5, bx = x2, by = y2;
    return `M${bx} ${by}L${bx - Math.cos(a) * L + Math.sin(a) * W} ${by - Math.sin(a) * L - Math.cos(a) * W}L${bx - Math.cos(a) * L - Math.sin(a) * W} ${by - Math.sin(a) * L + Math.cos(a) * W}Z`;
  };
  function cPath(c: CableV) {
    const A = portXY(c.a), B = portXY(c.b);
    if (!A || !B) return null;
    const w = tagW(c);
    const tagAt = (sd: 'l' | 'r', x: number, y: number) => (sd === 'r' ? [x + TAG_GAP, y] : [x - TAG_GAP - w, y]) as [number, number];
    const st = c.fields.style;
    if (st === 'arrow') {
      // Flèche (comme Arkyo) : pas de trait de bout en bout, un renvoi à chaque extrémité
      // terminé par une pointe, avec l'équipement et le port d'en face.
      const ends = ([[A, c.b], [B, c.a]] as const).map(([P, other]) => {
        const dx = P[2] === 'r' ? 1 : -1, end = P[0] + dx * (w + TAG_GAP * 2 + 50);
        const n = nodeOf(other), pt = n?.ports.find((x) => x.ref === other);
        return { d: `M${P[0]} ${P[1]}H${end}`, ah: arrowHead(end + dx * 8, P[1], dx > 0 ? 0 : Math.PI), ex: end + dx * 8, ey: P[1], dx, far: n && pt ? `${n.label} › ${pt.label}` : '?' };
      });
      return { d: ends.map((e) => e.d).join(''), ah: ends.map((e) => e.ah).join(''), t1: tagAt(A[2], A[0], A[1]), t2: tagAt(B[2], B[0], B[1]), pts: null, renvoi: ends };
    }
    const pts = cablePts(c)!;
    const d = st === 'rounded' ? rounded(pts, 12) : rounded(pts, 4);
    return { d, ah: '', t1: tagAt(A[2], A[0], A[1]), t2: tagAt(B[2], B[0], B[1]), pts, renvoi: null };
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
  const dups = $derived.by(() => { const seen = new Map<string, number>(); for (const c of syn?.cables ?? []) if (c.fields.number && !c.fields.attach) seen.set(c.fields.number, (seen.get(c.fields.number) ?? 0) + 1); return new Set([...seen].filter(([, n]) => n > 1).map(([x]) => x)); });
  const warnCount = $derived(syn?.cables.filter((c) => c.warnings.length).length ?? 0);
  // Câbles à vérifier : regroupés par constat, chacun nommé (numéro, sinon extrémité connue).
  const toCheck = $derived<Anomaly<string>[]>((syn?.cables ?? []).flatMap((c) => c.warnings.map((w) => {
    const ends = [endName(c.a, c.a_info), endName(c.b, c.b_info)].filter((x) => x !== 'Extrémité supprimée');
    const gone = w.includes('n’existe') || w.includes("n'existe");
    return { group: gone ? 'Extrémité manquante' : 'À vérifier', severity: 'warning', subject: c.fields.number ?? 'Câble non numéroté', place: c.fields.number ? null : (ends[0] ?? null), detail: w, src: c.id };
  })));
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
    // Pavé tactile : deux doigts font défiler, le pincement (Ctrl) zoome. Souris : la molette
    // (crans entiers, sans défilement horizontal) zoome vers le curseur.
    const mouseWheel = e.deltaMode === 1 || (e.deltaX === 0 && Number.isInteger(e.deltaY) && Math.abs(e.deltaY) >= 40);
    if (!e.ctrlKey && !mouseWheel) { tx -= e.deltaX; ty -= e.deltaY; return; }
    const r = svgEl.getBoundingClientRect(), mx = e.clientX - r.left, my = e.clientY - r.top;
    const f = e.ctrlKey && !mouseWheel ? Math.exp(-e.deltaY * 0.01) : e.deltaY < 0 ? 1.12 : 1 / 1.12;
    const nk = Math.min(2.5, Math.max(0.2, k * f));
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
    k = Math.min(1.2, Math.max(0.2, Math.min(r.width / w, (r.height - 60) / h)));
    tx = -Math.min(...xs) * k + (r.width - w * k) / 2; ty = -Math.min(...ys) * k + 60;
  }
  let fitted = '';
  function start() {
    fit();
    if (k < 0.6) { k = 0.75; tx = 10; ty = 10; }
  }
  $effect(() => { if (syn && svgEl && fitted !== syn.view) { fitted = syn.view; untrack(start); } });

  // ----- Historique : annuler / rétablir sur 50 états -----
  interface Step { label: string; undo: () => Promise<unknown>; redo: () => Promise<unknown> }
  let past = $state<Step[]>([]);
  let future = $state<Step[]>([]);
  function push(st: Step) { past = [...past.slice(-49), st]; future = []; }
  async function undo() {
    const st = past.at(-1); if (!st || !mine) return;
    past = past.slice(0, -1);
    try { await st.undo(); future = [...future, st]; toast(`Annulé : ${st.label}.`, 'info'); }
    catch (e) { fail(e); past = []; future = []; }
    await load();
  }
  async function redo() {
    const st = future.at(-1); if (!st || !mine) return;
    future = future.slice(0, -1);
    try { await st.redo(); past = [...past, st]; toast(`Rétabli : ${st.label}.`, 'info'); }
    catch (e) { fail(e); past = []; future = []; }
    await load();
  }
  const findCable = (a: string, b: string) => syn?.cables.find((c) => (c.a === a && c.b === b) || (c.a === b && c.b === a));
  /** Repose des câbles supprimés (câbles, attaches ou ports externes) avec leurs champs. */
  async function restore(list: CableV[]) {
    const plain = list.filter((c) => !c.fields.attach && !c.fields.external);
    if (plain.length) await api.post(`/projects/${pid}/cables`, { cables: plain.map((c) => ({ ...c.fields, a: c.a, b: c.b })) });
    for (const c of list.filter((x) => x.fields.attach)) await api.post(`/projects/${pid}/attach`, { adapter: c.b.startsWith('a:') ? c.b : c.a, port: c.b.startsWith('a:') ? c.a : c.b });
    for (const c of list.filter((x) => x.fields.external)) await api.post(`/projects/${pid}/external`, { port: c.a, ...c.fields.external, number: c.fields.number });
  }

  // ----- Outils et annotations -----
  type Tool = 'select' | 'pan' | 'cable' | 'lasso' | 'zone' | 'label' | 'eraser';
  let tool = $state<Tool>('select');
  const TOOLS: { id: Tool; icon: string; label: string; key: string; edit: boolean }[] = [
    { id: 'select', icon: 'cursor', label: 'Sélection', key: 'V', edit: false },
    { id: 'pan', icon: 'hand', label: 'Main', key: 'H', edit: false },
    { id: 'lasso', icon: 'lasso', label: 'Lasso', key: 'L', edit: false },
    { id: 'cable', icon: 'cable', label: 'Câble', key: 'C', edit: true },
    { id: 'label', icon: 'text', label: 'Texte', key: 'T', edit: true },
    { id: 'zone', icon: 'square', label: 'Zone', key: 'R', edit: true },
    { id: 'eraser', icon: 'eraser', label: 'Gomme', key: 'E', edit: true },
  ];
  function setTool(t: Tool) { if (t !== 'cable') { armed = []; wps = []; mouse = null; } tool = t; }
  /** Espace maintenu : main temporaire. */
  let spacePan = $state(false);
  /** Nœuds sélectionnés (clic sur l'en-tête, Maj pour ajouter, ou rectangle). */
  let selNodes = $state<Set<string>>(new Set());
  const nodeSel = $derived(selNodes.size === 1 ? (syn?.nodes.find((n) => selNodes.has(n.key)) ?? null) : null);
  const cablesOfNode = (key: string) => { const n = syn?.nodes.find((x) => x.key === key); const refs = new Set(n?.ports.map((p) => p.ref) ?? []); return (syn?.cables ?? []).filter((c) => refs.has(c.a) || refs.has(c.b)); };

  // ----- Compatibilité (comme Arkyo : jamais bloquante, signalée pendant le tracé) -----
  function incompat(src: Port, dst: Port): string[] {
    const r: string[] = [];
    if ((src.dir === 'in' || src.dir === 'out') && src.dir === dst.dir) r.push(src.dir === 'in' ? 'Deux entrées' : 'Deux sorties');
    if (src.signal && dst.signal) {
      if (src.connector !== dst.connector) r.push(`Connecteurs différents (${short(src.connector)} / ${short(dst.connector)})`);
      if (src.signal !== dst.signal) r.push(`Signaux différents (${refById('signals', src.signal)?.name ?? src.signal} / ${refById('signals', dst.signal)?.name ?? dst.signal})`);
    }
    if (used.has(dst.ref)) r.push('Port déjà câblé : le câble sera remplacé');
    return r;
  }
  const armedRefs = $derived(new Set(armed.map((a) => a.ref)));
  /** Port survolé pendant un tracé, pour l'infobulle et l'aperçu des ports d'arrivée. */
  let hoverPort = $state<{ p: Port; n: SNode; sx: number; sy: number } | null>(null);
  /** Port aimanté (30 px autour du curseur) pendant un tracé ou un rebranchement. */
  let snap = $state<{ p: Port; n: SNode } | null>(null);
  function nearestPort(wx: number, wy: number, skip: Set<string>): { p: Port; n: SNode } | null {
    let best: { p: Port; n: SNode } | null = null, bd = 30;
    for (const n of nodes) {
      const at = pos[n.key]; if (!at || wx < at.x - 40 || wx > at.x + NW + 40 || wy < at.y - 40 || wy > at.y + nodeH(n) + 40) continue;
      for (const p of n.ports) { if (skip.has(p.ref)) continue; const P = portXY(p.ref); if (!P) continue; const d = Math.hypot(P[0] - wx, P[1] - wy); if (d < bd) { bd = d; best = { p, n }; } }
    }
    return best;
  }
  /** Ports d'arrivée prévus : consécutifs, du même côté, à partir du port visé. */
  const targetsFrom = (p: Port, n: SNode, count: number) => { const list = p.side === 'l' ? left(n) : right(n); const i = list.indexOf(p); return list.slice(i, i + count); };
  const aimed = $derived(snap ?? (hoverPort && !armedRefs.has(hoverPort.p.ref) ? { p: hoverPort.p, n: hoverPort.n } : null));
  const tgtPreview = $derived(armed.length && aimed ? new Set(targetsFrom(aimed.p, aimed.n, armed.length).map((p) => p.ref)) : new Set<string>());
  const okTargets = $derived.by(() => {
    if (!armed.length) return null;
    const src = armed[0]!, ok = new Set<string>();
    for (const n of nodes) for (const p of n.ports) if (!armedRefs.has(p.ref) && !incompat(src, p).length) ok.add(p.ref);
    return ok;
  });
  let selAnn = $state<string | null>(null);
  const anns = $derived(syn?.annotations ?? []);
  const annSel = $derived(anns.find((a) => a.id === selAnn) ?? null);
  let draft = $state<{ x: number; y: number; w: number; h: number } | null>(null);
  async function createAnn(kind: 'zone' | 'label', data: object) {
    if (!syn) return;
    try {
      const a = await api.post<Ann>(`/projects/${pid}/annotations`, { view: syn.view, kind, data });
      const keep = { ...a.data };
      push({ label: kind === 'zone' ? 'zone' : 'étiquette', undo: () => api.post(`/projects/${pid}/annotations/delete`, { ids: [findAnnLike(kind, keep)?.id ?? a.id] }), redo: () => api.post(`/projects/${pid}/annotations`, { view: syn!.view, kind, data: keep }) });
      await load(); selAnn = (syn?.annotations.find((x) => JSON.stringify(x.data) === JSON.stringify(keep)) ?? a).id; tool = 'select';
    } catch (e) { fail(e); }
  }
  const findAnnLike = (kind: string, data: object) => syn?.annotations.find((x) => x.kind === kind && JSON.stringify(x.data) === JSON.stringify(data));
  async function patchAnn(a: Ann, data: object, label = 'annotation') {
    const before = { ...a.data };
    const after = { ...a.data, ...data };
    try {
      await api.patch(`/annotations/${a.id}`, { rev: a.rev, data: after });
      push({ label, undo: async () => { const cur = findAnnLike(a.kind, after) ?? anns.find((x) => x.id === a.id); if (cur) await api.patch(`/annotations/${cur.id}`, { rev: cur.rev, data: before }); },
        redo: async () => { const cur = findAnnLike(a.kind, before) ?? anns.find((x) => x.id === a.id); if (cur) await api.patch(`/annotations/${cur.id}`, { rev: cur.rev, data: after }); } });
      await load();
    } catch (e) { fail(e); await load(); }
  }
  async function deleteAnn(a: Ann) {
    try {
      await api.post(`/projects/${pid}/annotations/delete`, { ids: [a.id] });
      const keep = { ...a.data };
      push({ label: 'suppression', undo: () => api.post(`/projects/${pid}/annotations`, { view: a.view, kind: a.kind, data: keep }), redo: async () => { const cur = findAnnLike(a.kind, keep); if (cur) await api.post(`/projects/${pid}/annotations/delete`, { ids: [cur.id] }); } });
      selAnn = null; await load();
    } catch (e) { fail(e); }
  }

  // ----- Calques par famille de signal -----
  let hiddenFam = $state<Set<string>>(new Set((() => { try { return JSON.parse(localStorage.getItem('syn-hidden') ?? '[]'); } catch { return []; } })()));
  function toggleFam(f: string) { const h = new Set(hiddenFam); if (h.has(f)) h.delete(f); else h.add(f); hiddenFam = h; try { localStorage.setItem('syn-hidden', JSON.stringify([...h])); } catch { /* stockage indisponible */ } }
  function isolateFam(f: string) { const all = families.filter((x) => x !== f); hiddenFam = new Set(all); try { localStorage.setItem('syn-hidden', JSON.stringify(all)); } catch { /* stockage indisponible */ } }
  // Calques par famille de signal ou par signal ; verrouillés ou renommés, par projet.
  const LKEY = (k: string) => `syn-${k}:${pid}`;
  const lsGet = <T,>(k: string, d: T): T => { try { const v = localStorage.getItem(LKEY(k)); return v ? JSON.parse(v) : d; } catch { return d; } };
  const lsSet = (k: string, v: unknown) => { try { localStorage.setItem(LKEY(k), JSON.stringify(v)); } catch { /* stockage indisponible */ } };
  let layerBy = $state<'family' | 'signal'>(lsGet('layerby', 'family'));
  let lockedFam = $state<Set<string>>(new Set(lsGet<string[]>('locked', [])));
  let famNames = $state<Record<string, string>>(lsGet('names', {}));
  function toggleLock(f: string) { const l = new Set(lockedFam); if (l.has(f)) l.delete(f); else l.add(f); lockedFam = l; lsSet('locked', [...l]); if (l.has(f)) multi = new Set([...multi].filter((id) => { const c = syn?.cables.find((x) => x.id === id); return !c || famOf(c) !== f; })); }
  function renameLayer(f: string) { const cur = famNames[f] ?? layerName(f, true); const n = prompt('Nom du calque :', cur); if (n === null) return; const m = { ...famNames }; if (n.trim()) m[f] = n.trim().slice(0, 60); else delete m[f]; famNames = m; lsSet('names', m); }
  const layerName = (f: string, raw = false) => (!raw && famNames[f]) || (f === '—' ? (layerBy === 'signal' ? 'Sans signal' : 'Sans famille') : layerBy === 'signal' ? (refById('signals', f)?.name ?? f) : (refById('functions', f)?.name ?? f));
  const famOf = (c: CableV) => layerBy === 'signal' ? (portByRef(c.a)?.signal ?? portByRef(c.b)?.signal ?? '—') : (portByRef(c.a)?.family ?? portByRef(c.b)?.family ?? '—');
  const layerColor = (f: string) => f === '—' ? 'var(--fg-3)' : layerBy === 'signal' ? ((refById('signals', f) as any)?.data?.color || fnColor((refById('signals', f) as any)?.data?.function)) : fnColor(f);
  const families = $derived([...new Set((syn?.cables ?? []).map(famOf))]);

  let drag: { kind: 'pan' | 'node' | 'ann' | 'resize' | 'draft' | 'lasso' | 'rewire' | 'seg' | 'port'; key?: string; ox: number; oy: number; moved: boolean; rect?: boolean; from?: { x: number; y: number; w?: number; h?: number };
    group?: Record<string, { x: number; y: number }>; sx?: number; sy?: number; end?: 'a' | 'b'; seg?: number; base?: [number, number][]; segs?: { id: string; k: number; base: [number, number][] }[] } | null = null;
  /** Extrémité en cours de rebranchement : position du curseur (aimantée). */
  let rewire = $state<{ id: string; end: 'a' | 'b'; at: [number, number] } | null>(null);
  // Sélection au rectangle (Maj + glisser) ou au lasso (outil Lasso).
  let lasso = $state<[number, number][] | null>(null);
  let annMulti = $state<Set<string>>(new Set());
  function inPoly(x: number, y: number, poly: [number, number][]) {
    let inside = false;
    for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) { const [xi, yi] = poly[i]!, [xj, yj] = poly[j]!; if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside; }
    return inside;
  }
  function finishLasso(poly: [number, number][], add: boolean) {
    if (!syn || poly.length < 3) return;
    const picked = new Set(add ? multi : []);
    for (const c of syn.cables) {
      if (hiddenFam.has(famOf(c)) || lockedFam.has(famOf(c))) continue;
      const A = portXY(c.a), B = portXY(c.b); const pts = [A, B].filter(Boolean) as [number, number, string][];
      if (!pts.length) continue;
      // Tracé réel du câble : il est pris si un de ses points est dans la zone, ou si un de
      // ses tronçons la traverse (sélection au rectangle ou au lasso, souris ou pavé tactile).
      const line: [number, number][] = (A && B ? cablePts(c) : null) ?? pts.map((p) => [p[0], p[1]] as [number, number]);
      const xs = poly.map((q) => q[0]), ys = poly.map((q) => q[1]);
      const bx0 = Math.min(...xs), bx1 = Math.max(...xs), by0 = Math.min(...ys), by1 = Math.max(...ys);
      const crosses = line.some((q, i) => {
        if (inPoly(q[0], q[1], poly)) return true;
        const n = line[i + 1]; if (!n) return false;
        // Échantillonne le tronçon (orthogonal le plus souvent) dans la boîte de la zone.
        if (Math.max(q[0], n[0]) < bx0 || Math.min(q[0], n[0]) > bx1 || Math.max(q[1], n[1]) < by0 || Math.min(q[1], n[1]) > by1) return false;
        const L = Math.hypot(n[0] - q[0], n[1] - q[1]), steps = Math.min(200, Math.max(2, Math.ceil(L / 6)));
        for (let t = 1; t < steps; t++) { const x = q[0] + ((n[0] - q[0]) * t) / steps, y = q[1] + ((n[1] - q[1]) * t) / steps; if (inPoly(x, y, poly)) return true; }
        return false;
      });
      if (crosses) picked.add(c.id);
    }
    const am = new Set<string>();
    for (const a of anns) { const cx = a.kind === 'zone' ? a.data.x + a.data.w / 2 : a.data.x, cy = a.kind === 'zone' ? a.data.y + a.data.h / 2 : a.data.y; if (inPoly(cx, cy, poly)) am.add(a.id); }
    const nm = new Set(add ? selNodes : []);
    for (const n of nodes) { const at = pos[n.key]; if (at && inPoly(at.x + NW / 2, at.y + nodeH(n) / 2, poly)) nm.add(n.key); }
    multi = picked; sel = [...picked][0] ?? null; annMulti = am; selAnn = am.size === 1 ? [...am][0]! : null; selNodes = nm;
    if (picked.size) tab = 'cable';
    const parts = [picked.size && `${picked.size} câble${picked.size > 1 ? 's' : ''}`, nm.size && `${nm.size} nœud${nm.size > 1 ? 's' : ''}`, am.size && `${am.size} annotation${am.size > 1 ? 's' : ''}`].filter(Boolean);
    if (parts.length) toast(`Sélection : ${parts.join(', ')}.`, 'info');
  }
  const panning = () => tool === 'pan' || spacePan;
  function down(e: PointerEvent) {
    if (e.button === 2) return;
    const t = e.target as Element;
    const [wx0, wy0] = toWorld(e);
    if (panning() || e.button === 1) { drag = { kind: 'pan', ox: e.clientX - tx, oy: e.clientY - ty, moved: false }; (e.currentTarget as Element).setPointerCapture(e.pointerId); return; }
    // Câblage en cours : un port aimanté termine le câble ; en multipoint, le fond pose un point.
    if (mine && armed.length && !t.closest('.port')) {
      if (snap) { const sp = snap; clickPort(sp.p, sp.n, e); return; }
      if (cab.mode === 'multipoint' && !t.closest('[data-node]')) { addWaypoint(wx0, wy0); e.stopPropagation(); return; }
    }
    // Poignées d'un câble sélectionné : extrémité à rebrancher, tronçon à déplacer.
    const hd = t.closest<SVGElement>('[data-handle]');
    if (hd && mine && selected) {
      const [kind, v, j] = hd.dataset.handle!.split(':');
      if (kind === 'end') { drag = { kind: 'rewire', ox: wx0, oy: wy0, moved: false, end: v as 'a' | 'b', key: selected.id }; rewire = { id: selected.id, end: v as 'a' | 'b', at: [wx0, wy0] }; }
      else { const segs = segGroup(v!, Number(j)); if (segs.length) drag = { kind: 'seg', ox: wx0, oy: wy0, moved: false, key: v, segs }; }
      (e.currentTarget as Element).setPointerCapture(e.pointerId); e.stopPropagation(); return;
    }
    if (t.closest('.port') || ((tool === 'select' || tool === 'cable' || tool === 'eraser') && (t.closest('.cable') || t.closest('.lbl')))) return;
    const nodeEl = t.closest<SVGGElement>('[data-node]');
    const annEl = t.closest<SVGGElement>('[data-ann]');
    if (tool === 'eraser') { if (mine && annEl) { const a = anns.find((x) => x.id === annEl.dataset.ann); if (a) deleteAnn(a); } return; }
    if (!syn?.overview && tool === 'lasso') {
      drag = { kind: 'lasso', ox: wx0, oy: wy0, moved: false, rect: false }; lasso = [[wx0, wy0]];
    } else if (mine && tool === 'zone') {
      drag = { kind: 'draft', ox: wx0, oy: wy0, moved: false }; draft = { x: wx0, y: wy0, w: 0, h: 0 };
    } else if (mine && tool === 'label') {
      createAnn('label', { x: Math.round(wx0), y: Math.round(wy0), text: 'Texte' });
      return;
    } else if (annEl) {
      const a = anns.find((x) => x.id === annEl.dataset.ann); if (!a) return;
      if (e.shiftKey) { const s2 = new Set(annMulti); if (selAnn) s2.add(selAnn); if (s2.has(a.id)) s2.delete(a.id); else s2.add(a.id); annMulti = s2; } else annMulti = new Set();
      selAnn = a.id; sel = null; multi = new Set(); selNodes = new Set();
      if (mine) drag = { kind: t.closest('.rs') ? 'resize' : 'ann', key: a.id, ox: wx0 - a.data.x, oy: wy0 - a.data.y, moved: false, from: { x: a.data.x, y: a.data.y, w: a.data.w, h: a.data.h } };
    } else if (nodeEl && !syn?.overview) {
      // Clic sur un nœud : il est sélectionné (Maj ou Ctrl pour ajouter) et se déplace avec
      // les autres nœuds sélectionnés.
      const key = nodeEl.dataset.node!;
      const ns = new Set(e.shiftKey || e.ctrlKey || e.metaKey ? selNodes : selNodes.has(key) ? selNodes : []);
      if ((e.ctrlKey || e.metaKey) && ns.has(key)) ns.delete(key); else ns.add(key);
      selNodes = ns; sel = null; multi = new Set(); selAnn = null; annMulti = new Set();
      if (mine && ns.has(key)) drag = { kind: 'node', key, ox: wx0, oy: wy0, moved: false, from: { ...pos[key]! }, group: Object.fromEntries([...ns].filter((k2) => pos[k2]).map((k2) => [k2, { ...pos[k2]! }])) };
    } else if (nodeEl && mine && t.closest('.drag')) {
      const key = nodeEl.dataset.node!;
      drag = { kind: 'node', key, ox: wx0, oy: wy0, moved: false, from: { ...pos[key]! }, group: { [key]: { ...pos[key]! } } };
    } else if (!nodeEl && !syn?.overview && (e.shiftKey || (tool === 'select' && !e.altKey))) {
      // Outil Sélection sur le fond : rectangle de sélection (Alt ou Main pour faire défiler).
      drag = { kind: 'lasso', ox: wx0, oy: wy0, moved: false, rect: true }; lasso = [[wx0, wy0]];
    } else if (!nodeEl) {
      selAnn = null; annMulti = new Set();
      drag = { kind: 'pan', ox: e.clientX - tx, oy: e.clientY - ty, moved: false };
      sel = null; multi = new Set(); selNodes = new Set();
    }
    if (drag) { (e.currentTarget as Element).setPointerCapture(e.pointerId); }
  }
  function addWaypoint(wx: number, wy: number) {
    // Points orthogonaux, comme Arkyo : un tronçon horizontal, puis vertical, et ainsi de suite.
    const P = portXY(armed[0]!.ref); if (!P) return;
    const base: [number, number] = wps.length ? wps[wps.length - 1]! : stubOf(P, armed[0]!.ref);
    const pt: [number, number] = wps.length % 2 === 0 ? [snap8(wx), base[1]] : [base[0], snap8(wy)];
    wps = [...wps, pt];
  }
  function move(e: PointerEvent) {
    if (syn?.view) { const w = toWorld(e); sendCursor(`project:${pid}/syn:${syn.view}`, w[0], w[1]); }
    const [wx, wy] = toWorld(e);
    if (armed.length || drag?.kind === 'rewire') {
      const skip = new Set(armedRefs);
      if (drag?.kind === 'rewire' && selected) { skip.add(selected.a); skip.add(selected.b); }
      snap = nearestPort(wx, wy, skip);
      const sp = snap ? portXY(snap.p.ref) : null;
      mouse = sp ? [sp[0], sp[1]] : [wx, wy];
      if (rewire) rewire = { ...rewire, at: mouse };
    }
    if (!drag) return;
    if (!drag.moved && Math.hypot(wx - drag.ox, wy - drag.oy) * k < 4 && drag.kind !== 'pan') return;
    drag.moved = true;
    if (drag.kind === 'pan') { tx = e.clientX - drag.ox; ty = e.clientY - drag.oy; return; }
    if (drag.kind === 'rewire' || drag.kind === 'port') return;
    if (drag.kind === 'seg') {
      // Tronçon : un horizontal monte ou descend, un vertical va à gauche ou à droite ; en
      // sélection multiple, le tronçon correspondant de chaque câble suit du même pas.
      const lead = drag.segs![0]!, p1 = lead.base[lead.k]!, p2 = lead.base[lead.k + 1]!;
      const horiz = p1[1] === p2[1];
      const delta = horiz ? snap8(p1[1] + wy - drag.oy) - p1[1] : snap8(p1[0] + wx - drag.ox) - p1[0];
      const next: Record<string, [number, number][]> = {};
      for (const sg of drag.segs!) {
        const pts = sg.base.map((q) => [...q] as [number, number]);
        if (horiz) { pts[sg.k]![1] += delta; pts[sg.k + 1]![1] += delta; } else { pts[sg.k]![0] += delta; pts[sg.k + 1]![0] += delta; }
        next[sg.id] = pts;
      }
      ovr = next;
      return;
    }
    if (drag.kind === 'lasso') {
      if (drag.rect) lasso = [[drag.ox, drag.oy], [wx, drag.oy], [wx, wy], [drag.ox, wy]];
      else { const l = lasso ?? []; const last = l[l.length - 1]; if (!last || Math.hypot(wx - last[0], wy - last[1]) > 4 / k) lasso = [...l, [wx, wy]]; }
      return;
    }
    if (drag.kind === 'draft') { draft = { x: Math.min(drag.ox, wx), y: Math.min(drag.oy, wy), w: Math.abs(wx - drag.ox), h: Math.abs(wy - drag.oy) }; return; }
    if (drag.kind === 'ann' || drag.kind === 'resize') {
      const a = anns.find((x) => x.id === drag!.key); if (!a) return;
      if (drag.kind === 'ann') { a.data.x = Math.round((wx - drag.ox) / 10) * 10; a.data.y = Math.round((wy - drag.oy) / 10) * 10; }
      else { a.data.w = Math.max(40, Math.round((wx - a.data.x) / 10) * 10); a.data.h = Math.max(40, Math.round((wy - a.data.y) / 10) * 10); }
      return;
    }
    // Nœuds : tout le groupe sélectionné bouge ; Maj verrouille l'axe ; aimantation à 10 et aux repères.
    let dx = wx - drag.ox, dy = wy - drag.oy;
    if (e.shiftKey) { if (Math.abs(dx) > Math.abs(dy)) dy = 0; else dx = 0; }
    const f = drag.from!;
    let nx = Math.round((f.x + dx) / 10) * 10, ny = Math.round((f.y + dy) / 10) * 10;
    const tol = 8 / k, n = nodes.find((x) => x.key === drag!.key);
    for (const gx of rulers ? guides.v : []) for (const off of [0, NW / 2, NW]) if (Math.abs(nx + off - gx) < tol) nx = gx - off;
    if (n && rulers) for (const gy of guides.h) for (const off of [0, nodeH(n) / 2, nodeH(n)]) if (Math.abs(ny + off - gy) < tol) ny = gy - off;
    const ddx = nx - f.x, ddy = ny - f.y;
    for (const [k2, p0] of Object.entries(drag.group ?? {})) pos[k2] = { x: p0.x + ddx, y: p0.y + ddy };
    moving = { keys: new Set(Object.keys(drag.group ?? {})), dx: ddx, dy: ddy };
  }
  async function up(e?: PointerEvent) {
    const d = drag; drag = null;
    if (d?.kind !== 'node' || !d.moved) moving = null;
    if (d?.kind === 'rewire') { const r = rewire, sp = snap; rewire = null; snap = null; mouse = null; if (r && sp && d.moved) rewireCable(r.id, r.end, sp.p.ref); return; }
    if (d?.kind === 'seg') { const o = ovr; if (o && d.moved) await setWaypoints(Object.entries(o).map(([id, pts]) => ({ id, way: simplify(pts).slice(1, -1) })), Object.keys(o).length > 1 ? `tracé de ${Object.keys(o).length} câbles` : 'tracé du câble'); ovr = null; return; }
    if (d?.kind === 'port') { if (d.moved && snap && armed.length) { const sp = snap; clickPort(sp.p, sp.n, e as unknown as MouseEvent); } return; }
    if (d?.kind === 'lasso') { const poly = lasso; lasso = null; if (poly && d.moved) finishLasso(poly, !!e?.ctrlKey || !!e?.metaKey || !!e?.shiftKey); else if (!d.moved) { sel = null; multi = new Set(); selNodes = new Set(); selAnn = null; annMulti = new Set(); } return; }
    if (d?.kind === 'draft') { const z = draft; draft = null; if (z && z.w >= 20 && z.h >= 20) createAnn('zone', { x: Math.round(z.x), y: Math.round(z.y), w: Math.round(z.w), h: Math.round(z.h) }); return; }
    if ((d?.kind === 'ann' || d?.kind === 'resize') && d.moved) {
      const a = anns.find((x) => x.id === d.key); if (!a) return;
      const moved = { x: a.data.x, y: a.data.y, w: a.data.w, h: a.data.h };
      Object.assign(a.data, d.from);
      patchAnn(a, d.kind === 'ann' ? { x: moved.x, y: moved.y } : { w: moved.w, h: moved.h }, d.kind === 'ann' ? 'déplacement' : 'taille');
      return;
    }
    if (d?.kind === 'node' && d.moved && syn) {
      lastAction = Date.now();
      const view = syn.view, keys = Object.keys(d.group ?? {}), from = d.group!;
      const to = Object.fromEntries(keys.map((k2) => [k2, { ...pos[k2]! }]));
      const ddx = to[d.key!]!.x - from[d.key!]!.x, ddy = to[d.key!]!.y - from[d.key!]!.y;
      const put = (p: Record<string, { x: number; y: number }>) => api.put(`/projects/${pid}/synoptic/layout`, { view, positions: keys.map((k2) => ({ node: k2, ...p[k2]! })) });
      // Tracés posés à la main : ils suivent en entier quand les deux bouts bougent, seul le
      // coude voisin suit quand un seul bout bouge (exactement ce qui était affiché).
      const moved = new Set(keys);
      const touched = syn.cables.filter((c) => c.fields.waypoints?.length && (moved.has(nodeOf(c.a)?.key ?? '') || moved.has(nodeOf(c.b)?.key ?? '')))
        .map((c) => ({ a: c.a, b: c.b, before: c.fields.waypoints, after: liveWps(c) }));
      const setWp = async (pick: 'before' | 'after') => { for (const t of touched) { const cur = findCable(t.a, t.b); if (cur) await api.patch(`/cables/${cur.id}`, { rev: cur.rev, ...cur.fields, waypoints: t[pick] }); } };
      try {
        await put(to);
        await setWp('after');
        push({ label: keys.length > 1 ? `déplacement de ${keys.length} nœuds` : 'déplacement', undo: async () => { await put(from); await setWp('before'); }, redo: async () => { await put(to); await setWp('after'); } });
        if (touched.length) await load();
      } catch (x) { fail(x); await load(); }
      moving = null;
    }
  }
  function portDown(p: Port, n: SNode, e: PointerEvent) {
    if (e.button !== 0 || panning()) return;
    e.stopPropagation();
    const [wx, wy] = toWorld(e);
    clickPort(p, n, e);
    // Glisser d'un port à l'autre : relâcher sur un port (ou près d'un port) pose le câble.
    if (armed.length) { drag = { kind: 'port', ox: wx, oy: wy, moved: false }; try { svgEl.setPointerCapture(e.pointerId); } catch { /* pointeur synthétique */ } }
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
    const cableOn = syn?.cables.find((x) => x.a === p.ref || x.b === p.ref);
    if (!mine) { if (cableOn) { sel = cableOn.id; multi = new Set([cableOn.id]); } return; }
    lastAction = Date.now();
    if (tool === 'eraser') { if (cableOn) eraseCables([cableOn.id]); return; }
    // Hors outil Câble : un port câblé montre son câble, un port libre lance le tracé.
    if (tool !== 'cable' && !armed.length) {
      if (cableOn && tool === 'select') { sel = cableOn.id; multi = new Set([cableOn.id]); selNodes = new Set(); tab = 'cable'; return; }
      setTool('cable');
    }
    if (!armed.length) { armed = [p]; mouse = null; extForm = null; sel = null; multi = new Set(); selNodes = new Set(); return; }
    if (armed.some((a) => a.ref === p.ref)) { armed = armed.filter((a) => a.ref !== p.ref); if (!armed.length) wps = []; return; }
    // Entrée d'adaptateur sur un port du même connecteur (ou l'inverse) : attache, sans câble.
    if (armed.length === 1 && n.kind !== 'adapter' && isAdIn(armed[0]!) && armed[0]!.connector === p.connector) { const a = armed[0]!; armed = []; mouse = null; attachTo(a, p); return; }
    if (armed.length === 1 && isAdIn(p) && nodeOf(armed[0]!.ref)?.kind !== 'adapter' && armed[0]!.connector === p.connector) { const t = armed[0]!; armed = []; mouse = null; attachTo(p, t); return; }
    const first = nodeOf(armed[0]!.ref);
    // Mode Multiple : un second port du même équipement et du même côté arme toute la plage
    // (Maj + clic : un port de plus ou de moins).
    if (first?.key === n.key && armed[0]!.side === p.side && (e.shiftKey || cab.count === 'multiple')) {
      if (e.shiftKey || armed.length > 1) { armed = [...armed, p]; return; }
      const list = p.side === 'l' ? left(n) : right(n);
      const i0 = list.indexOf(armed[0]!), i1 = list.indexOf(p);
      armed = list.slice(Math.min(i0, i1), Math.max(i0, i1) + 1);
      toast(`${armed.length} ports sources : cliquez le premier port d’arrivée.`, 'info');
      return;
    }
    // Cible : ports consécutifs du même côté, à partir du port cliqué.
    const srcs = [...armed].sort((a, b) => (portXY(a.ref)?.[1] ?? 0) - (portXY(b.ref)?.[1] ?? 0));
    const targets = targetsFrom(p, n, srcs.length);
    if (targets.length < srcs.length) toast(`${targets.length} / ${srcs.length} câbles posés : plus de port libre après « ${p.label} ».`, 'warn');
    if (!targets.length) return;
    // Multipoint : exactement le tracé de l'aperçu (faisceau parallèle en Écarté), prolongé
    // jusqu'à la sortie de chaque port d'arrivée ; les points intermédiaires sont enregistrés.
    const used2 = srcs.slice(0, targets.length);
    const T0 = portXY(targets[0]!.ref);
    const lines = cab.mode === 'multipoint' && wps.length && T0 ? bundle(used2, wps, stubOf(T0, targets[0]!.ref), cab.parallel === 'spread') : [];
    const body = { cables: used2.map((s2, i) => {
      const L = lines.find((l) => l.ref === s2.ref), Ti = portXY(targets[i]!.ref);
      const way = L && Ti ? simplify([...orthoTo(L.pts, stubOf(Ti, targets[i]!.ref)), [Ti[0], Ti[1]]]).slice(1, -1) : [];
      return { a: s2.ref, b: targets[i]!.ref, style: cab.style, waypoints: way };
    }) };
    // Les avertissements (signal, connecteur, sens) reviennent du serveur après la pose.
    armed = []; mouse = null; wps = []; snap = null; hoverPort = null;
    place(body, false);
  }
  async function place(body: { cables: { a: string; b: string; style?: string; waypoints?: [number, number][] }[] }, replace: boolean) {
    try {
      const r = await api.post<{ created: { id: string }[]; warnings: string[][] }>(`/projects/${pid}/cables`, { ...body, replace });
      push({ label: `${body.cables.length} câble(s)`, undo: async () => { const ids = body.cables.map((c) => findCable(c.a, c.b)?.id).filter(Boolean); if (ids.length) await api.post(`/projects/${pid}/cables/delete`, { ids }); }, redo: () => api.post(`/projects/${pid}/cables`, { ...body, replace: true }) });
      const w = r.warnings.flat();
      if (w.length) toast(`${r.created.length} câble(s) posé(s) · ${[...new Set(w)].join(' ')}`, 'warn');
      await load();
      if (r.created.length === 1) { sel = r.created[0]!.id; multi = new Set([sel]); }
      if (cab.askModel && r.created.length) askModel(r.created.map((x) => x.id));
    } catch (e) {
      if (e instanceof ApiError && e.status === 409 && e.code === 'occupied') confirmReplace = { body, n: body.cables.length };
      else { fail(e); await load(); }
    }
  }
  // ----- Modèle de câble (proposé après chaque pose, comme Arkyo) -----
  interface Model { id: string; name: string; reference: string | null; data: { function?: string | null } }
  let models = $state<Model[] | null>(null);
  let modelPick = $state<{ ids: string[]; family: string | null } | null>(null);
  async function loadModels() { if (models) return; try { models = (await api.get<{ items: Model[] }>('/catalog/items?kind=cable-models&active=true&limit=500')).items; } catch { models = []; } }
  async function askModel(ids: string[]) {
    await loadModels();
    if (!models?.length) return;
    const c = syn?.cables.find((x) => x.id === ids[0]);
    modelPick = { ids, family: c ? (portByRef(c.a)?.family ?? portByRef(c.b)?.family ?? null) : null };
  }
  const modelFit = (m: Model, family: string | null) => !family || !m.data.function ? 'unknown' : m.data.function === family ? 'ok' : 'bad';
  async function setModel(ids: string[], id: string | null) {
    try {
      for (const cid of ids) { const c = syn?.cables.find((x) => x.id === cid); if (c) await api.patch(`/cables/${c.id}`, { rev: c.rev, ...c.fields, cable_model_id: id }); }
      modelPick = null; await load();
    } catch (e) { fail(e); await load(); }
  }

  /** Modifie un câble avec annulation. */
  /** Tronçons à déplacer ensemble : celui saisi, et dans chaque autre câble sélectionné le
      tronçon intérieur de même orientation le plus proche (un faisceau garde ses écarts). */
  function segGroup(id: string, j: number): { id: string; k: number; base: [number, number][] }[] {
    const lead = syn?.cables.find((c) => c.id === id); const lp = lead && cablePts(lead);
    if (!lp || !lp[j + 1]) return [];
    const [a, b] = [lp[j]!, lp[j + 1]!], horiz = a[1] === b[1], mid = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
    const out = [{ id, k: j, base: lp.map((q) => [...q] as [number, number]) }];
    if (!multi.has(id) || multi.size < 2) return out;
    for (const c of syn!.cables) {
      if (c.id === id || !multi.has(c.id) || c.fields.attach || c.fields.style === 'arrow' || !c.a_in_view || !c.b_in_view) continue;
      const pts = cablePts(c); if (!pts) continue;
      let best = -1, bd = Infinity;
      for (let i = 1; i + 2 < pts.length; i++) {
        const p = pts[i]!, q = pts[i + 1]!;
        if ((p[1] === q[1]) !== horiz) continue;
        const d = Math.hypot((p[0] + q[0]) / 2 - mid[0]!, (p[1] + q[1]) / 2 - mid[1]!);
        if (d < bd) { bd = d; best = i; }
      }
      // Seulement les tronçons voisins (même faisceau) : pas un tronçon à l'autre bout du schéma.
      if (best >= 0 && bd < 600) out.push({ id: c.id, k: best, base: pts.map((q) => [...q] as [number, number]) });
    }
    return out;
  }
  /** Enregistre les points de passage de plusieurs câbles en une seule étape d'annulation. */
  async function setWaypoints(list: { id: string; way: [number, number][] }[], label: string) {
    if (!syn || !list.length) return;
    lastAction = Date.now();
    const items = list.map(({ id, way }) => { const c = syn!.cables.find((x) => x.id === id)!; return { a: c.a, b: c.b, before: { ...c.fields }, after: { ...c.fields, waypoints: way } }; }).filter((x) => x.a);
    const apply = async (pick: 'before' | 'after') => { for (const it of items) { const cur = findCable(it.a, it.b); if (cur) await api.patch(`/cables/${cur.id}`, { rev: cur.rev, ...it[pick] }); } };
    try { await apply('after'); push({ label, undo: () => apply('before'), redo: () => apply('after') }); } catch (e) { fail(e); }
    await load();
  }
  async function patchCable(c: CableV, f: Partial<Fields>, label = 'modification du câble') {
    lastAction = Date.now();
    const before = { ...c.fields }, after = { ...c.fields, ...f }, a = c.a, b = c.b;
    const apply = async (fields: Fields) => { const cur = findCable(a, b); if (cur) await api.patch(`/cables/${cur.id}`, { rev: cur.rev, ...fields }); };
    try { await api.patch(`/cables/${c.id}`, { rev: c.rev, ...after }); push({ label, undo: () => apply(before), redo: () => apply(after) }); await load(); } catch (e) { fail(e); await load(); }
  }
  /** Rebranche une extrémité sur un autre port (le câble garde son numéro et ses réglages). */
  async function rewireCable(id: string, end: 'a' | 'b', to: string) {
    const c = syn?.cables.find((x) => x.id === id); if (!c || c.fields.attach || c.fields.external) return;
    const na = end === 'a' ? to : c.a, nb = end === 'b' ? to : c.b;
    if (na === c.a && nb === c.b) return;
    if (used.has(to) && !confirm('Ce port porte déjà un câble. Le remplacer ?')) return;
    const keep = { ...c, fields: { ...c.fields } };
    const moveTo = async (fa: string, fb: string, ta: string, tb: string) => {
      const cur = findCable(fa, fb); if (cur) await api.post(`/projects/${pid}/cables/delete`, { ids: [cur.id] });
      await api.post(`/projects/${pid}/cables`, { cables: [{ ...keep.fields, a: ta, b: tb }], replace: true });
    };
    try {
      await moveTo(c.a, c.b, na, nb);
      push({ label: 'rebranchement', undo: () => moveTo(na, nb, keep.a, keep.b), redo: () => moveTo(keep.a, keep.b, na, nb) });
      await load();
      const nc = findCable(na, nb); if (nc) { sel = nc.id; multi = new Set([nc.id]); }
      toast('Câble rebranché.');
    } catch (e) { fail(e); await load(); }
  }
  /** Gomme : supprime sans confirmation (annulable). */
  async function eraseCables(ids: string[]) {
    const gone = (syn?.cables ?? []).filter((c) => ids.includes(c.id)).map((c) => ({ ...c, fields: { ...c.fields } }));
    if (!gone.length) return;
    try {
      await api.post(`/projects/${pid}/cables/delete`, { ids });
      push({ label: 'gomme', undo: () => restore(gone), redo: async () => { const again = gone.map((g) => findCable(g.a, g.b)?.id).filter(Boolean); if (again.length) await api.post(`/projects/${pid}/cables/delete`, { ids: again }); } });
      if (sel && ids.includes(sel)) sel = null;
      await load();
    } catch (e) { fail(e); }
  }

  // ----- Menus contextuels -----
  interface CtxItem { label: string; act?: () => void; danger?: boolean; sep?: boolean; disabled?: boolean }
  let ctx = $state<{ x: number; y: number; title?: string; items: CtxItem[] } | null>(null);
  function contextMenu(e: MouseEvent) {
    e.preventDefault();
    if (!syn || syn.overview) return;
    const t = e.target as Element, r = stageEl.getBoundingClientRect();
    const at = { x: e.clientX - r.left, y: e.clientY - r.top };
    const cEl = t.closest<SVGElement>('[data-cable]'), nEl = t.closest<SVGElement>('[data-node]'), aEl = t.closest<SVGElement>('[data-ann]');
    armed = []; wps = []; mouse = null;
    if (cEl) {
      const id = cEl.dataset.cable!; const c = syn.cables.find((x) => x.id === id); if (!c) return;
      if (!multi.has(id)) { multi = new Set([id]); }
      sel = id; selNodes = new Set(); tab = 'cable';
      const ids = [...multi], n = ids.length;
      ctx = { ...at, title: n > 1 ? `${n} câbles` : (c.fields.number ?? 'Câble sans numéro'), items: [
        ...(mine ? [
          { label: n > 1 ? `Créer un faisceau (${n})` : 'Ranger dans un faisceau…', act: () => (newHarness = { name: '', color: '#2459E0' }) },
          ...(c.fields.harness_id ? [{ label: 'Retirer du faisceau', act: () => assign(null) }] : []),
          { label: n > 1 ? 'Numéroter la sélection…' : 'Numéroter…', act: () => (numbering = true) },
          ...(c.fields.number && n === 1 ? [{ label: 'Effacer le numéro', act: () => patchCable(c, { number: null }, 'effacement du numéro') }] : []),
          ...(ids.some((id) => syn!.cables.find((x) => x.id === id)?.fields.waypoints?.length) ? [{ label: n > 1 ? 'Repasser les tracés en automatique' : 'Repasser le tracé en automatique', act: () => setWaypoints(ids.filter((id) => syn!.cables.find((x) => x.id === id)?.fields.waypoints?.length).map((id) => ({ id, way: [] })), 'tracé automatique') }] : []),
          { label: 'Style : orthogonal', act: () => setStyle('orthogonal') }, { label: 'Style : arrondi', act: () => setStyle('rounded') }, { label: 'Style : flèche (renvoi)', act: () => setStyle('arrow') },
          { label: 'Modèle de câble…', act: () => askModel(ids) },
          { label: n > 1 ? `Supprimer les ${n} câbles` : 'Supprimer le câble', danger: true, sep: true, act: () => (confirmDelete = true) },
        ] : [{ label: 'Propriétés', act: () => {} }]),
      ] };
    } else if (nEl) {
      const key = nEl.dataset.node!; const n = syn.nodes.find((x) => x.key === key); if (!n) return;
      if (!selNodes.has(key)) selNodes = new Set([key]);
      sel = null; multi = new Set();
      const cs = cablesOfNode(key);
      ctx = { ...at, title: n.label, items: [
        { label: `Sélectionner ses câbles (${cs.length})`, disabled: !cs.length, act: () => { multi = new Set(cs.map((c) => c.id)); sel = cs[0]?.id ?? null; selNodes = new Set(); } },
        ...(mine ? [{ label: 'Numéroter ses câbles…', disabled: !cs.length, act: () => { multi = new Set(cs.map((c) => c.id)); sel = cs[0]?.id ?? null; numbering = true; } },
          { label: 'Décâbler cet équipement', danger: true, disabled: !cs.length, sep: true, act: () => { multi = new Set(cs.map((c) => c.id)); sel = cs[0]?.id ?? null; confirmDelete = true; } }] : []),
      ] };
    } else if (aEl) {
      const a = anns.find((x) => x.id === aEl.dataset.ann); if (!a) return;
      selAnn = a.id; sel = null; multi = new Set(); selNodes = new Set();
      ctx = { ...at, title: a.kind === 'zone' ? (a.data.name || 'Zone') : 'Texte', items: [
        { label: 'Copier', act: () => copyAnns() },
        ...(mine ? [{ label: 'Supprimer', danger: true, sep: true, act: () => deleteAnn(a) }] : []),
      ] };
    } else {
      ctx = { ...at, items: [
        { label: 'Tout afficher (F)', act: fit },
        { label: 'Centrer', act: () => { const [cx, cy] = centerOf(); tx = stageW / 2 - cx * k; ty = stageH / 2 - cy * k; } },
        ...(mine && clip.length ? [{ label: 'Coller', act: pasteAnns }] : []),
        { label: 'Raccourcis clavier (?)', sep: true, act: () => (help = true) },
      ] };
    }
  }
  const centerOf = (): [number, number] => { const xs = nodes.map((n) => pos[n.key]!.x + NW / 2), ys = nodes.map((n) => pos[n.key]!.y + nodeH(n) / 2); return xs.length ? [(Math.min(...xs) + Math.max(...xs)) / 2, (Math.min(...ys) + Math.max(...ys)) / 2] : [0, 0]; };
  let help = $state(false);
  let rulers = $state(lsGet('rulers', false));
  let showMap = $state(lsGet('map', true));
  let moreEl = $state<HTMLDetailsElement | null>(null);

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
  async function patchSel(f: Partial<Fields>) { if (selected) await patchCable(selected, f); }
  async function deleteSel() {
    const ids = multi.size > 1 ? [...multi] : selected ? [selected.id] : [];
    if (!ids.length) return;
    const gone = (syn?.cables ?? []).filter((c) => ids.includes(c.id)).map((c) => ({ ...c, fields: { ...c.fields } }));
    try {
      await api.post(`/projects/${pid}/cables/delete`, { ids }); sel = null; multi = new Set(); confirmDelete = false;
      push({ label: `suppression de ${gone.length} câble(s)`, undo: () => restore(gone), redo: async () => { const again = gone.map((g) => findCable(g.a, g.b)?.id).filter(Boolean); if (again.length) await api.post(`/projects/${pid}/cables/delete`, { ids: again }); } });
      await load();
    } catch (e) { fail(e); }
  }
  // Décâbler la vue (irréversible) et supprimer sa numérotation.
  let unplug = $state(false);
  async function unplugView() {
    const ids = (syn?.cables ?? []).map((c) => c.id);
    try { for (let i = 0; i < ids.length; i += 500) await api.post(`/projects/${pid}/cables/delete`, { ids: ids.slice(i, i + 500) }); toast(`${ids.length} câble(s) retiré(s).`); past = []; future = []; unplug = false; await load(); } catch (e) { fail(e); await load(); }
  }
  let unnumber = $state(false);
  async function unnumberView() {
    try { const r = await api.post<{ changed: number }>(`/projects/${pid}/numbering`, { mode: 'clear', view: syn?.view }); toast(`${r.changed} numéro(s) effacé(s).`); unnumber = false; await load(); } catch (e) { fail(e); }
  }
  function key(e: KeyboardEvent) {
    if ((e.target as Element).closest('input,textarea,select')) return;
    if (e.key === 'Backspace' && armed.length && wps.length) { e.preventDefault(); wps = wps.slice(0, -1); return; }
    if (e.key === 'Escape') { ctx = null; wps = []; armed = []; mouse = null; snap = null; sel = null; multi = new Set(); selNodes = new Set(); hl = { cables: new Set(), nodes: new Set() }; }
    if (e.key === ' ' && !e.repeat) { spacePan = true; e.preventDefault(); return; }
    // Raccourcis d'Arkyo (sans modificateur).
    if (!e.ctrlKey && !e.metaKey && !e.altKey && syn && !syn.overview) {
      const kk = e.key.toLowerCase();
      const toolKey: Record<string, Tool> = { v: 'select', s: 'select', h: 'pan', l: 'lasso', c: 'cable', t: 'label', r: 'zone', z: 'zone', e: 'eraser' };
      if (e.shiftKey && kk === 'a') { cab.mode = 'auto'; wps = []; return; }
      if (!e.shiftKey && toolKey[kk] && (!TOOLS.find((x) => x.id === toolKey[kk])!.edit || mine)) { setTool(toolKey[kk]!); return; }
      if (kk === '1') { cab.count = 'simple'; return; }
      if (kk === '2') { cab.count = 'multiple'; return; }
      if (kk === 'm') { cab.mode = 'multipoint'; return; }
      if (kk === 'q') { setStyle('orthogonal'); return; }
      if (kk === 'w') { setStyle('rounded'); return; }
      if (kk === 'n' && mine) { numbering = true; return; }
      if (kk === 'f') { fit(); return; }
      if (e.key === '?') { help = true; return; }
    }
    if ((e.key === 'Delete' || e.key === 'Backspace') && mine) {
      if (annMulti.size > 1) { e.preventDefault(); cutAnns(); }
      else if (annSel) { e.preventDefault(); deleteAnn(annSel); }
      else if (selected || multi.size) { e.preventDefault(); confirmDelete = true; }
      else if (selNodes.size) { e.preventDefault(); const cs = [...selNodes].flatMap((k2) => cablesOfNode(k2)); if (cs.length) { multi = new Set(cs.map((c) => c.id)); sel = cs[0]!.id; confirmDelete = true; } }
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z') { e.preventDefault(); if (e.shiftKey) redo(); else undo(); }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y') { e.preventDefault(); redo(); }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c') copyAnns();
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'x' && mine) { if (copyAnns()) cutAnns(); }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'v' && mine) { e.preventDefault(); pasteAnns(); }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a' && syn && !syn.overview) { e.preventDefault(); finishLasso([[-1e7, -1e7], [1e7, -1e7], [1e7, 1e7], [-1e7, 1e7]], false); }
  }
  // ----- Presse-papiers des zones et étiquettes -----
  let clip: { kind: 'zone' | 'label'; data: any }[] = [];
  let pasteN = 0;
  const selectedAnns = () => { const ids = new Set(annMulti); if (selAnn) ids.add(selAnn); return anns.filter((a) => ids.has(a.id)); };
  function copyAnns(): boolean {
    const list = selectedAnns(); if (!list.length) return false;
    clip = list.map((a) => ({ kind: a.kind, data: structuredClone($state.snapshot(a.data)) })); pasteN = 0;
    toast(`${list.length} annotation${list.length > 1 ? 's' : ''} copiée${list.length > 1 ? 's' : ''}.`, 'info'); return true;
  }
  async function cutAnns() {
    const list = selectedAnns(); if (!list.length || !syn) return;
    const keep = list.map((a) => ({ view: a.view, kind: a.kind, data: { ...a.data } }));
    try {
      await api.post(`/projects/${pid}/annotations/delete`, { ids: list.map((a) => a.id) });
      push({ label: 'couper', undo: async () => { for (const k2 of keep) await api.post(`/projects/${pid}/annotations`, k2); }, redo: async () => { for (const k2 of keep) { const c = findAnnLike(k2.kind, k2.data); if (c) await api.post(`/projects/${pid}/annotations/delete`, { ids: [c.id] }); } } });
      selAnn = null; annMulti = new Set(); await load();
    } catch (x) { fail(x); }
  }
  async function pasteAnns() {
    if (!clip.length || !syn) return;
    pasteN++;
    const view = syn.view, made: { kind: string; data: any }[] = [];
    try {
      for (const c of clip) { const data = { ...c.data, x: c.data.x + 30 * pasteN, y: c.data.y + 30 * pasteN }; await api.post(`/projects/${pid}/annotations`, { view, kind: c.kind, data }); made.push({ kind: c.kind, data }); }
      push({ label: 'coller', undo: async () => { for (const m2 of made) { const c = findAnnLike(m2.kind, m2.data); if (c) await api.post(`/projects/${pid}/annotations/delete`, { ids: [c.id] }); } }, redo: async () => { for (const m2 of made) await api.post(`/projects/${pid}/annotations`, { view, kind: m2.kind, data: m2.data }); } });
      await load();
      annMulti = new Set(made.map((m2) => findAnnLike(m2.kind, m2.data)?.id).filter(Boolean) as string[]); selAnn = annMulti.size === 1 ? [...annMulti][0]! : null;
    } catch (x) { fail(x); await load(); }
  }

  // ----- Règles et repères (par vue, sur ce poste) -----
  let guides = $state<{ v: number[]; h: number[] }>({ v: [], h: [] });
  $effect(() => { const v = syn?.view; if (v) guides = lsGet(`guides:${v}`, { v: [], h: [] }); });
  const saveGuides = () => { if (syn?.view) lsSet(`guides:${syn.view}`, guides); };
  let gdrag = $state<{ axis: 'v' | 'h'; i: number | null; at: number } | null>(null);
  let stageW = $state(800), stageH = $state(600), stageEl = $state<HTMLDivElement>(null!);
  function guideStart(e: PointerEvent, axis: 'v' | 'h', i: number | null) {
    e.stopPropagation(); e.preventDefault();
    const r = stageEl.getBoundingClientRect();
    const at = axis === 'v' ? (e.clientX - r.left - tx) / k : (e.clientY - r.top - ty) / k;
    gdrag = { axis, i, at };
    const mv = (ev: PointerEvent) => { if (!gdrag) return; gdrag = { ...gdrag, at: Math.round((gdrag.axis === 'v' ? (ev.clientX - r.left - tx) / k : (ev.clientY - r.top - ty) / k) / 5) * 5 }; };
    const end = (ev: PointerEvent) => {
      window.removeEventListener('pointermove', mv); window.removeEventListener('pointerup', end);
      if (!gdrag) return;
      const onRuler = gdrag.axis === 'v' ? ev.clientY - r.top < 20 : ev.clientX - r.left < 20;
      const list = [...guides[gdrag.axis]];
      if (gdrag.i != null) list.splice(gdrag.i, 1);
      if (!onRuler) list.push(gdrag.at);
      guides = { ...guides, [gdrag.axis]: list }; saveGuides(); gdrag = null;
    };
    window.addEventListener('pointermove', mv); window.addEventListener('pointerup', end);
  }
  const rulerStep = $derived.by(() => { for (const st of [20, 50, 100, 200, 500, 1000, 2000]) if (st * k >= 60) return st; return 5000; });
  const ticks = (from: number, to: number) => { const o: number[] = []; for (let v = Math.ceil(from / rulerStep) * rulerStep; v <= to && o.length < 300; v += rulerStep) o.push(v); return o; };

  // ----- Mini-carte -----
  const MM = { w: 180, h: 120 };
  const mm = $derived.by(() => {
    if (!syn || syn.overview || !nodes.length) return null;
    const xs: number[] = [], ys: number[] = [];
    for (const n of nodes) { const p = pos[n.key]; if (!p) continue; xs.push(p.x, p.x + NW); ys.push(p.y, p.y + nodeH(n)); }
    for (const a of anns) { xs.push(a.data.x, a.data.x + (a.data.w ?? 0)); ys.push(a.data.y, a.data.y + (a.data.h ?? 0)); }
    if (!xs.length) return null;
    const x0 = Math.min(...xs) - 100, y0 = Math.min(...ys) - 100, w = Math.max(...xs) - x0 + 100, h = Math.max(...ys) - y0 + 100;
    const sc = Math.min(MM.w / w, MM.h / h);
    return { x0, y0, sc, w: w * sc, h: h * sc };
  });
  function mmGo(e: PointerEvent) {
    if (!mm) return;
    const r = (e.currentTarget as Element).getBoundingClientRect();
    const go = (ev: PointerEvent) => { const wx = mm.x0 + (ev.clientX - r.left) / mm.sc, wy = mm.y0 + (ev.clientY - r.top) / mm.sc; tx = stageW / 2 - wx * k; ty = stageH / 2 - wy * k; };
    go(e);
    const mv = (ev: PointerEvent) => go(ev); const end = () => { window.removeEventListener('pointermove', mv); window.removeEventListener('pointerup', end); };
    window.addEventListener('pointermove', mv); window.addEventListener('pointerup', end);
  }
  let exp = $state<{ format: 'pdf' | 'png' | 'jpg'; paper: Paper; landscape: boolean; frame: 'all' | 'view'; revision: string; title: string; busy: boolean } | null>(null);
  async function doExport() {
    if (!exp || !syn || !svgEl) return;
    exp.busy = true;
    const keep = { sel, multi, hl, armed, selAnn };
    sel = null; multi = new Set(); hl = { cables: new Set(), nodes: new Set() }; armed = []; selAnn = null;
    try {
      const site = (await api.get<{ name: string }>('/site')).name;
      await withLightTheme(async () => {
        const world = svgEl.querySelector('g.world') as SVGGElement;
        let vb: [number, number, number, number];
        if (exp!.frame === 'all') { const b = world.getBBox(); vb = [b.x - 30, b.y - 30, b.width + 60, b.height + 60]; }
        else { const r = svgEl.getBoundingClientRect(); vb = [-tx / k, -ty / k, r.width / k, r.height / k]; }
        const svg = standalone(svgEl, vb, (el) => el.classList.contains('gridbg') || el.classList.contains('hit') || el.classList.contains('preview') || el.classList.contains('rs') || el.classList.contains('off'));
        svg.querySelector('g')?.removeAttribute('transform');
        for (const g of svg.querySelectorAll('g')) if (g.getAttribute('transform')?.startsWith('translate(') && g.getAttribute('transform')?.includes('scale(')) g.removeAttribute('transform');
        const viewName = syn!.view === 'overview' ? 'Vue générale' : syn!.views.containers.find((c) => syn!.view === `container:${c.id}`)?.name ?? syn!.views.locations.find((l) => syn!.view === `location:${l.id}`)?.name ?? '';
        const day = new Date().toISOString().slice(0, 10);
        const stem = `Synoptique_${project.project.name}_${viewName}`.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^A-Za-z0-9]+/g, '_') + `_${day}`;
        if (exp!.format === 'pdf') save(await toPdf(svg, vb[2], vb[3], exp!.paper, exp!.landscape, { title: exp!.title, project: project.project.name, client: project.project.client ?? 'Projet interne', view: viewName, revision: exp!.revision, site, date: new Date().toLocaleDateString('fr-FR') }), `${stem}.pdf`);
        else save(await raster(svg, vb[2], vb[3], exp!.format === 'png' ? 'image/png' : 'image/jpeg'), `${stem}.${exp!.format}`);
      });
      toast('Synoptique exporté.');
      exp = null;
    } catch (e) { fail(e); if (exp) exp.busy = false; }
    finally { ({ sel, multi, hl, armed, selAnn } = keep); }
  }
  // Aperçu du câblage : un chemin de référence orthogonal (sortie du port, points de passage,
  // puis horizontal-vertical-horizontal jusqu'au pointeur), et pour les autres câbles du même
  // tracé décalé en parallèle de l'écart de leurs ports : un faisceau qui ne converge pas sur
  // le pointeur et ne se croise pas. La pose suit exactement la même géométrie.
  /** Sortie horizontale d'un port, au-delà de l'étiquette du câble (connecteur et numéro). */
  const outOf = (ref?: string) => Math.ceil((textWidth(short(portByRef(ref ?? '')?.connector ?? ''), MONO_S) + 28 + TAG_GAP * 2 + 12) / 10) * 10;
  const stubOf = (P: [number, number, 'l' | 'r'], ref?: string): [number, number] => [P[0] + (P[2] === 'r' ? outOf(ref) : -outOf(ref)), P[1]];
  /** Chemin de référence depuis un port, par les points de passage, jusqu'à `end`. */
  function refPath(P: [number, number, 'l' | 'r'], way: [number, number][], end: [number, number], ref?: string): [number, number][] {
    let pts: [number, number][] = [[P[0], P[1]], stubOf(P, ref)];
    for (const w of way) pts = orthoTo(pts, w);
    return simplify(orthoTo(pts, end));
  }
  /** Faisceau : chemins parallèles de chaque port armé (triés de haut en bas) vers `end`. */
  function bundle(srcs: Port[], way: [number, number][], end: [number, number], spread = true): { ref: string; pts: [number, number][] }[] {
    const ps = srcs.map((a) => ({ a, P: portXY(a.ref) })).filter((x): x is { a: Port; P: [number, number, 'l' | 'r'] } => !!x.P).sort((u, v) => u.P[1] - v.P[1]);
    if (!ps.length) return [];
    const P0 = ps[0]!.P, ref = refPath(P0, way, end, ps[0]!.a.ref);
    // Écart signé : le premier tronçon part à droite (décalage vers le bas) ou à gauche.
    const sgn = P0[2] === 'r' ? 1 : -1;
    return ps.map(({ a, P }) => {
      const dy = P[1] - P0[1];
      if (!dy) return { ref: a.ref, pts: ref };
      if (!spread && way.length) { let pts: [number, number][] = [[P[0], P[1]], stubOf(P, a.ref)]; for (const w of way) pts = orthoTo(pts, w); return { ref: a.ref, pts: simplify(orthoTo(pts, [end[0], end[1] + dy])) }; }
      const off = offsetPath(ref, dy * sgn);
      off[0] = [P[0], P[1]];
      return { ref: a.ref, pts: simplify(off) };
    });
  }
  const linePath = (pts: [number, number][]) => pts.map((q, i) => `${i ? 'L' : 'M'}${q[0]} ${q[1]}`).join('');
  const armedPreview = $derived.by(() => {
    if (!armed.length || !mouse) return '';
    return bundle(armed, cab.mode === 'multipoint' ? wps : [], mouse, cab.parallel === 'spread' || cab.mode !== 'multipoint').map((b) => linePath(b.pts)).join(' ');
  });
  const endName = (ref: string, info: Far | null) => { const n = nodeOf(ref); const p = n?.ports.find((x) => x.ref === ref); return n && p ? `${n.label} › ${p.label}` : info ? `${info.label} › ${info.port}` : 'Extrémité supprimée'; };
</script>

<svelte:window onkeydown={key} onkeyup={(e) => { if (e.key === ' ') spacePan = false; }} onpointerdown={(e) => { if (ctx && !(e.target as Element).closest('.ctx')) ctx = null; }} />

{#if !syn}<span class="spin"></span>
{:else}
  <div class="syn">
    <div class="dtool">
      <ViewPicker views={syn.views} value={syn.view} onpick={(v) => navigate(viewUrl(v))} />
      {#if !syn.overview}<span class="muted small">{nodes.length} nœud{nodes.length > 1 ? 's' : ''} · {syn.cables.length} câble{syn.cables.length > 1 ? 's' : ''}</span>{:else}<span class="muted small">{syn.overview.locations.length} lieu{syn.overview.locations.length > 1 ? 'x' : ''} · {nf(syn.overview.totals.weight_kg, 1)} kg · {nf(syn.overview.totals.power_w)} W · double-clic sur un lieu pour l’ouvrir</span>{/if}
      <span style="flex:1"></span>
      {#if mine}
        {#if requested}<span class="lockchip req"><Icon name="lock" size="sm" />{requested} demande la main · <button onclick={() => { requested = null; release(); }}>Céder</button></span>{/if}
      {:else if syn.lock}
        <span class="lockchip other" title="Vous voyez la dernière version enregistrée"><Icon name="lock" size="sm" />{syn.lock.account_name} a la main{#if canDesign} · <button onclick={requestHand}>Demander</button>{/if}</span>
      {:else if canDesign}
        <button class="btn sm primary" onclick={take}><Icon name="unlock" size="sm" />Prendre la main</button>
      {:else}<span class="lockchip other"><Icon name="lock" size="sm" />Lecture seule</span>{/if}
      <button class="btn sm valid" class:ok={issues && !issues.counts.error && !issues.counts.warning} class:bad={issues && (issues.counts.error || issues.counts.warning)} class:on={tab === 'issues'} title="Validation du câblage" onclick={() => { tab = tab === 'issues' ? 'cable' : 'issues'; if (tab === 'issues') validate(); }}><Icon name={issues && (issues.counts.error || issues.counts.warning) ? 'alert' : 'check'} size="sm" />Validation{#if issues} <span class="mono">{issues.counts.error + issues.counts.warning}</span>{/if}</button>
      <button class="btn sm" onclick={() => (exp = { format: 'pdf', paper: 'A3', landscape: true, frame: 'all', revision: '', title: 'Synoptique', busy: false })}><Icon name="download" size="sm" />Exporter</button>
      <details class="more" bind:this={moreEl}><summary class="btn sm" aria-label="Plus d’actions">⋯</summary><div class="more-m" role="menu" tabindex="-1" onclick={() => moreEl?.removeAttribute('open')} onkeydown={() => {}}>
        {#if !syn.overview}
          <label class="mi"><input type="checkbox" bind:checked={showEmpty} onchange={() => { if (syn) pos = { ...autoLayout(syn.nodes.filter((n) => showEmpty || n.ports.length)), ...syn.layout }; }}>Afficher les nœuds sans port{hidden ? ` (${hidden})` : ''}</label>
          <label class="mi"><input type="checkbox" checked={rulers} onchange={() => { rulers = !rulers; lsSet('rulers', rulers); }}>Règles et repères</label>
          <label class="mi"><input type="checkbox" checked={showMap} onchange={() => { showMap = !showMap; lsSet('map', showMap); }}>Mini-carte</label>
        {/if}
        <button onclick={() => (help = true)}>Raccourcis clavier</button>
        {#if mine && !syn.overview}
          <hr />
          <button onclick={() => (unnumber = true)}>Supprimer la numérotation de la vue</button>
          <button class="danger" onclick={() => (unplug = true)}>Décâbler la vue…</button>
        {/if}
      </div></details>
      {#if mine}<button class="btn sm primary" title="Numérotation (N)" onclick={() => (numbering = true)}><Icon name="hash" size="sm" />Numéroter</button>{/if}
    </div>
    <div class="syn-body" class:norail={!!syn.overview}>
      {#if !syn.overview}
        <nav class="rail" aria-label="Outils">
          {#each TOOLS.filter((x) => !x.edit || mine) as t, i (t.id)}
            {#if i > 0 && t.edit && !TOOLS.filter((x) => !x.edit || mine)[i - 1]!.edit}<span class="rail-sep"></span>{/if}
            <button class:on={tool === t.id} title="{t.label} ({t.key})" aria-label={t.label} onclick={() => setTool(t.id)}><Icon name={t.icon} size="sm" /></button>
            {#if t.id === 'cable' && tool === 'cable' && mine && !syn.overview}
            <!-- Options du câble : sous-outils dans la barre d'outils, juste sous
                 l'outil Câble (rien ne recouvre le schéma) ; le détail de chaque option est dans l'infobulle. -->
            <div class="cabfly" role="toolbar" aria-orientation="vertical" aria-label="Options du câble">
              {#each [
                [[['count', 'simple', 'Un câble à la fois (1)', 'M5 12h14', [[5, 12], [19, 12]]], ['count', 'multiple', 'Multiple : premier port, dernier port de la plage, puis premier port d’arrivée (2)', 'M5 7h14M5 12h14M5 17h14', []]]],
                [[['mode', 'auto', 'Tracé automatique (Maj+A)', 'M4 7h7v10h9M17.5 3.5l.8 1.7 1.7.8-1.7.8-.8 1.7-.8-1.7-1.7-.8 1.7-.8z', []], ['mode', 'multipoint', 'Multipoint : points de passage posés sur le fond (M)', 'M4 18h5V7h7v10h4', [[9, 18], [9, 7], [16, 7], [16, 17]]]]],
                [[['style', 'orthogonal', 'Orthogonal (Q)', 'M4 6h8v12h8', []], ['style', 'rounded', 'Arrondi (W)', 'M4 6h4.5a3.5 3.5 0 0 1 3.5 3.5v5a3.5 3.5 0 0 0 3.5 3.5H20', []], ['style', 'arrow', 'Flèche : renvoi à chaque extrémité', 'M3 12h7M7 8.5l3.5 3.5L7 15.5M21 12h-7M17 8.5 13.5 12l3.5 3.5', []]]],
                [[['parallel', 'stacked', 'Multiple : même chemin (superposé)', 'M4 8h8v8h8', []], ['parallel', 'spread', 'Multiple : faisceau parallèle (écarté)', 'M4 5h10v10h6M4 10h5v10h11', []]]],
              ] as [opts]}
                <div class="cf-g">
                  {#each opts as [key, val, title, d, dots]}
                    <button class:on={cab[key as 'count'] === val} title={title as string} aria-label={title as string} aria-pressed={cab[key as 'count'] === val}
                      onclick={() => { if (key === 'style') setStyle(val as CabPrefs['style']); else { (cab as any)[key as string] = val; if (key === 'mode' && val === 'auto') wps = []; } }}>
                      <svg viewBox="0 0 24 24" aria-hidden="true"><path d={d as string} class:thick={val === 'stacked'} />{#each dots as [cx, cy]}<circle {cx} {cy} r="1.8" />{/each}</svg>
                    </button>
                  {/each}
                </div>
              {/each}
              <div class="cf-g"><button class:on={cab.askModel} title="Proposer un modèle de câble après chaque pose" aria-label="Proposer un modèle de câble" aria-pressed={cab.askModel} onclick={() => (cab.askModel = !cab.askModel)}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5h8l8 7-8 7H4zM8 12h.01" /></svg></button></div>
            </div>
            {/if}
          {/each}
          {#if mine}<span class="rail-sep"></span>
            <button title="Annuler (Ctrl+Z)" aria-label="Annuler" disabled={!past.length} onclick={undo}><Icon name="undo" size="sm" /></button>
            <button class="flip" title="Rétablir (Ctrl+Maj+Z)" aria-label="Rétablir" disabled={!future.length} onclick={redo}><Icon name="undo" size="sm" /></button>
          {/if}
          <span style="flex:1"></span>
          <button title="Raccourcis clavier (?)" aria-label="Raccourcis clavier" onclick={() => (help = true)}><Icon name="keys" size="sm" /></button>
        </nav>
      {/if}
      <div class="syn-stage" bind:this={stageEl} bind:clientWidth={stageW} bind:clientHeight={stageH}><svg bind:this={svgEl} class="syn-canvas t-{panning() ? 'pan' : tool}" class:arming={armed.length > 0} role="application" aria-label="Synoptique"
        onwheel={wheel} onpointerdown={down} onpointermove={move} onpointerup={up} oncontextmenu={contextMenu}
        ondblclick={(e) => { if (!(e.target as Element).closest('[data-node],[data-cable],[data-ann],.port')) fit(); }}>
        <defs><pattern id="syn-grid" width="20" height="20" patternUnits="userSpaceOnUse" patternTransform="translate({tx} {ty}) scale({k})"><circle cx="1" cy="1" r="1" fill="var(--grid-dot, rgba(120,130,140,.25))" /></pattern></defs>
        <rect class="gridbg" width="100%" height="100%" fill="url(#syn-grid)" />
        <g class="world" transform="translate({tx} {ty}) scale({k})">
          {#each anns.filter((a) => a.kind === 'zone') as a (a.id)}
            {@const d = a.data}
            <g class="zone" class:sel={selAnn === a.id} data-ann={a.id} style="--z:{d.color}">
              <rect class="zbody" x={d.x} y={d.y} width={d.w} height={d.h} rx={d.radius} style="fill-opacity:{d.opacity};stroke-width:{d.width}" stroke-dasharray={d.stroke === 'dashed' ? '10 6' : d.stroke === 'dotted' ? '2 5' : undefined} />
              <text class="zname" x={d.x + 6} y={d.y - 8}>{d.name || 'Zone sans nom'}</text>
              {#if selAnn === a.id && mine}<rect class="rs" x={d.x + d.w - 9} y={d.y + d.h - 9} width="14" height="14" rx="3" />{/if}
            </g>
          {/each}
          {#if draft}<rect class="draft" x={draft.x} y={draft.y} width={draft.w} height={draft.h} rx="12" />{/if}
          {#each anns.filter((a) => annMulti.has(a.id)) as a (a.id)}<rect class="amulti" x={a.data.x - 6} y={a.data.y - (a.kind === 'zone' ? 6 : 18)} width={(a.data.w ?? 120) + 12} height={(a.data.h ?? 24) + 12} rx="10" />{/each}
          {#each rulers ? guides.v : [] as gx, i}<line class="syn-guide" x1={gx} x2={gx} y1={-1e5} y2={1e5} role="separator" tabindex="-1" onpointerdown={(e) => guideStart(e, 'v', i)} ondblclick={() => { guides = { ...guides, v: guides.v.filter((_, j) => j !== i) }; saveGuides(); }} />{/each}
          {#each rulers ? guides.h : [] as gy, i}<line class="syn-guide" x1={-1e5} x2={1e5} y1={gy} y2={gy} role="separator" tabindex="-1" onpointerdown={(e) => guideStart(e, 'h', i)} ondblclick={() => { guides = { ...guides, h: guides.h.filter((_, j) => j !== i) }; saveGuides(); }} />{/each}
          {#if gdrag}{#if gdrag.axis === 'v'}<line class="syn-guide live" x1={gdrag.at} x2={gdrag.at} y1={-1e5} y2={1e5} />{:else}<line class="syn-guide live" x1={-1e5} x2={1e5} y1={gdrag.at} y2={gdrag.at} />{/if}{/if}
          {#if lasso && lasso.length > 1}<polygon class="lasso" points={lasso.map((p) => p.join(',')).join(' ')} />{/if}
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
                  <g class="cable" data-cable={c.id} class:renvoi={!!g.renvoi} class:off={hiddenFam.has(famOf(c))} class:attach={c.fields.attach} class:harnessed={!!harnessOf(c)} class:sel={multi.has(c.id) || sel === c.id} class:rsel={rsel.has(c.id)} class:hl={hl.cables.has(c.id)} class:warn={c.warnings.length > 0} style="--c:{cableColor(c)};--h:{harnessOf(c)?.color ?? 'transparent'};--rs:{rsel.get(c.id) ?? 'transparent'}" role="button" tabindex="-1" onclick={(e) => pickCable(c.id, e)} onkeydown={() => {}}>
                    <title>{c.fields.number ?? 'Sans numéro'} · {endName(c.a, c.a_info)} → {endName(c.b, c.b_info)}</title>
                    <path class="case" d={g.d} /><path class="vis" d={g.d} /><path class="hit" d={g.d} />{#if g.ah}<path class="ahead" d={g.ah} />{/if}
                  </g>
                {/if}
              {:else}
                {@const s = stub(c)}
                {#if s}
                  <g class="cable stub" data-cable={c.id} class:off={hiddenFam.has(famOf(c))} class:sel={multi.has(c.id) || sel === c.id} class:hl={hl.cables.has(c.id)} class:warn={c.warnings.length > 0} style="--c:{cableColor(c)}" role="button" tabindex="-1" onclick={(e) => pickCable(c.id, e)} onkeydown={() => {}}>
                    <path class="case" d={s.d} /><path class="vis" d={s.d} /><path class="hit" d={s.d} />
                    <g class="far" transform="translate({s.dx > 0 ? s.ex + 4 : s.ex - 4} {s.ey - 11})"><rect x={s.dx > 0 ? 0 : -190} width="190" height="22" rx="6" /><text x={s.dx > 0 ? 8 : -182} y="15">{s.far?.external ? '↗' : '→'} {(s.far ? (s.far.external ? s.far.label : `${s.far.label} › ${s.far.port}`) : 'Extrémité supprimée').slice(0, 30)}</text></g>
                  </g>
                {/if}
              {/if}
            {/each}
          </g>
          <g class="nodes">
            {#each nodes as n (n.key)}
              {@const at = pos[n.key] ?? { x: 0, y: 0 }}
              {@const h = nodeH(n)}
              <g class="node {n.kind}" class:hl={hl.nodes.has(n.key)} class:nsel={selNodes.has(n.key)} data-node={n.key} transform="translate({at.x} {at.y})" style="--c:{n.function ? fnColor(n.function) : n.kind === 'panel' ? '#5B636D' : '#8A919B'}">
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
                    {@const s = portChip(p)}
                    {@const mk = out ? NW - 19 : 12}
                    {@const tx = out ? NW - 24 : 24}
                    <g class="port" class:used={used.has(p.ref)} class:armed={armedRefs.has(p.ref)} class:ok={okTargets?.has(p.ref)} class:bad={okTargets && !okTargets.has(p.ref) && !armedRefs.has(p.ref)} class:tgt={tgtPreview.has(p.ref)} class:snap={snap?.p.ref === p.ref} style="--c:{portColor(p)}" role="button" tabindex="-1"
                      onpointerdown={(e) => portDown(p, n, e)} onkeydown={(e) => { if (e.key === 'Enter') clickPort(p, n, e as unknown as MouseEvent); }}
                      onpointerenter={(e) => { const r = stageEl.getBoundingClientRect(); hoverPort = { p, n, sx: e.clientX - r.left, sy: e.clientY - r.top }; }} onpointerleave={() => { if (hoverPort?.p.ref === p.ref) hoverPort = null; }}>
                      <title>{p.label} · {refById('connectors', p.connector)?.name ?? p.connector}{p.variant ? ` ${p.variant}` : ''}{p.gender ? ` · ${p.gender === 'M' ? 'mâle' : p.gender === 'F' ? 'femelle' : 'neutre'}` : ''}{p.signal ? ` · ${refById('signals', p.signal)?.name ?? p.signal}` : ''}</title>
                      <circle class="ph" cx={x} cy={y} r="13" />
                      <circle class="pdot" cx={x} cy={y} r="4.5" />
                      {#if p.dir === 'in' || p.dir === 'out'}<path class="pmk" d="M{mk} {y - 4.5}l7 4.5-7 4.5z" />
                      {:else}<path class="pmk" d="M{mk + 3.5} {y - 4.5}l-3.5 4.5 3.5 4.5z" /><path class="pmk2" d="M{mk + 3.5} {y - 4.5}l3.5 4.5-3.5 4.5z" />{/if}
                      <text class="pn" x={tx} y={y - 0.5} text-anchor={out ? 'end' : 'start'}>{out ? fittedLabels(n)[i]?.r : fittedLabels(n)[i]?.l}</text>
                      <text class="pc" x={tx} y={y + 10} text-anchor={out ? 'end' : 'start'}>{s}</text>
                    </g>
                  {/each}
                {/each}
                {#if !n.ports.length}<text class="ns" x={NW / 2} y={HH + 26} text-anchor="middle">Aucun port dans la fiche</text>{/if}
              </g>
            {/each}
          </g>
          <!-- Vol d'oiseau : trait direct entre les deux extrémités des câbles sélectionnés. -->
          <g class="bird" pointer-events="none">
            {#each [...new Set([...(sel ? [sel] : []), ...multi])].slice(0, 60) as id (id)}
              {@const c = syn.cables.find((x) => x.id === id)}
              {#if c}
                {@const A = portXY(c.a)}{@const B = portXY(c.b)}{@const st = !A || !B ? stub(c) : null}
                {@const p1 = A ?? (st ? [st.ex, st.ey] : null)}{@const p2 = B ?? (st ? [st.ex, st.ey] : null)}
                {#if p1 && p2 && (p1[0] !== p2[0] || p1[1] !== p2[1])}
                  <line class="bird-line" x1={p1[0]} y1={p1[1]} x2={p2[0]} y2={p2[1]} style="--c:{cableColor(c)}" />
                  <circle class="bird-end" cx={p1[0]} cy={p1[1]} r="3" />
                  <circle class="bird-end" cx={p2[0]} cy={p2[1]} r="3" />
                {/if}
              {/if}
            {/each}
          </g>
          <g class="tags">
            {#each syn.cables.filter((x) => !x.fields.attach) as c (c.id)}
              {@const both = c.a_in_view && c.b_in_view}
              {@const g = both ? cPath(c) : null}
              {@const s = both ? null : stub(c)}
              {@const cw = textWidth(connOf(c), MONO_S)}
              {#if g?.renvoi}{#each g.renvoi as r}{@const fw = Math.min(220, textWidth(r.far, '600 11px Inter') + 18)}<g class="far renvoi-l" class:off={hiddenFam.has(famOf(c))} style="--c:{cableColor(c)}" data-cable={c.id} role="button" tabindex="-1" onclick={(e) => pickCable(c.id, e)} onkeydown={() => {}} transform="translate({r.dx > 0 ? r.ex + 4 : r.ex - 4} {r.ey - 11})"><rect x={r.dx > 0 ? 0 : -fw} width={fw} height="22" rx="6" /><text x={r.dx > 0 ? 9 : -fw + 9} y="15">{fitText(r.far, 202, '600 11px Inter')}</text></g>{/each}{/if}
              {#each (g ? [g.t1, g.t2] : s ? [s.t] : []) as t}
                <g class="lbl" data-cable={c.id} class:off={hiddenFam.has(famOf(c))} class:none={!c.fields.number} class:dup={!!c.fields.number && dups.has(c.fields.number)} class:sel={multi.has(c.id) || sel === c.id} class:hl={hl.cables.has(c.id)} style="--c:{cableColor(c)}" transform="translate({t[0]} {t[1] - 9})" role="button" tabindex="-1" onclick={(e) => pickCable(c.id, e)} onkeydown={() => {}}>
                  <rect class="tag" width={tagW(c)} height="18" rx="4" />
                  <rect class="bandc" x="3.5" y="3.5" width="3" height="11" rx="1" />
                  <text class="tconn" x="10" y="12.5">{connOf(c)}</text>
                  {#if c.fields.number}<text x={cw + 16} y="13">{c.fields.number}</text>{:else}<text class="tnone" x={cw + 16} y="13">—</text>{/if}
                  {#if (c.fields.number && dups.has(c.fields.number)) || c.warnings.length}<g class="dupm" transform="translate({tagW(c) + 4} 0)"><path d="M8 1l7.5 14h-15z" /><text x="8" y="13.5" text-anchor="middle">!</text><title>{[...(c.fields.number && dups.has(c.fields.number) ? ['Numéro en double'] : []), ...c.warnings].join(' · ')}</title></g>{/if}
                </g>
              {/each}
            {/each}
          </g>
          {#each anns.filter((a) => a.kind === 'label') as a (a.id)}
            {@const d = a.data}
            {@const fam = d.font === 'mono' ? 'var(--mono)' : d.font === 'serif' ? 'Georgia, serif' : 'var(--font)'}
            <g class="alabel" class:sel={selAnn === a.id} data-ann={a.id}>
              {#if d.pill}<rect x={d.x - 8} y={d.y - d.size} width={d.text.length * d.size * 0.58 + 16} height={d.size * 1.45} rx={d.size * 0.72} fill={d.color === '#15171A' ? 'var(--fg)' : d.color} opacity="0.14" />{/if}
              <text x={d.x} y={d.y} fill={d.color === '#15171A' ? 'var(--fg)' : d.color} style="font-family:{fam};font-size:{d.size}px;font-weight:{d.bold ? 700 : 400};font-style:{d.italic ? 'italic' : 'normal'};text-decoration:{d.underline ? 'underline' : 'none'}">{d.text}</text>
            </g>
          {/each}
          {#if armedPreview}<path class="preview" d={armedPreview} />{/if}
          {#if mine && selected && (tool === 'select' || tool === 'cable') && !armed.length}
            <!-- Poignées de tronçon : sur chaque câble sélectionné (auto ou multipoint) ; en
                 sélection multiple, saisir un tronçon déplace celui de tous les câbles. -->
            {#each (multi.size > 1 ? syn.cables.filter((c) => multi.has(c.id)).slice(0, 80) : [selected]) as hc (hc.id)}
              {@const gp = hc.a_in_view && hc.b_in_view && !hc.fields.attach && hc.fields.style !== 'arrow' && !hiddenFam.has(famOf(hc)) ? cablePts(hc) : null}
              {#if gp}
                {#each gp as q1, j}{@const q2 = gp[j + 1]}
                  {#if q2 && j >= 1 && j <= gp.length - 3 && (q1[0] === q2[0] || q1[1] === q2[1]) && Math.hypot(q2[0] - q1[0], q2[1] - q1[1]) > 24}
                    <rect class="seg-h" class:v={q1[0] === q2[0]} data-handle="seg:{hc.id}:{j}" x={(q1[0] + q2[0]) / 2 - (q1[0] === q2[0] ? 4 : 11)} y={(q1[1] + q2[1]) / 2 - (q1[0] === q2[0] ? 11 : 4)} width={q1[0] === q2[0] ? 8 : 22} height={q1[0] === q2[0] ? 22 : 8} rx="4"><title>{multi.size > 1 ? 'Glisser pour déplacer ce tronçon sur tous les câbles sélectionnés' : 'Glisser pour déplacer ce tronçon'}</title></rect>
                  {/if}
                {/each}
              {/if}
            {/each}
            {#if multi.size <= 1 && !selected.fields.attach}
              {#each [['a', selected.a], ['b', selected.b]] as [end, ref]}{@const P = portXY(ref)}{#if P && !selected.fields.external}<circle class="end-h" data-handle="end:{end}" cx={P[0]} cy={P[1]} r="7"><title>Glisser vers un autre port pour rebrancher</title></circle>{/if}{/each}
            {/if}
          {/if}
          {#if rewire && selected}{@const O = portXY(rewire.end === 'a' ? selected.b : selected.a)}{#if O}<path class="preview" d={linePath(refPath(O, [], rewire.at, rewire.end === 'a' ? selected.b : selected.a))} />{/if}{/if}
          {#each wps as w}<circle class="wp" cx={w[0]} cy={w[1]} r="4" />{/each}
        </g>
      </svg>
      {#if !syn.overview && rulers}
        <svg class="ruler rt" aria-hidden="true" onpointerdown={(e) => guideStart(e, 'h', null)}><rect width="100%" height="20" class="rbg" />{#each ticks(-tx / k, (stageW - tx) / k) as v}<line x1={tx + v * k} x2={tx + v * k} y1="12" y2="20" class="rtk" /><text x={tx + v * k + 3} y="11" class="rlb">{v}</text>{/each}</svg>
        <svg class="ruler rl" aria-hidden="true" onpointerdown={(e) => guideStart(e, 'v', null)}><rect width="20" height="100%" class="rbg" />{#each ticks(-ty / k, (stageH - ty) / k) as v}<line y1={ty + v * k} y2={ty + v * k} x1="12" x2="20" class="rtk" /><text transform="translate(11 {ty + v * k - 3}) rotate(-90)" class="rlb">{v}</text>{/each}</svg>
      {/if}
      {#if mm && showMap}
        <div class="minimap" style="width:{mm.w}px;height:{mm.h}px" role="slider" tabindex="-1" aria-label="Mini-carte" aria-valuenow={0} onpointerdown={mmGo}>
          <svg width={mm.w} height={mm.h}>
            {#each anns.filter((a) => a.kind === 'zone') as a (a.id)}<rect x={(a.data.x - mm.x0) * mm.sc} y={(a.data.y - mm.y0) * mm.sc} width={a.data.w * mm.sc} height={a.data.h * mm.sc} fill={a.data.color ?? '#4F6BFF'} opacity=".18" />{/each}
            {#each nodes as n (n.key)}{@const p = pos[n.key]}{#if p}<rect x={(p.x - mm.x0) * mm.sc} y={(p.y - mm.y0) * mm.sc} width={Math.max(2, NW * mm.sc)} height={Math.max(2, nodeH(n) * mm.sc)} class="mmn" />{/if}{/each}
            <rect class="mmv" x={(-tx / k - mm.x0) * mm.sc} y={(-ty / k - mm.y0) * mm.sc} width={(stageW / k) * mm.sc} height={(stageH / k) * mm.sc} />
          </svg>
        </div>
      {/if}
      {#if syn.view}<RemoteCursors room={`project:${pid}/syn:${syn.view}`} toScreen={(x, y) => [tx + x * k, ty + y * k]} bind:selections={remoteSel} />{/if}
      {#if mine && !syn.overview && tool === 'cable'}
        <div class="cap-hint">{#if !armed.length}{cab.count === 'multiple' ? 'Cliquez le premier port de la plage, puis le dernier' : 'Cliquez ou glissez d’un port vers un autre'}{:else if cab.mode === 'multipoint'}Cliquez le fond pour poser un point ({wps.length}) · port d’arrivée pour finir · Échap pour annuler{:else}{armed.length} port{armed.length > 1 ? 's' : ''} : cliquez le {armed.length > 1 ? 'premier ' : ''}port d’arrivée · Échap pour annuler{/if}</div>
      {/if}
      <div class="zoomer" role="group" aria-label="Zoom">
        <button title="Zoom arrière" aria-label="Zoom arrière" onclick={() => (k = Math.max(0.2, k / 1.2))}><Icon name="zout" size="sm" /></button>
        <span class="mono">{Math.round(k * 100)} %</span>
        <button title="Zoom avant" aria-label="Zoom avant" onclick={() => (k = Math.min(2.5, k * 1.2))}><Icon name="zin" size="sm" /></button>
        <button title="Tout afficher (F)" aria-label="Tout afficher" onclick={fit}><Icon name="fit" size="sm" /></button>
      </div>
      {#if armed.length && hoverPort && !armedRefs.has(hoverPort.p.ref)}
        {@const why = incompat(armed[0]!, hoverPort.p)}
        {#if why.length}<div class="incompat" style="left:{Math.max(8, Math.min(hoverPort.sx + 14, stageW - 300))}px;top:{hoverPort.sy + 18}px"><b><Icon name="alert" size="sm" />Connexion non recommandée</b>{#each why as w}<span>{w}</span>{/each}</div>{/if}
      {/if}
      {#if ctx}
        <div class="ctx" style="left:{Math.min(ctx.x, stageW - 250)}px;top:{Math.min(ctx.y, stageH - 40 - ctx.items.length * 34)}px" role="menu">
          {#if ctx.title}<div class="ctx-t">{ctx.title}</div>{/if}
          {#each ctx.items as it}{#if it.sep}<hr />{/if}<button role="menuitem" class:danger={it.danger} disabled={it.disabled} onclick={() => { const f = it.act; ctx = null; f?.(); }}>{it.label}</button>{/each}
        </div>
      {/if}</div>

      <aside class="syn-side">
        <div class="stabs" role="tablist">
          {#each [['cable', 'Propriétés'], ['layers', 'Calques'], ['harness', 'Faisceaux']] as [t2, l]}<button role="tab" aria-selected={tab === t2} class:on={tab === t2} onclick={() => (tab = t2 as typeof tab)}>{l}</button>{/each}
          {#if tab === 'issues'}<button role="tab" aria-selected="true" class="on">Validation</button>{/if}
        </div>
        <div class="sbody">
        {#if tab === 'layers'}
          <section>
            {#if families.length}
              <div class="seg" role="group" aria-label="Calques par">{#each [['family', 'Par famille'], ['signal', 'Par signal']] as [kk, l]}<button class:on={layerBy === kk} onclick={() => { layerBy = kk as typeof layerBy; lsSet('layerby', kk); }}>{l}</button>{/each}</div>
              {#each families as f (f)}
                {@const n = syn.cables.filter((c) => famOf(c) === f).length}
                <div class="hrow lay" class:locked={lockedFam.has(f)}>
                  <label class="toggle-row small" style="flex:1;min-width:0"><input type="checkbox" checked={!hiddenFam.has(f)} onchange={() => toggleFam(f)} aria-label="Afficher {layerName(f)}"><i class="dot" style="--c:{layerColor(f)}"></i><span class="ln">{layerName(f)}</span><span class="muted mono">{n}</span></label>
                  <button class="icon-btn sm" title={lockedFam.has(f) ? 'Déverrouiller' : 'Verrouiller'} aria-label={lockedFam.has(f) ? 'Déverrouiller' : 'Verrouiller'} onclick={() => toggleLock(f)}><Icon name={lockedFam.has(f) ? 'lock' : 'unlock'} size="sm" /></button>
                  <button class="icon-btn sm" title="Renommer" aria-label="Renommer {layerName(f)}" onclick={() => renameLayer(f)}><Icon name="pen" size="sm" /></button>
                  <button class="btn sm ghost" onclick={() => isolateFam(f)} title="Isoler">Isoler</button>
                </div>
              {/each}
              {#if hiddenFam.size}<button class="btn sm ghost" onclick={() => { hiddenFam = new Set(); try { localStorage.removeItem('syn-hidden'); } catch { /* */ } }}>Tout afficher</button>{/if}
            {:else}<p class="muted small">Aucun câble dans cette vue.</p>{/if}
          </section>
        {:else if tab === 'harness'}
          <section>
            {#each syn.harnesses as h (h.id)}
              {@const n = syn.cables.filter((c) => c.fields.harness_id === h.id).length}
              <div class="hrow"><button class="hname" onclick={() => showHarness(h)}><i class="dot" style="--c:{h.color}"></i><b>{h.name}</b><span class="muted small">{n} câble{n > 1 ? 's' : ''} dans la vue</span></button>{#if mine}<button class="icon-btn sm" aria-label="Supprimer le faisceau {h.name}" onclick={() => dropHarness(h)}><Icon name="trash" size="sm" /></button>{/if}</div>
            {:else}<span class="muted small">Aucun faisceau. Choisissez des câbles, puis clic droit › « Créer un faisceau ».</span>{/each}
            {#if mine}<button class="btn sm ghost" style="justify-self:start" onclick={() => (newHarness = { name: '', color: '#2459E0' })}><Icon name="plus" size="sm" />Faisceau</button>{/if}
          </section>
        {:else if annSel && tab !== 'issues'}
          {@const a = annSel}
          {@const d = a.data}
          <section>
            <span class="lbl-t">{a.kind === 'zone' ? 'Zone' : 'Étiquette'}</span>
            {#if a.kind === 'zone'}
              <div class="field"><label for="an">Nom</label><input class="input" id="an" value={d.name} maxlength="120" disabled={!mine} onchange={(e) => patchAnn(a, { name: (e.target as HTMLInputElement).value })}></div>
              <div class="sh2">
                <div class="field"><label for="ac">Couleur</label><input class="input" id="ac" type="color" style="padding:4px;height:40px" value={d.color} disabled={!mine} onchange={(e) => patchAnn(a, { color: (e.target as HTMLInputElement).value })}></div>
                <div class="field"><label for="ar">Rayon</label><input class="input mono" id="ar" type="number" min="0" max="200" value={d.radius} disabled={!mine} onchange={(e) => patchAnn(a, { radius: Number((e.target as HTMLInputElement).value) })}></div>
              </div>
              <div class="field"><span class="flabel">Trait</span><div class="seg">{#each [['solid', 'Plein'], ['dashed', 'Tirets'], ['dotted', 'Pointillés']] as [v, l]}<button class:on={d.stroke === v} disabled={!mine} onclick={() => patchAnn(a, { stroke: v })}>{l}</button>{/each}</div></div>
              <div class="sh2">
                <div class="field"><label for="aw">Épaisseur</label><input class="input mono" id="aw" type="number" min="0.5" max="12" step="0.5" value={d.width} disabled={!mine} onchange={(e) => patchAnn(a, { width: Number((e.target as HTMLInputElement).value) })}></div>
                <div class="field"><label for="ao">Opacité du fond</label><input id="ao" type="range" min="0" max="0.6" step="0.02" value={d.opacity} disabled={!mine} onchange={(e) => patchAnn(a, { opacity: Number((e.target as HTMLInputElement).value) })}></div>
              </div>
            {:else}
              <div class="field"><label for="at">Texte</label><textarea class="input" id="at" rows="2" maxlength="500" value={d.text} disabled={!mine} onchange={(e) => patchAnn(a, { text: (e.target as HTMLTextAreaElement).value })}></textarea></div>
              <div class="sh2">
                <div class="field"><label for="af">Police</label><select class="input" id="af" value={d.font} disabled={!mine} onchange={(e) => patchAnn(a, { font: (e.target as HTMLSelectElement).value })}><option value="sans">Sans</option><option value="serif">Serif</option><option value="mono">Mono</option></select></div>
                <div class="field"><label for="as">Taille</label><input class="input mono" id="as" type="number" min="8" max="96" value={d.size} disabled={!mine} onchange={(e) => patchAnn(a, { size: Number((e.target as HTMLInputElement).value) })}></div>
              </div>
              <div class="seg wrap">{#each [['bold', 'Gras'], ['italic', 'Italique'], ['underline', 'Souligné'], ['pill', 'Pastille']] as [k, l]}<button class:on={d[k]} disabled={!mine} onclick={() => patchAnn(a, { [k]: !d[k] })}>{l}</button>{/each}</div>
              <div class="field"><label for="alc">Couleur</label><input class="input" id="alc" type="color" style="padding:4px;height:40px" value={d.color} disabled={!mine} onchange={(e) => patchAnn(a, { color: (e.target as HTMLInputElement).value })}></div>
            {/if}
            {#if mine}<button class="btn sm ghost danger" onclick={() => deleteAnn(a)}><Icon name="trash" size="sm" />Supprimer</button>{/if}
          </section>
        {:else if tab === 'issues'}
          <section>
            <span class="lbl-t">Validation</span>
            {#if !issues}<span class="spin"></span>
            {:else}
              <div class="counts"><span class="cnt err"><b>{issues.counts.error}</b> erreur{issues.counts.error > 1 ? 's' : ''}</span><span class="cnt warn"><b>{issues.counts.warning}</b> avertissement{issues.counts.warning > 1 ? 's' : ''}</span><span class="cnt info"><b>{issues.counts.info}</b> info{issues.counts.info > 1 ? 's' : ''}</span></div>
              {#if issues.issues.length}<AnomalyGroups compact list={issueList} onpick={(a) => a.src && goIssue(a.src)} />
              {:else}<p class="muted">Aucune anomalie : le câblage est cohérent.</p>{/if}
              <button class="btn sm ghost" onclick={validate}><Icon name="refresh" size="sm" />Relancer</button>
            {/if}
          </section>
        {:else if selNodes.size > 1 && !sel}
          {@const cs = [...selNodes].flatMap((k2) => cablesOfNode(k2))}
          <section>
            <span class="lbl-t">{selNodes.size} nœuds sélectionnés</span>
            <p class="muted small" style="margin:0">Glissez l’un d’eux pour déplacer tout le groupe (Maj : axe verrouillé). Les points de passage des câbles entre ces nœuds suivent.</p>
            <button class="btn sm" disabled={!cs.length} onclick={() => { multi = new Set(cs.map((c) => c.id)); sel = cs[0]?.id ?? null; selNodes = new Set(); }}>Sélectionner leurs câbles ({new Set(cs.map((c) => c.id)).size})</button>
          </section>
        {:else if nodeSel && !sel}
          {@const n = nodeSel}
          {@const cs = cablesOfNode(n.key)}
          <section>
            <span class="lbl-t">{n.kind === 'panel' ? 'Panel' : n.kind === 'adapter' ? 'Adaptateur' : 'Équipement'}</span>
            <h3 class="nd-t">{n.label}</h3>
            <span class="muted small">{n.sub}</span>
            <dl class="kv">
              <dt>Entrées / gauche</dt><dd>{left(n).length}</dd><dt>Sorties / droite</dt><dd>{right(n).length}</dd>
              <dt>Ports câblés</dt><dd>{n.ports.filter((p) => used.has(p.ref)).length} / {n.ports.length}</dd>
              {#if n.function}<dt>Fonction</dt><dd>{refById('functions', n.function)?.name ?? n.function}</dd>{/if}
              <dt>Position</dt><dd class="mono">{Math.round(pos[n.key]?.x ?? 0)}, {Math.round(pos[n.key]?.y ?? 0)}</dd>
            </dl>
            <button class="btn sm" disabled={!cs.length} onclick={() => { multi = new Set(cs.map((c) => c.id)); sel = cs[0]?.id ?? null; selNodes = new Set(); }}>Sélectionner ses câbles ({cs.length})</button>
            {#if mine}<button class="btn sm primary" disabled={!cs.length} onclick={() => { multi = new Set(cs.map((c) => c.id)); sel = cs[0]?.id ?? null; numbering = true; }}><Icon name="hash" size="sm" />Numéroter ses câbles</button>{/if}
          </section>
        {:else if multi.size > 1}
          <section>
            <span class="lbl-t">{multi.size} câbles choisis</span>
            <p class="muted small" style="margin:0">Ctrl ou Maj + clic pour en ajouter ou en retirer.</p>
            {#if mine}
              <div class="field"><label for="mh">Faisceau</label><select class="input" id="mh" onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v === '+') newHarness = { name: '', color: '#2459E0' }; else assign(v || null); (e.target as HTMLSelectElement).value = '~'; }}><option value="~">Ranger dans…</option><option value="">Aucun faisceau</option>{#each syn.harnesses as h}<option value={h.id}>{h.name}</option>{/each}<option value="+">+ Nouveau faisceau…</option></select></div>
              <div class="field"><span class="flabel">Style du tracé</span><div class="seg">{#each [['orthogonal', 'Orthogonal'], ['rounded', 'Arrondi'], ['arrow', 'Flèche']] as [k2, l]}<button onclick={() => setStyle(k2 as CabPrefs['style'])}>{l}</button>{/each}</div></div>
              <button class="btn sm" onclick={() => askModel([...multi])}>Modèle de câble…</button>
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
            {#if c.fields.external && !c.fields.external.location_id}<ProjectLinks {pid} cable={c.id} />{/if}
            <div class="sh2">
              <div class="field"><label for="cc">Couleur</label><input class="input" id="cc" type="color" style="padding:4px;height:40px" value={c.fields.color ?? cableColor(c)} disabled={!mine} onchange={(e) => patchSel({ color: (e.target as HTMLInputElement).value })}></div>
              <div class="field"><label for="cl">Longueur réelle (m)</label><input class="input mono" id="cl" type="number" min="0" step="0.5" value={c.fields.length_m ?? ''} disabled={!mine} placeholder={c.estimate_m ? `≈ ${nf(c.estimate_m, 1)}` : 'À saisir'} onchange={(e) => { const v = (e.target as HTMLInputElement).value; patchSel({ length_m: v === '' ? null : Number(v) }); }}></div>
            </div>
            <div class="field"><label for="ch">Faisceau</label><select class="input" id="ch" disabled={!mine} value={c.fields.harness_id ?? ''} onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v === '+') { newHarness = { name: '', color: '#2459E0' }; } else assign(v || null); }}><option value="">Aucun</option>{#each syn.harnesses as h}<option value={h.id}>{h.name}</option>{/each}{#if mine}<option value="+">+ Nouveau faisceau…</option>{/if}</select></div>
            {#if !c.fields.external}
              <div class="field"><span class="flabel">Style du tracé</span><div class="seg">{#each [['orthogonal', 'Orthogonal'], ['rounded', 'Arrondi'], ['arrow', 'Flèche']] as [k2, l]}<button class:on={c.fields.style === k2} disabled={!mine} onclick={() => patchSel({ style: k2 })}>{l}</button>{/each}</div></div>
              <div class="field"><label for="cm">Modèle de câble</label>
                <select class="input" id="cm" disabled={!mine} value={c.fields.cable_model_id ?? ''} onfocus={loadModels} onchange={(e) => patchSel({ cable_model_id: (e.target as HTMLSelectElement).value || null })}>
                  <option value="">Non défini</option>{#if c.fields.cable_model_id && !models?.some((m) => m.id === c.fields.cable_model_id)}<option value={c.fields.cable_model_id}>Modèle choisi</option>{/if}
                  {#each models ?? [] as m (m.id)}<option value={m.id}>{m.name}{m.reference ? ` · ${m.reference}` : ''}</option>{/each}
                </select></div>
              {#if mine && (c.fields.waypoints?.length || c.fields.number)}<div style="display:flex;gap:6px;flex-wrap:wrap">
                {#if c.fields.waypoints?.length}<button class="btn sm ghost" onclick={() => patchSel({ waypoints: [] })}>Réinitialiser le tracé</button>{/if}
                {#if c.fields.number}<button class="btn sm ghost" onclick={() => patchSel({ number: null })}>Effacer le numéro</button>{/if}
              </div>{/if}
              {#if mine}<p class="muted small" style="margin:0">Glissez une extrémité vers un autre port pour rebrancher ; glissez une poignée de tronçon pour modifier le tracé.</p>{/if}
            {/if}
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
            <span class="lbl-t">Rien de sélectionné</span>
            <p class="muted small" style="margin:0">{mine ? 'Outil Câble (C) : cliquez ou glissez d’un port à l’autre ; les ports compatibles s’allument. Outil Sélection (V) : cliquez un nœud ou un câble, glissez le fond pour sélectionner au rectangle. Clic droit : actions. ? : raccourcis.' : 'Cliquez un câble ou un port câblé pour voir ses détails.'}</p>
            {#if warnCount}
              <span class="lbl-t" style="margin-top:8px">À vérifier</span>
              <AnomalyGroups compact list={toCheck} onpick={(a) => { if (a.src) { sel = a.src; multi = new Set([a.src]); centerOn(`[data-cable="${a.src}"]`); } }} />
            {/if}
          </section>
        {/if}
        </div>
      </aside>
    </div>
  </div>
{/if}

{#if unplug && syn}
  <Confirm title="Décâbler la vue ?" confirmLabel="Décâbler" danger onconfirm={unplugView} onclose={() => (unplug = false)}>
    <p>Les {syn.cables.length} câbles, attaches et ports externes de cette vue sont supprimés. C’est irréversible : l’historique d’annulation est vidé.</p>
  </Confirm>
{/if}
{#if unnumber}
  <Confirm title="Supprimer la numérotation de la vue ?" confirmLabel="Supprimer les numéros" danger onconfirm={unnumberView} onclose={() => (unnumber = false)}>
    <p>Les câbles de la vue perdent leur numéro. Les compteurs ne reculent pas : les numéros ne seront pas réattribués.</p>
  </Confirm>
{/if}
{#if exp && syn}
  <Dialog title="Exporter le synoptique" eyebrow={project.project.name} onclose={() => (exp = null)}>
    <div class="stack">
      <div class="field"><span class="flabel">Format</span><div class="seg">{#each [['pdf', 'PDF'], ['png', 'PNG'], ['jpg', 'JPG']] as [k, l]}<button class:on={exp.format === k} onclick={() => (exp!.format = k as 'pdf')}>{l}</button>{/each}</div></div>
      <div class="field"><span class="flabel">Cadrage</span><div class="seg"><button class:on={exp.frame === 'all'} onclick={() => (exp!.frame = 'all')}>Tout le dessin</button><button class:on={exp.frame === 'view'} onclick={() => (exp!.frame = 'view')}>Vue affichée</button></div></div>
      {#if exp.format === 'pdf'}
        <div class="sh2">
          <div class="field"><label for="ep">Papier</label><select class="input" id="ep" bind:value={exp.paper}>{#each Object.keys(PAPERS).filter((p) => p.startsWith('A')) as p}<option value={p}>{p}</option>{/each}</select></div>
          <div class="field"><span class="flabel">Orientation</span><div class="seg"><button class:on={exp.landscape} onclick={() => (exp!.landscape = true)}>Paysage</button><button class:on={!exp.landscape} onclick={() => (exp!.landscape = false)}>Portrait</button></div></div>
        </div>
        <div class="sh2">
          <div class="field"><label for="et">Titre</label><input class="input" id="et" bind:value={exp.title} maxlength="80"></div>
          <div class="field"><label for="er">Révision</label><input class="input mono" id="er" bind:value={exp.revision} maxlength="20" placeholder="A"></div>
        </div>
      {/if}
      <p class="muted small" style="margin:0">Rendu sur fond blanc avec la palette claire, même en thème sombre ; câbles et textes vectoriels dans le PDF.</p>
    </div>
    {#snippet footer()}<button class="btn ghost" onclick={() => (exp = null)}>Annuler</button><button class="btn primary" disabled={exp!.busy} onclick={doExport}>{#if exp!.busy}<span class="spin"></span>{/if}Exporter</button>{/snippet}
  </Dialog>
{/if}
{#if newHarness}
  <Confirm title="Nouveau faisceau" confirmLabel="Créer" onconfirm={createHarness} onclose={() => (newHarness = null)}>
    <div class="sh2"><div class="field"><label for="hn">Nom</label><input class="input" id="hn" bind:value={newHarness.name} maxlength="120" placeholder="Multi vidéo plateau"></div><div class="field"><label for="hc">Couleur</label><input class="input" id="hc" type="color" style="padding:4px;height:40px" bind:value={newHarness.color}></div></div>
    <p class="muted small">{multi.size || sel ? 'Les câbles choisis y seront rangés.' : 'Rangez-y des câbles ensuite.'}</p>
  </Confirm>
{/if}
{#if modelPick}
  {@const mp = modelPick}
  <Dialog title="Modèle de câble" eyebrow="{mp.ids.length} câble{mp.ids.length > 1 ? 's' : ''} posé{mp.ids.length > 1 ? 's' : ''}" onclose={() => (modelPick = null)}>
    <div class="mlist">
      {#each [...(models ?? [])].sort((a, b) => ['ok', 'unknown', 'bad'].indexOf(modelFit(a, mp.family)) - ['ok', 'unknown', 'bad'].indexOf(modelFit(b, mp.family)) || a.name.localeCompare(b.name, 'fr')) as m (m.id)}
        {@const f = modelFit(m, mp.family)}
        <button class="mrow {f}" onclick={() => setModel(mp.ids, m.id)}><b>{m.name}</b><span class="muted small">{m.reference ?? ''}</span><span class="mfit">{f === 'ok' ? 'Correspond' : f === 'bad' ? 'Autre fonction' : 'À vérifier'}</span></button>
      {:else}<p class="muted">Aucun modèle de câble au catalogue.</p>{/each}
    </div>
    {#snippet footer()}<label class="toggle-row small" style="margin-right:auto"><input type="checkbox" checked={!cab.askModel} onchange={(e) => (cab.askModel = !(e.target as HTMLInputElement).checked)}>Ne plus proposer</label><button class="btn ghost" onclick={() => (modelPick = null)}>Passer</button>{/snippet}
  </Dialog>
{/if}
{#if help}
  <Dialog title="Raccourcis du synoptique" onclose={() => (help = false)}>
    <div class="keys">
      {#each [
        ['Outils', [['V', 'Sélection'], ['H', 'Main (ou Espace maintenu)'], ['L', 'Lasso'], ['C', 'Câble'], ['T', 'Texte'], ['R', 'Zone'], ['E', 'Gomme']]],
        ['Câble', [['1 / 2', 'Simple / multiple'], ['Maj+A / M', 'Auto / multipoint'], ['Q / W', 'Orthogonal / arrondi'], ['Retour arrière', 'Enlever le dernier point'], ['Échap', 'Annuler le tracé']]],
        ['Édition', [['Suppr', 'Supprimer la sélection'], ['Ctrl+Z / Ctrl+Maj+Z', 'Annuler / rétablir'], ['Ctrl+A', 'Tout sélectionner'], ['Ctrl+C / X / V', 'Copier, couper, coller zones et textes'], ['N', 'Numérotation'], ['Clic droit', 'Menu d’actions']]],
        ['Vue', [['Molette', 'Zoom vers le curseur'], ['F ou double-clic sur le fond', 'Tout afficher'], ['Alt + glisser', 'Faire défiler'], ['Glisser depuis une règle', 'Poser un repère']]],
      ] as [g, list]}
        <section><h4>{g}</h4>{#each list as [kk, l]}<div class="krow"><kbd>{kk}</kbd><span>{l}</span></div>{/each}</section>
      {/each}
    </div>
  </Dialog>
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
  .dtool { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; min-height: 54px; padding: 8px 12px; }
  .syn-stage { position: relative; min-width: 0; min-height: 0; overflow: hidden; }
  .ruler { position: absolute; z-index: 4; cursor: copy; }
  .ruler.rt { left: 0; top: 0; width: 100%; height: 20px; cursor: row-resize; }
  .ruler.rl { left: 0; top: 0; width: 20px; height: 100%; cursor: col-resize; }
  .rbg { fill: color-mix(in srgb, var(--surface) 90%, transparent); }
  .rtk { stroke: var(--fg-3); stroke-width: 1; }
  .rlb { font: 600 9px var(--font-mono, monospace); fill: var(--fg-3); }
  :global(.syn-guide) { stroke: #E23C7A; stroke-width: 1; vector-effect: non-scaling-stroke; stroke-dasharray: 6 4; cursor: move; }
  :global(.syn-guide.live) { stroke-dasharray: none; }
  :global(.lasso) { fill: color-mix(in srgb, var(--accent, #4F6BFF) 10%, transparent); stroke: var(--accent, #4F6BFF); stroke-width: 1.5; vector-effect: non-scaling-stroke; stroke-dasharray: 5 4; }
  :global(.amulti) { fill: none; stroke: var(--accent, #4F6BFF); stroke-width: 2; vector-effect: non-scaling-stroke; stroke-dasharray: 4 3; }
  .minimap { position: absolute; right: 12px; bottom: 56px; z-index: 4; background: color-mix(in srgb, var(--surface) 70%, transparent); border-radius: 8px; cursor: pointer; overflow: hidden; }
  .minimap svg { display: block; }
  .mmn { fill: var(--fg-3); }
  .mmv { fill: color-mix(in srgb, var(--accent, #4F6BFF) 12%, transparent); stroke: var(--accent, #4F6BFF); stroke-width: 1.5; }
  .lay.locked .ln { opacity: .55; }
  .lay .ln { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* .syn-canvas sert aussi ailleurs (app.css) : on neutralise ici sa marge intérieure, qui
     décalait le canevas par rapport au calcul des coordonnées de la souris. */
  .syn-stage > .syn-canvas { height: 100%; padding: 0; margin: 0; border-radius: 0; box-shadow: none; display: block; background-image: none; }
  :global(.cable.rsel) path { filter: drop-shadow(0 0 2.5px var(--rs)) drop-shadow(0 0 2.5px var(--rs)); }
  .syn { margin: -6px -8px 0; }
  .syn-body { display: grid; grid-template-columns: 52px minmax(0, 1fr) 320px; border-top: 1px solid var(--line); height: calc(100vh - 176px); min-height: 520px; }
  .syn-body.norail { grid-template-columns: minmax(0, 1fr) 320px; }
  /* Rail d'outils (hors du dessin) */
  .rail { display: flex; flex-direction: column; align-items: center; gap: 3px; padding: 10px 0; border-right: 1px solid var(--line); background: var(--surface); }
  .rail button { width: 36px; height: 36px; display: grid; place-items: center; border: 0; border-radius: 9px; background: none; color: var(--fg-2); cursor: pointer; }
  .rail button:hover:not(:disabled) { background: var(--surface-2); color: var(--fg); }
  .rail button.on { background: var(--accent); color: #fff; }
  .rail button:disabled { opacity: .3; cursor: default; }
  .rail-sep { width: 24px; height: 1px; margin: 5px 0; background: var(--line); }
  /* Options de l'outil Câble : capsule flottante, sans décaler le dessin */
  .cabfly { display: flex; flex-direction: column; align-items: center; gap: 4px; margin: 2px 0 4px; padding: 4px 0; width: 40px; border-radius: 10px; background: var(--surface-2); box-shadow: inset 0 0 0 1px var(--line); }
  .cf-g { display: flex; flex-direction: column; gap: 2px; }
  .cf-g + .cf-g { padding-top: 4px; border-top: 1px solid var(--line); width: 28px; align-items: center; }
  .rail .cabfly button { width: 32px; height: 28px; display: grid; place-items: center; border: 0; border-radius: 8px; background: none; color: var(--fg-2); cursor: pointer; }
  .rail .cabfly button:hover { background: var(--surface-2); color: var(--fg); }
  .rail .cabfly button.on { background: color-mix(in srgb, var(--accent) 20%, transparent); color: var(--fg); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 60%, transparent); }
  .cabfly svg { width: 19px; height: 19px; fill: none; stroke: currentColor; stroke-width: 1.7; stroke-linecap: round; stroke-linejoin: round; }
  .cabfly svg circle { fill: currentColor; stroke: none; }
  .cabfly svg .thick { stroke-width: 3.2; }
  .cap-hint { position: absolute; left: 50%; bottom: 16px; transform: translateX(-50%); z-index: 5; padding: 3px 10px; border-radius: 99px; font-size: 12px; color: var(--accent); background: color-mix(in srgb, var(--surface) 88%, transparent); white-space: nowrap; pointer-events: none; }
  /* Zoom et mini-carte en bas à droite */
  .zoomer { position: absolute; right: 12px; bottom: 12px; z-index: 5; display: flex; align-items: center; gap: 2px; padding: 2px; border-radius: 9px; background: color-mix(in srgb, var(--surface) 80%, transparent); }
  .zoomer button { width: 30px; height: 30px; display: grid; place-items: center; border: 0; border-radius: 8px; background: none; color: var(--fg-2); cursor: pointer; }
  .zoomer button:hover { background: var(--surface-2); color: var(--fg); }
  .zoomer .mono { min-width: 46px; text-align: center; font-size: 12px; }
  /* Barre unique */
  .lockchip { display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 12px; border-radius: 99px; font-size: 13px; font-weight: 600; white-space: nowrap; }
  .lockchip.mine { background: color-mix(in srgb, var(--ok, #1E8E4F) 13%, transparent); color: var(--ok, #1E8E4F); }
  .lockchip.other { background: var(--surface-2); color: var(--fg-2); }
  .lockchip.req { background: var(--warn-soft); color: var(--warn); }
  .lockchip button { border: 0; background: none; padding: 0; font: inherit; color: inherit; text-decoration: underline; cursor: pointer; }
  .valid.ok { color: var(--ok, #1E8E4F); }
  .valid.bad { color: var(--warn, #B78400); }
  .more-m .mi { display: flex; align-items: center; gap: 8px; padding: 8px 10px; border-radius: 8px; font-size: 14px; cursor: pointer; }
  .more-m .mi:hover { background: var(--surface-2); }
  .more-m hr { border: 0; border-top: 1px solid var(--line); margin: 4px 2px; }
  /* Panneau à onglets */
  .stabs { display: flex; gap: 2px; padding: 8px; border-bottom: 1px solid var(--line); position: sticky; top: 0; background: var(--surface); z-index: 1; }
  .stabs button { flex: 1; padding: 7px 4px; border: 0; border-radius: 8px; background: none; font: inherit; font-size: 13.5px; font-weight: 600; color: var(--fg-3); cursor: pointer; }
  .stabs button.on { background: var(--surface-2); color: var(--fg); }
  .sbody { padding: 16px; }
  .syn-canvas { width: 100%; height: 100%; display: block; background: var(--surface-2); touch-action: none; cursor: grab; user-select: none; }
  .syn-canvas:active { cursor: grabbing; }
  .syn-canvas.arming { cursor: crosshair; }
  .syn-side { border-left: 1px solid var(--line); padding: 0; overflow: auto; background: var(--surface); }
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
  /* Vol d'oiseau : trait fin orange, sans halo. */
  .bird-line { stroke: #FF8A1F; stroke-width: 1.4; stroke-dasharray: 6 5; stroke-linecap: round; vector-effect: non-scaling-stroke; animation: bird 0.9s linear infinite; }
  .bird-end { fill: #FF8A1F; stroke: none; }
  @keyframes bird { to { stroke-dashoffset: -22; } }
  @media (prefers-reduced-motion: reduce) { .bird-line { animation: none; } }
  .pdot { fill: var(--node-bg, var(--surface)); stroke: var(--c); stroke-width: 2; }
  .port.used .pdot { fill: var(--c); }
  .port:hover .ph, .port.armed .ph { fill: color-mix(in srgb, var(--c) 22%, transparent); }
  .port.armed .ph { stroke: var(--c); stroke-width: 2; }
  .port :global(.cg-shell) { fill: var(--node-bg, var(--surface)); stroke: var(--c); stroke-width: 2.2; }
  .port.used :global(.cg-shell) { fill: color-mix(in srgb, var(--c) 30%, var(--node-bg, var(--surface))); }
  .port :global(.cg-ring) { fill: none; stroke: var(--c); stroke-width: 1.2; opacity: .75; }
  .port :global(.cg-pin), .port :global(.cg-key) { fill: var(--c); }
  .cchip rect { fill: color-mix(in srgb, var(--fg) 7%, transparent); stroke: color-mix(in srgb, var(--fg) 14%, transparent); stroke-width: .8; }
  .cchip text { font: 600 9.5px var(--mono); fill: var(--fg-3); }
  .pn { font: 600 12px var(--font); fill: var(--fg); }
  .cb-l { font: 600 10px var(--mono); letter-spacing: .09em; text-transform: uppercase; color: var(--fg-3); margin-left: 6px; }
  .port:focus, .port:focus-visible { outline: none; }
  .wp { fill: var(--surface); stroke: var(--accent); stroke-width: 2; }
  .cable .ahead { fill: var(--c); stroke: none; pointer-events: none; }
  .pc { font: 500 8.5px var(--mono); letter-spacing: .03em; fill: var(--fg-3); }
  .pmk { fill: var(--c); }
  .pmk2 { fill: var(--fg-2); }
  /* Câbles */
  .cable { cursor: pointer; }
  .cable .case { fill: none; stroke: var(--surface-2); stroke-width: 6; }
  .cable .vis { fill: none; stroke: var(--c); stroke-width: 2.2; stroke-linejoin: round; }
  .cable .hit { fill: none; stroke: transparent; stroke-width: 14; }
  .cable.sel .vis { stroke-width: 3.2; }
  .cable.stub .vis { stroke-dasharray: 2 4; }
  .far rect { fill: var(--surface); stroke: var(--c); stroke-width: 1.2; }
  .far text { font: 600 11px var(--font); fill: var(--fg-2); }
  .lbl { cursor: pointer; }
  .lbl .tag { fill: #FFFFFF; stroke: color-mix(in srgb, var(--c) 70%, #000); stroke-width: 1; }
  .lbl .bandc { fill: var(--c); }
  .lbl text { font: 700 11px var(--mono); fill: #15171A; }
  .lbl.none .tag { fill: color-mix(in srgb, #B78400 18%, #fff); stroke-dasharray: 3 2; }
  .lbl.sel .tag { stroke-width: 1.4; }
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
  .zone .zbody { fill: var(--z); stroke: var(--z); cursor: move; }
  .zone.sel .zbody { filter: drop-shadow(0 0 4px var(--z)); }
  .zone .zname { font: 700 15px var(--font); fill: var(--z); pointer-events: none; }
  .zone .rs { fill: var(--surface); stroke: var(--z); stroke-width: 2; cursor: nwse-resize; }
  .draft { fill: color-mix(in srgb, var(--accent, #4F6BFF) 8%, transparent); stroke: var(--accent, #4F6BFF); stroke-width: 2; stroke-dasharray: 8 5; }
  .alabel { cursor: move; }
  .alabel.sel text { paint-order: stroke; stroke: color-mix(in srgb, var(--accent, #4F6BFF) 40%, transparent); stroke-width: 4px; }
  .cable.off, .lbl.off { display: none; }
  .flip :global(svg) { transform: scaleX(-1); }
  .more { position: relative; }
  .more summary { list-style: none; cursor: pointer; }
  .more summary::-webkit-details-marker { display: none; }
  .more-m { position: absolute; right: 0; top: calc(100% + 6px); z-index: 20; min-width: 260px; display: grid; padding: 6px; border-radius: 12px; background: var(--surface); box-shadow: 0 12px 30px -10px rgba(0, 0, 0, .35), 0 0 0 1px var(--line); }
  .more-m button { text-align: left; padding: 9px 10px; border: 0; border-radius: 8px; background: none; font: inherit; color: inherit; cursor: pointer; }
  .more-m button:hover { background: var(--surface-2); }
  .more-m button.danger { color: var(--err); }
  .seg.wrap { flex-wrap: wrap; }
  .syn-canvas.t-select { cursor: default; } .syn-canvas.t-pan { cursor: grab; } .syn-canvas.t-pan:active { cursor: grabbing; }
  .syn-canvas.t-cable, .syn-canvas.t-zone, .syn-canvas.t-lasso { cursor: crosshair; } .syn-canvas.t-label { cursor: text; }
  .syn-canvas.t-eraser { cursor: not-allowed; } .syn-canvas.t-eraser .cable:hover .vis { stroke: var(--err); stroke-dasharray: 4 3; }
  .node.nsel .box { stroke: var(--accent); stroke-width: 1.6; }
  .nd-t { margin: 0; font-size: 17px; }
  /* Compatibilité pendant le tracé */
  .arming .port.ok .pdot { r: 6.5; }
  .arming .port.ok .ph { fill: color-mix(in srgb, var(--c) 16%, transparent); stroke: color-mix(in srgb, var(--c) 55%, transparent); stroke-width: 1.5; }
  .arming .port.bad { opacity: .45; }
  .port.tgt .ph, .port.snap .ph { fill: color-mix(in srgb, var(--accent) 26%, transparent); stroke: var(--accent); stroke-width: 2; }
  .incompat { position: absolute; z-index: 6; pointer-events: none; display: grid; gap: 2px; width: max-content; max-width: 290px; padding: 8px 10px; border-radius: 10px; background: var(--surface); box-shadow: 0 10px 24px -10px rgba(0, 0, 0, .4), 0 0 0 1px color-mix(in srgb, var(--warn, #B78400) 50%, var(--line)); font-size: 12.5px; }
  .incompat b { display: flex; align-items: center; gap: 6px; color: var(--warn, #B78400); }
  /* Poignées du câble sélectionné */
  .end-h { fill: var(--surface); stroke: var(--accent); stroke-width: 2.5; cursor: grab; }
  .seg-h { fill: var(--accent); stroke: var(--surface); stroke-width: 1.5; cursor: ns-resize; }
  .seg-h.v { cursor: ew-resize; }
  .cable.renvoi .vis { stroke-width: 2.6; }
  .renvoi-l { cursor: pointer; } .renvoi-l.off { display: none; }
  /* Étiquettes d'extrémité */
  .lbl .tconn { font: 600 9.5px var(--mono); fill: #6A717A; }
  .lbl .tnone { fill: #9AA0A8; font-style: italic; font-weight: 500; }
  .lbl.none .tag { fill: #FFFFFF; stroke-dasharray: none; }
  .lbl.dup .tag { stroke: #E07A00; stroke-width: 1.8; }
  .dupm path { fill: #F29100; } .dupm text { font: 800 10px var(--font); fill: #fff; }
  /* Menu contextuel */
  .ctx { position: absolute; z-index: 30; min-width: 230px; display: grid; padding: 6px; border-radius: 12px; background: var(--surface); box-shadow: 0 14px 34px -10px rgba(0, 0, 0, .4), 0 0 0 1px var(--line); }
  .ctx-t { padding: 6px 10px 4px; font: 600 11px var(--mono); letter-spacing: .06em; text-transform: uppercase; color: var(--fg-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ctx button { text-align: left; padding: 8px 10px; border: 0; border-radius: 8px; background: none; font: inherit; font-size: 14px; color: inherit; cursor: pointer; }
  .ctx button:hover:not(:disabled) { background: var(--surface-2); }
  .ctx button:disabled { opacity: .4; cursor: default; }
  .ctx button.danger { color: var(--err); }
  .ctx hr { border: 0; border-top: 1px solid var(--line); margin: 4px 2px; }
  /* Modèle de câble et raccourcis */
  .mlist { display: grid; gap: 6px; max-height: 50vh; overflow: auto; }
  .mrow { display: grid; grid-template-columns: minmax(0, 1fr) auto; grid-template-rows: auto auto; column-gap: 10px; padding: 10px 12px; border: 0; border-radius: 10px; background: var(--surface-2); text-align: left; font: inherit; color: inherit; cursor: pointer; }
  .mrow:hover { box-shadow: inset 0 0 0 1.5px var(--accent); }
  .mrow .mfit { grid-row: 1 / 3; grid-column: 2; align-self: center; font-size: 12px; font-weight: 600; padding: 3px 8px; border-radius: 99px; background: var(--surface); }
  .mrow.ok .mfit { color: var(--ok, #1E8E4F); } .mrow.bad { opacity: .65; } .mrow.bad .mfit { color: var(--err); }
  .keys { display: grid; grid-template-columns: 1fr 1fr; gap: 18px; }
  .keys h4 { margin: 0 0 6px; font-size: 13px; text-transform: uppercase; letter-spacing: .06em; color: var(--fg-3); }
  .krow { display: grid; grid-template-columns: 140px 1fr; gap: 8px; padding: 3px 0; font-size: 13.5px; align-items: baseline; }
  kbd { font: 600 11.5px var(--mono); padding: 2px 6px; border-radius: 5px; background: var(--surface-2); box-shadow: inset 0 -1px 0 var(--line); justify-self: start; }
  @media (max-width: 700px) { .keys { grid-template-columns: 1fr; } }
  .preview { fill: none; stroke: var(--accent, #4F6BFF); stroke-width: 2; stroke-dasharray: 6 4; pointer-events: none; }
  @media (max-width: 1000px) { .syn-body, .syn-body.norail { grid-template-columns: 1fr; height: auto; } .rail { flex-direction: row; flex-wrap: wrap; border-right: 0; border-bottom: 1px solid var(--line); padding: 6px; } .rail-sep { width: 1px; height: 24px; margin: 0 4px; } .syn-canvas, .syn-stage { height: 70vh; } .syn-side { border-left: 0; border-top: 1px solid var(--line); } }
</style>
