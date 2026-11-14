// Rendus SVG autonomes pour le dossier (section 11.6) : pages de dessin, étages de carte et
// synoptique par élément, à partir des données de l'API, en palette claire et polices
// standard (rendu PDF fidèle). Les images sont passées en data URL avant l'appel.

const esc = (s: unknown) => String(s ?? '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
import { FONT, SVG_MONO, SVG_SANS, familyDash } from './pdfkit';

const F = SVG_SANS;
/** `families` : familles de signal des câbles dessinés (légende du PDF). */
export interface Rendered { svg: string; w: number; h: number; families?: string[] }

export function toSvgEl(r: Rendered): SVGSVGElement {
  const el = new DOMParser().parseFromString(r.svg, 'image/svg+xml').documentElement as unknown as SVGSVGElement;
  return document.importNode(el, true);
}

export async function dataUrl(url: string): Promise<string | null> {
  try { let b = await (await fetch(url, { credentials: 'same-origin' })).blob(); if (b.type.startsWith('image/svg')) b = new Blob([await b.text()], { type: 'image/svg+xml;charset=utf-8' }); return await new Promise((ok) => { const r = new FileReader(); r.onload = () => ok(r.result as string); r.onerror = () => ok(null); r.readAsDataURL(b); }); } catch { return null; }
}

// ----- Dessins (11.2) -----

const SIZES: Record<string, [number, number]> = { A4: [210, 297], A3: [297, 420], A2: [420, 594], A1: [594, 841], A0: [841, 1189] };
type El = { id: string; type: string; [k: string]: any };

export function drawingSvg(page: { paper: string; landscape: boolean; background: string | null; legend: boolean; scale: number | null; nature: string; dmx_display: string; elements: El[] }, opts: { images: Record<string, string>; fnColor: (f: string | null) => string; sigColor: (id: string) => string; sigName: (id: string) => string }): Rendered {
  const s = SIZES[page.paper] ?? [297, 420];
  const [w, h] = page.landscape ? [s[1], s[0]] : [s[0], s[1]];
  const by = new Map(page.elements.map((e) => [e.id, e]));
  const center = (e: El): [number, number] => (e.type === 'line' ? [e.points[0][0], e.points[0][1]] : e.type === 'text' ? [e.x, e.y] : [e.x + e.w / 2, e.y + e.h / 2]);
  const at = (a: any): [number, number] => { const e = a.element ? by.get(a.element) : null; return e ? center(e) : [a.x, a.y]; };
  const edge = (a: any, t: [number, number]): [number, number] => {
    const c = at(a), e = a.element ? by.get(a.element) : null;
    if (!e || !('w' in e) || e.type === 'text' || e.type === 'line') return c;
    const dx = t[0] - c[0], dy = t[1] - c[1]; if (!dx && !dy) return c;
    const k = Math.min(dx ? e.w / 2 / Math.abs(dx) : Infinity, dy ? e.h / 2 / Math.abs(dy) : Infinity);
    return k >= 1 ? c : [c[0] + dx * k, c[1] + dy * k];
  };
  const arrow = (x1: number, y1: number, x2: number, y2: number, wd: number, col: string) => { const a = Math.atan2(y2 - y1, x2 - x1), z = 2.2 + wd * 2; const p = (d: number) => `${x2 - z * Math.cos(a + d)},${y2 - z * Math.sin(a + d)}`; return `<polygon points="${x2},${y2} ${p(0.4)} ${p(-0.4)}" fill="${col}"/>`; };
  const dmxText = (e: El) => { const d = e.dmx; if (!d || !d.address) return 'DMX —'; if (page.dmx_display === 'channel') return `Ch ${(d.universe - 1) * 512 + d.address}`; if (page.dmx_display === 'number') return `#${d.console}`; return `U${d.universe}.${String(d.address).padStart(3, '0')}`; };
  const out: string[] = [];
  out.push(`<rect width="${w}" height="${h}" fill="${esc(page.background ?? '#ffffff')}"/>`);
  for (const e of page.elements.filter((x) => x.type === 'zone')) {
    const c = esc(e.color ?? '#3A7BD5');
    out.push(`<rect x="${e.x}" y="${e.y}" width="${e.w}" height="${e.h}" rx="2" fill="${c}" fill-opacity="0.1" stroke="${c}" stroke-width="0.4" stroke-dasharray="2 1.2"/><text x="${e.x + 2}" y="${e.y + 4.5}" font-family="${F}" font-size="3.4" font-weight="bold" fill="${c}">${esc(e.label)}</text>`);
  }
  for (const e of page.elements.filter((x) => x.type !== 'zone' && x.type !== 'link')) {
    if (e.type === 'text') out.push(`<text x="${e.x}" y="${e.y}" font-family="${F}" font-size="${e.size}" font-weight="${e.bold ? 'bold' : 'normal'}" fill="${esc(e.color ?? '#15171A')}">${String(e.text).split('\n').map((l, i) => `<tspan x="${e.x}" dy="${i ? e.size * 1.25 : 0}">${esc(l)}</tspan>`).join('')}</text>`);
    else if (e.type === 'rect') out.push(`<rect x="${e.x}" y="${e.y}" width="${e.w}" height="${e.h}" rx="${e.radius ?? 0}" fill="${esc(e.fill ?? 'none')}" stroke="${esc(e.stroke ?? '#15171A')}" stroke-width="${e.width}"/>`);
    else if (e.type === 'ellipse') out.push(`<ellipse cx="${e.x + e.w / 2}" cy="${e.y + e.h / 2}" rx="${e.w / 2}" ry="${e.h / 2}" fill="${esc(e.fill ?? 'none')}" stroke="${esc(e.stroke ?? '#15171A')}" stroke-width="${e.width}"/>`);
    else if (e.type === 'line') {
      const st = esc(e.stroke ?? '#15171A'), pts = e.points as number[][], n = pts.length;
      out.push(`<polyline points="${pts.map((p) => p.join(',')).join(' ')}" fill="none" stroke="${st}" stroke-width="${e.width}"${e.dash ? ' stroke-dasharray="2 1.2"' : ''}/>`);
      if (e.arrow_end) out.push(arrow(pts[n - 2]![0]!, pts[n - 2]![1]!, pts[n - 1]![0]!, pts[n - 1]![1]!, e.width, st));
      if (e.arrow_start) out.push(arrow(pts[1]![0]!, pts[1]![1]!, pts[0]![0]!, pts[0]![1]!, e.width, st));
    } else if (e.type === 'image') { const href = opts.images[e.file]; if (href) out.push(`<image href="${href}" x="${e.x}" y="${e.y}" width="${e.w}" height="${e.h}" preserveAspectRatio="xMidYMid meet"/>`); }
    else if (e.type === 'equipment') {
      const s2 = e.snapshot, fc = esc(e.frame ?? (s2.function ? opts.fnColor(s2.function) : '#15171A'));
      out.push(`<rect x="${e.x}" y="${e.y}" width="${e.w}" height="${e.h}" rx="1.5" fill="#ffffff" stroke="${fc}" stroke-width="0.5"/>`);
      if (e.image === 'pictogram') out.push(`<circle cx="${e.x + 4.5}" cy="${e.y + 4.5}" r="2.6" fill="${fc}"/><text x="${e.x + 4.5}" y="${e.y + 5.6}" font-family="${F}" font-size="3" text-anchor="middle" font-weight="bold" fill="#ffffff">${esc(s2.function ?? '·')}</text>`);
      const lines: string[] = [];
      for (const k of e.show as string[]) {
        if (k === 'label') lines.push(e.label ?? s2.name); if (k === 'name') lines.push(s2.name); if (k === 'reference' && s2.reference) lines.push(s2.reference);
        if (k === 'tag') lines.push(s2.tag); if (k === 'category' && s2.category) lines.push(s2.category); if (k === 'dmx' && page.nature === 'lighting') lines.push(dmxText(e));
      }
      const tx = e.x + (e.image && e.image !== 'none' ? 8.5 : 2), max = Math.max(5, Math.floor((e.w - 8) / 1.6));
      lines.forEach((t, i) => out.push(`<text x="${tx}" y="${e.y + 4.6 + i * 3.6}" font-family="${F}" font-size="${i ? 2.6 : 3}" font-weight="${i ? 'normal' : 'bold'}" fill="${i ? '#5B636D' : '#15171A'}">${esc(t.length > max ? t.slice(0, max) + '…' : t)}</text>`));
    }
  }
  for (const l of page.elements.filter((x) => x.type === 'link')) {
    const col = esc(l.color ?? (l.signal_id ? opts.sigColor(l.signal_id) : '#15171A'));
    const ca = at(l.from), cb = at(l.to);
    let d: string, s1: number[], e1: number[];
    if (l.style === 'orthogonal') { const mx = (ca[0] + cb[0]) / 2; const a = edge(l.from, [mx, ca[1]]), b = edge(l.to, [mx, cb[1]]); d = `M${a[0]} ${a[1]} H${mx} V${b[1]} H${b[0]}`; s1 = [mx, a[1], a[0], a[1]]; e1 = [mx, b[1], b[0], b[1]]; }
    else if (l.style === 'curve') { const dx = Math.max(20, Math.abs(cb[0] - ca[0]) / 2); const a = edge(l.from, [ca[0] + dx, ca[1]]), b = edge(l.to, [cb[0] - dx, cb[1]]); d = `M${a[0]} ${a[1]} C${a[0] + dx} ${a[1]} ${b[0] - dx} ${b[1]} ${b[0]} ${b[1]}`; s1 = [a[0] + dx, a[1], a[0], a[1]]; e1 = [b[0] - dx, b[1], b[0], b[1]]; }
    else { const a = edge(l.from, cb), b = edge(l.to, ca); d = `M${a[0]} ${a[1]} L${b[0]} ${b[1]}`; s1 = [b[0], b[1], a[0], a[1]]; e1 = [a[0], a[1], b[0], b[1]]; }
    out.push(`<path d="${d}" fill="none" stroke="${col}" stroke-width="${l.width}"/>`);
    if (l.arrow_end) out.push(arrow(e1[0]!, e1[1]!, e1[2]!, e1[3]!, l.width, col));
    if (l.arrow_start) out.push(arrow(s1[0]!, s1[1]!, s1[2]!, s1[3]!, l.width, col));
    if (l.label) out.push(`<text x="${(ca[0] + cb[0]) / 2}" y="${(ca[1] + cb[1]) / 2 - 1.2}" font-family="${F}" font-size="2.8" text-anchor="middle" fill="${col}">${esc(l.label)}</text>`);
  }
  if (page.legend) {
    const m = new Map<string, string>();
    for (const e of page.elements) if (e.type === 'link' && e.signal_id) m.set(opts.sigName(e.signal_id), e.color ?? opts.sigColor(e.signal_id));
    for (const e of page.elements) if (e.type === 'zone' && e.label) m.set(e.label, e.color ?? '#3A7BD5');
    if (page.nature === 'lighting') { const c = new Map<string, number>(); for (const e of page.elements) if (e.type === 'equipment') c.set(e.snapshot.name, (c.get(e.snapshot.name) ?? 0) + 1); for (const [n, v] of c) m.set(`${v} × ${n}`, ''); }
    const L = [...m.entries()];
    if (L.length) {
      const lh = 6 + L.length * 4, x0 = w - 72, y0 = h - lh - 8;
      out.push(`<g transform="translate(${x0} ${y0})"><rect width="64" height="${lh}" fill="#ffffff" stroke="#15171A" stroke-width="0.3"/><text x="3" y="4.4" font-family="${F}" font-size="2.8" font-weight="bold" fill="#15171A">LÉGENDE</text>${L.map(([n, c], i) => `${c ? `<rect x="3" y="${6.6 + i * 4}" width="6" height="1.4" fill="${esc(c)}"/>` : ''}<text x="${c ? 11 : 3}" y="${7.9 + i * 4}" font-family="${F}" font-size="2.6" fill="#15171A">${esc(n.length > 34 ? n.slice(0, 33) + '…' : n)}</text>`).join('')}</g>`);
    }
  }
  if (page.scale) out.push(`<text x="8" y="${h - 6}" font-family="${F}" font-size="3" fill="#5B636D">Échelle 1:${page.scale}</text>`);
  return { svg: `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}">${out.join('')}</svg>`, w, h };
}

// ----- Cartographie (11.1) : un étage -----

export function mapFloorSvg(d: { floors: any[]; objects: any[]; liaisons: any[]; links: any[] }, floor: any, opts: { bg: string | null; sigName: (id: string) => string }): Rendered | null {
  const objs = d.objects.filter((o) => o.floor_id === floor.id);
  const xs: number[] = [], ys: number[] = [];
  let bgT = '';
  if (opts.bg && floor.bg_w && floor.bg_h) {
    const c = floor.calibration;
    let m = [10, 0, 0, 10, 0, 0];
    if (c) { const dx = c.b[0] - c.a[0], dy = c.b[1] - c.a[1], s = c.dist_mm / Math.hypot(dx, dy), ang = Math.atan2(dy, dx); const co = Math.cos(ang) * s, si = Math.sin(ang) * s; m = [co, -si, si, co, -(co * c.a[0] + si * c.a[1]), -(-si * c.a[0] + co * c.a[1])]; }
    for (const [px, py] of [[0, 0], [floor.bg_w, 0], [0, floor.bg_h], [floor.bg_w, floor.bg_h]] as [number, number][]) { xs.push(m[0]! * px + m[2]! * py + m[4]!); ys.push(m[1]! * px + m[3]! * py + m[5]!); }
    bgT = `<image href="${opts.bg}" width="${floor.bg_w}" height="${floor.bg_h}" transform="matrix(${m.join(' ')})" opacity="${floor.bg_opacity ?? 0.6}" preserveAspectRatio="none"/>`;
  }
  for (const o of objs) { if (o.kind === 'measure') for (const p of o.points) { xs.push(p[0]); ys.push(p[1]); } else { xs.push(o.x); ys.push(o.y); } }
  if (!xs.length) return null;
  const x0 = Math.min(...xs) - 1500, y0 = Math.min(...ys) - 1500, x1 = Math.max(...xs) + 1500, y1 = Math.max(...ys) + 1500;
  const W = x1 - x0, H = y1 - y0, u = Math.max(W, H) / 300; // unité de trait et de texte
  const out: string[] = [`<rect x="${x0}" y="${y0}" width="${W}" height="${H}" fill="#ffffff"/>`, bgT];
  const level = (fid: string) => d.floors.find((f: any) => f.id === fid)?.level;
  const endOf = (cid: string) => { const o = d.objects.find((x) => x.kind === 'container' && x.container_id === cid); return o ? { x: o.x, y: o.y, level: level(o.floor_id) } : null; };
  for (const l of d.liaisons) {
    if (l.state !== 'ready' || !l.waypoints) continue;
    const a = endOf(l.a.id), b = endOf(l.b.id); if (!a || !b) continue;
    const all = [a, ...l.waypoints, b], col = esc(l.color ?? '#5B636D');
    for (let i = 0; i < all.length - 1; i++) {
      const p = all[i], q = all[i + 1];
      if (p.level === floor.level && q.level === floor.level) out.push(`<line x1="${p.x}" y1="${p.y}" x2="${q.x}" y2="${q.y}" stroke="${col}" stroke-width="${u * 1.2}" stroke-linecap="round"/>`);
      else if (p.level === floor.level || q.level === floor.level) { const r = p.level === floor.level ? p : p; out.push(`<circle cx="${r.x}" cy="${r.y}" r="${u * 2.5}" fill="#ffffff" stroke="${col}" stroke-width="${u * 0.6}"/><text x="${r.x}" y="${r.y + u}" font-family="${F}" font-size="${u * 2.6}" text-anchor="middle" fill="${col}">${q.level > p.level ? '▲' : '▼'}</text>`); }
    }
    if (l.length_m != null) { const pts = all.filter((p: any) => p.level === floor.level); if (pts.length) { const p = pts[Math.floor(pts.length / 2)]; out.push(`<text x="${p.x + u * 2}" y="${p.y - u * 2}" font-family="${F}" font-size="${u * 3}" font-weight="bold" fill="${col}">${esc(String(l.length_m).replace('.', ','))} m</text>`); } }
  }
  for (const o of objs) {
    if (o.kind === 'container') {
      const [cw, cd] = (o.rot === 90 || o.rot === 270 ? [o.dims?.[1], o.dims?.[0]] : [o.dims?.[0], o.dims?.[1]]).map((v: number | undefined) => v ?? 600) as [number, number];
      out.push(`<rect x="${o.x - cw / 2}" y="${o.y - cd / 2}" width="${cw}" height="${cd}" fill="#DCE4FF" stroke="#15171A" stroke-width="${u * 0.5}"/><text x="${o.x}" y="${o.y + cd / 2 + u * 4}" font-family="${F}" font-size="${u * 3.4}" font-weight="bold" text-anchor="middle" fill="#15171A">${esc(o.name)}</text>`);
    } else if (o.kind === 'point') {
      out.push(`<rect x="${o.x - u * 2}" y="${o.y - u * 2}" width="${u * 4}" height="${u * 4}" rx="${u * 0.8}" fill="${esc(o.color ?? '#5B636D')}"/><text x="${o.x + u * 3}" y="${o.y + u}" font-family="${F}" font-size="${u * 3}" fill="#15171A">${esc(o.label)} (${o.free}/${o.ports})</text>`);
    } else if (o.kind === 'text') out.push(`<text x="${o.x}" y="${o.y}" font-family="${F}" font-size="${o.size}" fill="#15171A">${esc(o.text)}</text>`);
    else if (o.kind === 'measure') { const last = o.points[o.points.length - 1]; out.push(`<polyline points="${o.points.map((p: number[]) => p.join(',')).join(' ')}" fill="none" stroke="#CF3A2F" stroke-width="${u * 0.5}" stroke-dasharray="${u * 2} ${u}"/><text x="${last[0] + u * 2}" y="${last[1] - u * 2}" font-family="${F}" font-size="${u * 3}" font-weight="bold" fill="#CF3A2F">${esc((o.length_mm / 1000).toFixed(2).replace('.', ','))} m</text>`); }
  }
  // Barre d'échelle
  let sb = 1000; for (const s of [1000, 2000, 5000, 10000, 20000, 50000]) if (s <= W / 5) sb = s;
  out.push(`<g><rect x="${x0 + u * 4}" y="${y1 - u * 8}" width="${sb}" height="${u}" fill="#15171A"/><text x="${x0 + u * 4}" y="${y1 - u * 9.5}" font-family="${F}" font-size="${u * 3}" fill="#15171A">${sb / 1000} m</text></g>`);
  return { svg: `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${x0} ${y0} ${W} ${H}" width="${W}" height="${H}">${out.join('')}</svg>`, w: W, h: H };
}

// ----- Synoptique (10) : une vue d'élément, simplifiée -----

const NW = 270, HH = 52, PS = 28;
export function synopticSvg(syn: { nodes: any[]; cables: any[]; layout: Record<string, { x: number; y: number }> }, opts: { fnColor: (f: string | null) => string }): Rendered | null {
  const nodes = syn.nodes.filter((n) => n.ports.length);
  if (!nodes.length) return null;
  const left = (n: any) => n.ports.filter((p: any) => p.side === 'l'), right = (n: any) => n.ports.filter((p: any) => p.side !== 'l');
  const nh = (n: any) => HH + Math.max(left(n).length, right(n).length, 1) * PS + 12;
  // Positions enregistrées, sinon grille : sources, intermédiaires, destinations.
  const pos: Record<string, { x: number; y: number }> = {};
  const cols: number[] = [40, 40, 40], colX = [40, 560, 1080];
  for (const n of nodes) {
    if (syn.layout[n.key]) { pos[n.key] = syn.layout[n.key]!; continue; }
    const c = n.kind !== 'equipment' ? 1 : !left(n).length ? 0 : !right(n).length ? 2 : 0;
    pos[n.key] = { x: colX[c]!, y: cols[c]! }; cols[c]! += nh(n) + 40;
  }
  const portXY = (ref: string): [number, number, string] | null => {
    for (const n of nodes) { const p = n.ports.find((x: any) => x.ref === ref); if (!p) continue; const list = p.side === 'l' ? left(n) : right(n); const i = list.indexOf(p); const at = pos[n.key]!; return [p.side === 'l' ? at.x : at.x + NW, at.y + HH + i * PS + PS / 2 + 4, p.side]; }
    return null;
  };
  const out: string[] = [];
  const xs: number[] = [], ys: number[] = [];
  for (const n of nodes) { const p = pos[n.key]!; xs.push(p.x - 140, p.x + NW + 140); ys.push(p.y - 30, p.y + nh(n) + 30); }
  // Famille de signal d'un câble : style de trait propre (lisible en noir et blanc).
  const famOf = (ref: string) => { for (const n of nodes) { const p = n.ports.find((x: any) => x.ref === ref); if (p) return (p.family as string | null) ?? ''; } return ''; };
  const fams = new Set<string>();
  const dash = (f: string) => { const d = familyDash(f || null); return d.length ? ` stroke-dasharray="${d.map((x) => x * 2).join(' ')}"${Math.min(...d) <= 1 ? ' stroke-linecap="round"' : ''}` : ''; };
  for (const c of syn.cables) {
    if (c.fields?.attach) continue;
    const A = portXY(c.a), B = portXY(c.b), col = esc(c.fields?.color ?? '#5B636D');
    const fam = famOf(c.a) || famOf(c.b); if (A || B) fams.add(fam);
    if (A && B) {
      const ax = A[0] + (A[2] === 'l' ? -24 : 24), bx = B[0] + (B[2] === 'l' ? -24 : 24);
      out.push(`<path d="M${A[0]} ${A[1]} H${ax} L${bx} ${B[1]} H${B[0]}" fill="none" stroke="${col}" stroke-width="2"${dash(fam)}/>`);
      if (c.fields?.number) out.push(`<text x="${(ax + bx) / 2}" y="${(A[1] + B[1]) / 2 - 4}" font-family="${SVG_MONO}" font-size="11" font-weight="bold" text-anchor="middle" fill="#15171A">${esc(c.fields.number)}</text>`);
    } else if (A || B) {
      const P = (A ?? B)!, far = A ? c.b_info : c.a_info, dx = P[2] === 'l' ? -1 : 1, end = P[0] + dx * 120;
      out.push(`<path d="M${P[0]} ${P[1]} H${end}" fill="none" stroke="${col}" stroke-width="2"${dash(fam)}/><text x="${end + dx * 4}" y="${P[1] + 4}" font-family="${F}" font-size="10" text-anchor="${dx > 0 ? 'start' : 'end'}" fill="#333333">${esc(c.fields?.number ? `${c.fields.number} ${FONT.unicode ? '→' : '»'} ` : `${FONT.unicode ? '→' : '»'} `)}${esc(far ? `${far.label} › ${far.port}` : '')}</text>`);
      xs.push(end + dx * 160);
    }
  }
  for (const n of nodes) {
    const p = pos[n.key]!, h = nh(n), col = esc(n.function ? opts.fnColor(n.function) : '#8A919B');
    out.push(`<rect x="${p.x}" y="${p.y}" width="${NW}" height="${h}" rx="10" fill="#ffffff" stroke="#15171A" stroke-width="1.2"/><rect x="${p.x}" y="${p.y}" width="6" height="${h}" fill="${col}"/>`);
    out.push(`<text x="${p.x + 16}" y="${p.y + 22}" font-family="${F}" font-size="14" font-weight="bold" fill="#15171A">${esc(n.label.length > 30 ? n.label.slice(0, 29) + '…' : n.label)}</text><text x="${p.x + 16}" y="${p.y + 39}" font-family="${F}" font-size="11" fill="#333333">${esc(n.sub.length > 42 ? n.sub.slice(0, 41) + '…' : n.sub)}</text>`);
    left(n).forEach((q: any, i: number) => out.push(`<circle cx="${p.x}" cy="${p.y + HH + i * PS + PS / 2 + 4}" r="4" fill="#ffffff" stroke="#15171A" stroke-width="1.2"/><text x="${p.x + 10}" y="${p.y + HH + i * PS + PS / 2 + 8}" font-family="${F}" font-size="11" fill="#15171A">${esc(q.label.length > 20 ? q.label.slice(0, 19) + '…' : q.label)}</text>`));
    right(n).forEach((q: any, i: number) => out.push(`<circle cx="${p.x + NW}" cy="${p.y + HH + i * PS + PS / 2 + 4}" r="4" fill="#ffffff" stroke="#15171A" stroke-width="1.2"/><text x="${p.x + NW - 10}" y="${p.y + HH + i * PS + PS / 2 + 8}" font-family="${F}" font-size="11" text-anchor="end" fill="#15171A">${esc(q.label.length > 20 ? q.label.slice(0, 19) + '…' : q.label)}</text>`));
  }
  const x0 = Math.min(...xs), y0 = Math.min(...ys), W = Math.max(...xs) - x0, H = Math.max(...ys) - y0;
  return { svg: `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${x0} ${y0} ${W} ${H}" width="${W}" height="${H}"><rect x="${x0}" y="${y0}" width="${W}" height="${H}" fill="#ffffff"/>${out.join('')}</svg>`, w: W, h: H, families: [...fams] };
}
