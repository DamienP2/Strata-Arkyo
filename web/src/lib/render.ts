// Rendus SVG autonomes pour le dossier (section 11.6) et les planches : pages de dessin, étages
// de carte et synoptique par élément, à partir des données de l'API, en palette claire et
// polices incorporées au PDF (rendu fidèle). Les images sont passées en data URL avant l'appel.
// Couleurs : une liaison, un câble ou un port prend la couleur de sa famille de signal
// (familyColor, papier), FAMILY_NONE sans famille ; petits textes (moins de 2,82 mm) en #333.

import { FAMILY_NONE, familyColor, refById } from './catalog.svelte';
import { rackLines } from './racktext.svelte';
import { routeAll, rounded, type Req as RouteReq } from './cableroute';
import { fitText, shareRow, textWidth } from './fit';
import { FONT, HEX, SVG_MONO, SVG_SANS, familyDash } from './pdfkit';

const esc = (s: unknown) => String(s ?? '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const F = SVG_SANS;
/** `families` : familles de signal dessinées (légende du PDF) ; `blocks` : rectangles à ne pas
 *  couper en découpant en feuilles (unités du dessin, origine au coin du viewBox). */
export interface Rendered { svg: string; w: number; h: number; families?: string[]; blocks?: { box: [number, number, number, number]; title: string }[] }
/** Famille de signal d'un signal du catalogue. */
const sigFamily = (id: string | null | undefined): string | null => (id ? (refById('signals', id)?.data?.function as string | undefined) ?? null : null);
/** Raccourcit un texte au dernier mot entier qui tient (largeur estimée), sans points de suspension. */
const cutWord = (s: string, max: number) => { if (s.length <= max) return s; const c = s.slice(0, max), sp = c.lastIndexOf(' '); return (sp > max * 0.6 ? c.slice(0, sp) : c).trimEnd(); };

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

export function drawingSvg(page: { paper: string; landscape: boolean; background: string | null; legend: boolean; scale: number | null; nature: string; dmx_display: string; elements: El[] }, opts: { images: Record<string, string> }): Rendered {
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
      const s2 = e.snapshot, fc = esc(e.frame ?? (s2.function ? familyColor(s2.function, { screen: false }) : HEX.ink));
      out.push(`<rect x="${e.x}" y="${e.y}" width="${e.w}" height="${e.h}" rx="1.5" fill="#ffffff" stroke="${fc}" stroke-width="0.5"/>`);
      if (e.image === 'pictogram') out.push(`<circle cx="${e.x + 4.5}" cy="${e.y + 4.5}" r="2.6" fill="${fc}"/><text x="${e.x + 4.5}" y="${e.y + 5.6}" font-family="${F}" font-size="3" text-anchor="middle" font-weight="bold" fill="#ffffff">${esc(s2.function ?? '·')}</text>`);
      const lines: string[] = [];
      for (const k of e.show as string[]) {
        if (k === 'label') lines.push(e.label ?? s2.name); if (k === 'name') lines.push(s2.name); if (k === 'reference' && s2.reference) lines.push(s2.reference);
        if (k === 'tag') lines.push(s2.tag); if (k === 'category' && s2.category) lines.push(s2.category); if (k === 'dmx' && page.nature === 'lighting') lines.push(dmxText(e));
      }
      const tx = e.x + (e.image && e.image !== 'none' ? 8.5 : 2), max = Math.max(5, Math.floor((e.w - 8) / 1.6));
      lines.forEach((t, i) => out.push(`<text x="${tx}" y="${e.y + 4.6 + i * 3.6}" font-family="${F}" font-size="${i ? 2.6 : 3}" font-weight="${i ? 'normal' : 'bold'}" fill="${i ? HEX.dark : HEX.ink}">${esc(cutWord(t, max))}</text>`));
    }
  }
  // Liaisons : couleur et motif de trait de la famille du signal (lisible en noir et blanc) ;
  // sans signal, la couleur choisie, à défaut le gris des liaisons sans famille.
  const fams = new Set<string>();
  for (const l of page.elements.filter((x) => x.type === 'link')) {
    const fam = sigFamily(l.signal_id);
    if (l.signal_id) fams.add(fam ?? '');
    const col = esc(l.signal_id ? familyColor(fam, { screen: false }) : l.color ?? FAMILY_NONE);
    const dl = l.signal_id ? familyDash(fam) : [];
    const dashAttr = dl.length ? ` stroke-dasharray="${dl.map((x) => +(x * l.width).toFixed(2)).join(' ')}"${Math.min(...dl) <= 1 ? ' stroke-linecap="round"' : ''}` : '';
    const ca = at(l.from), cb = at(l.to);
    let d: string, s1: number[], e1: number[];
    if (l.style === 'orthogonal') { const mx = (ca[0] + cb[0]) / 2; const a = edge(l.from, [mx, ca[1]]), b = edge(l.to, [mx, cb[1]]); d = `M${a[0]} ${a[1]} H${mx} V${b[1]} H${b[0]}`; s1 = [mx, a[1], a[0], a[1]]; e1 = [mx, b[1], b[0], b[1]]; }
    else if (l.style === 'curve') { const dx = Math.max(20, Math.abs(cb[0] - ca[0]) / 2); const a = edge(l.from, [ca[0] + dx, ca[1]]), b = edge(l.to, [cb[0] - dx, cb[1]]); d = `M${a[0]} ${a[1]} C${a[0] + dx} ${a[1]} ${b[0] - dx} ${b[1]} ${b[0]} ${b[1]}`; s1 = [a[0] + dx, a[1], a[0], a[1]]; e1 = [b[0] - dx, b[1], b[0], b[1]]; }
    else { const a = edge(l.from, cb), b = edge(l.to, ca); d = `M${a[0]} ${a[1]} L${b[0]} ${b[1]}`; s1 = [b[0], b[1], a[0], a[1]]; e1 = [a[0], a[1], b[0], b[1]]; }
    out.push(`<path d="${d}" fill="none" stroke="${col}" stroke-width="${l.width}"${dashAttr}/>`);
    if (l.arrow_end) out.push(arrow(e1[0]!, e1[1]!, e1[2]!, e1[3]!, l.width, col));
    if (l.arrow_start) out.push(arrow(s1[0]!, s1[1]!, s1[2]!, s1[3]!, l.width, col));
    if (l.label) out.push(`<text x="${(ca[0] + cb[0]) / 2}" y="${(ca[1] + cb[1]) / 2 - 1.2}" font-family="${F}" font-size="2.8" text-anchor="middle" fill="${col}">${esc(l.label)}</text>`);
  }
  if (page.legend) {
    // Légende commune aux exports : « Familles de signal » (trait de 12 mm avec le motif de la
    // famille, jamais les noms de signaux), puis un bloc « Zones » séparé, puis les appareils.
    type Row = { kind: 'fam' | 'zone' | 'text'; label: string; color?: string; dash?: number[] };
    const blocks: [string, Row[]][] = [];
    const famRows: Row[] = [...fams].map((f) => ({ kind: 'fam' as const, label: f ? refById('functions', f)?.name ?? f : 'Famille non définie', color: familyColor(f || null, { screen: false }), dash: familyDash(f || null) })).sort((a, b) => a.label.localeCompare(b.label, 'fr'));
    if (famRows.length) blocks.push(['Familles de signal', famRows]);
    const zones = new Map<string, string>();
    for (const e of page.elements) if (e.type === 'zone' && e.label) zones.set(e.label, e.color ?? '#3A7BD5');
    if (zones.size) blocks.push(['Zones', [...zones].map(([label, color]) => ({ kind: 'zone' as const, label, color }))]);
    if (page.nature === 'lighting') { const c = new Map<string, number>(); for (const e of page.elements) if (e.type === 'equipment') c.set(e.snapshot.name, (c.get(e.snapshot.name) ?? 0) + 1); if (c.size) blocks.push(['Appareils', [...c].map(([n, v]) => ({ kind: 'text' as const, label: `${v} × ${n}` }))]); }
    if (blocks.length) {
      const LW_ = 70, rows = blocks.reduce((a, [, r]) => a + r.length, 0), lh = 3 + blocks.length * 5.4 + rows * 4, x0 = w - LW_ - 10, y0 = h - lh - 10;
      const g: string[] = [`<rect width="${LW_}" height="${lh}" fill="#ffffff" stroke="${HEX.ink}" stroke-width="0.35"/>`];
      let y = 3;
      for (const [title, rs] of blocks) {
        g.push(`<text x="3" y="${y + 3}" font-family="${F}" font-size="2.82" font-weight="bold" fill="${HEX.ink}">${esc(title)}</text>`); y += 5.4;
        for (const r of rs) {
          const c = esc(r.color ?? HEX.ink), tx = r.kind === 'text' ? 3 : 18;
          if (r.kind === 'fam') g.push(`<line x1="3" x2="15" y1="${y - 1}" y2="${y - 1}" stroke="${c}" stroke-width="0.6"${r.dash?.length ? ` stroke-dasharray="${r.dash.map((x) => +(x * 0.6).toFixed(2)).join(' ')}"` : ''}/>`);
          if (r.kind === 'zone') g.push(`<rect x="3" y="${y - 2.4}" width="12" height="3" rx="0.6" fill="${c}" fill-opacity="0.1" stroke="${c}" stroke-width="0.3" stroke-dasharray="2 1.2"/>`);
          g.push(`<text x="${tx}" y="${y}" font-family="${F}" font-size="2.82" fill="${HEX.ink}">${esc(cutWord(r.label, 34))}</text>`); y += 4;
        }
      }
      out.push(`<g transform="translate(${x0} ${y0})">${g.join('')}</g>`);
    }
  }
  if (page.scale) out.push(`<text x="8" y="${h - 6}" font-family="${F}" font-size="3" fill="${HEX.mute}">Échelle 1:${page.scale}</text>`);
  return { svg: `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}">${out.join('')}</svg>`, w, h, families: [...fams] };
}

// ----- Cartographie (11.1) : un étage -----

export function mapFloorSvg(d: { floors: any[]; objects: any[]; liaisons: any[]; links: any[] }, floor: any, opts: { bg: string | null }): Rendered | null {
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
    // Couleur de la liaison : sa famille de signal, gris des liaisons sans famille sinon.
    const all = [a, ...l.waypoints, b], col = esc(l.family ? familyColor(l.family, { screen: false }) : FAMILY_NONE);
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
      out.push(`<rect x="${o.x - cw / 2}" y="${o.y - cd / 2}" width="${cw}" height="${cd}" fill="#FFFFFF" stroke="${HEX.ink}" stroke-width="${u * 0.5}"/><text x="${o.x}" y="${o.y + cd / 2 + u * 4}" font-family="${F}" font-size="${u * 3.4}" font-weight="bold" text-anchor="middle" fill="${HEX.ink}">${esc(o.name)}</text>`);
    } else if (o.kind === 'point') {
      out.push(`<rect x="${o.x - u * 2}" y="${o.y - u * 2}" width="${u * 4}" height="${u * 4}" rx="${u * 0.8}" fill="${esc(o.color ?? FAMILY_NONE)}"/><text x="${o.x + u * 3}" y="${o.y + u}" font-family="${F}" font-size="${u * 3}" fill="#15171A">${esc(o.label)} (${o.free}/${o.ports})</text>`);
    } else if (o.kind === 'text') out.push(`<text x="${o.x}" y="${o.y}" font-family="${F}" font-size="${o.size}" fill="#15171A">${esc(o.text)}</text>`);
    else if (o.kind === 'measure') { const last = o.points[o.points.length - 1]; out.push(`<polyline points="${o.points.map((p: number[]) => p.join(',')).join(' ')}" fill="none" stroke="${HEX.measure}" stroke-width="${u * 0.5}" stroke-dasharray="${u * 2} ${u}"/><text x="${last[0] + u * 2}" y="${last[1] - u * 2}" font-family="${F}" font-size="${u * 3}" font-weight="bold" fill="${HEX.measure}">${esc((o.length_mm / 1000).toFixed(2).replace('.', ','))} m</text>`); }
  }
  // Barre d'échelle
  let sb = 1000; for (const s of [1000, 2000, 5000, 10000, 20000, 50000]) if (s <= W / 5) sb = s;
  out.push(`<g><rect x="${x0 + u * 4}" y="${y1 - u * 8}" width="${sb}" height="${u}" fill="#15171A"/><text x="${x0 + u * 4}" y="${y1 - u * 9.5}" font-family="${F}" font-size="${u * 3}" fill="#15171A">${sb / 1000} m</text></g>`);
  return { svg: `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${x0} ${y0} ${W} ${H}" width="${W}" height="${H}">${out.join('')}</svg>`, w: W, h: H };
}

// ----- Synoptique (10) : une vue d'élément, au rendu de l'écran -----

/** Géométrie des nœuds, la même à l'écran (synoptique, aperçu de la fiche) et dans les exports :
 *  largeur, en-tête, pas des ports, écart entre le port et l'étiquette du câble. */
export const SYN = { NW: 270, HH: 52, PS: 28, TAG_GAP: 26 } as const;
/** Polices des nœuds (comme le CSS de l'écran) : titre 15, sous-titre 11 mono, port 12, connecteur 8,5 mono. */
const SANS_CSS = "'Inter Variable', Inter, system-ui, sans-serif", MONO_CSS = "'JetBrains Mono', ui-monospace, monospace";
const MONO_S = `600 9.5px ${MONO_CSS}`, MONO_N = `700 11px ${MONO_CSS}`, FAR_F = `600 11px ${SANS_CSS}`;
const TITLE_F = `650 15px ${SANS_CSS}`, SUB_F = `500 11px ${MONO_CSS}`;
const TAG_L = 10, TAG_SEP = 6, TAG_R = 8;

/** Nom court d'un connecteur (« BNC », « XLR 3 ») : un nom trop long est abrégé par ses
 *  initiales (« DisplayPort » → « DP », « Mini DisplayPort » → « Mini DP »), jamais coupé. */
export function connShort(id: string | null | undefined): string {
  const n = (id && refById('connectors', id)?.name) || id || '';
  const s = n.replace(/\s*\/.*$/, '').replace(/ monophasé| triphasé/, '');
  if (s.length <= 9) return s;
  const caps = (w: string) => w.replace(/[^A-Z0-9]/g, '');
  const parts = s.split(/\s+/).map((w) => (w.length > 6 && caps(w).length >= 2 ? caps(w) : w));
  const r = parts.join(' ');
  if (r.length <= 9) return r;
  return parts[0]!.length <= 9 ? parts[0]! : caps(parts[0]!) || parts[0]!.slice(0, 9);
}
/** Pastille d'un port : connecteur et genre. */
export const portChip = (p: { connector: string; gender?: string | null }) => connShort(p.connector) + (p.gender === 'M' ? ' M' : p.gender === 'F' ? ' F' : '');

/** Lignes d'un nœud : [titre, sous-titre]. Équipement : le libellé de l'exemplaire, à défaut son
 *  ID ; en petit la marque et le modèle (l'ID quand le titre est le libellé), puis l'élément.
 *  Le moteur les calcule déjà (`label`, `sub`) ; une réponse plus ancienne, sans `model`, est
 *  recalculée ici avec la même règle (rackLines). Panels et adaptateurs : nom et sous-titre. */
export function nodeLines(n: { kind?: string; label: string; sub: string; model?: string; custom?: string | null; tag?: string | null; brand?: string | null; place?: string }): [string, string] {
  if (n.model !== undefined || (n.kind && n.kind !== 'equipment') || n.place === undefined) return [n.label, n.sub];
  const [main, model] = rackLines({ label: n.label, custom: n.custom, tag: n.tag, brand: n.brand }, 'both');
  return [main, [model, n.custom?.trim() && n.tag ? n.tag : '', n.place].filter(Boolean).join(' · ')];
}

type SynNodeData = { key: string; kind: string; label: string; sub: string; function: string | null; ports: any[]; model?: string; custom?: string | null; tag?: string | null; brand?: string | null; place?: string };
type Pos = Record<string, { x: number; y: number }>;
/** Disposition automatique de l'écran : sources, panels et adaptateurs, destinations, chaque
 *  famille repliée en sous-colonnes au-delà de 1 400 unités de haut. */
function synLayout(nodes: SynNodeData[], nh: (n: SynNodeData) => number): Pos {
  const L = (n: SynNodeData) => n.ports.filter((p) => p.side === 'l').length, R = (n: SynNodeData) => n.ports.filter((p) => p.side !== 'l').length;
  const col = (n: SynNodeData) => (n.kind !== 'equipment' ? 1 : L(n) === 0 ? 0 : R(n) === 0 ? 2 : L(n) > R(n) ? 2 : 0);
  const MAXH = 1400, COLW = 480;
  const groups: SynNodeData[][] = [[], [], []];
  for (const n of [...nodes].sort((a, b) => a.sub.localeCompare(b.sub, 'fr') || a.label.localeCompare(b.label, 'fr'))) groups[col(n)]!.push(n);
  const out: Pos = {};
  let x = 40;
  for (const g of groups) {
    if (!g.length) continue;
    let y = 40;
    for (const n of g) { if (y > 40 && y + nh(n) > MAXH) { x += COLW; y = 40; } out[n.key] = { x, y }; y += nh(n) + 44; }
    x += COLW;
  }
  return out;
}

/** Un nœud, dessiné comme à l'écran (couleurs papier) : en-tête teinté et bande à la couleur de
 *  la famille, titre et sous-titre, ports à la couleur de leur famille avec sens, nom et
 *  connecteur. `data-block` : le découpage en feuilles ne coupe pas le nœud. */
export function synNodeSvg(n: SynNodeData, x: number, y: number, fitted: { l: string; r: string }[]): string {
  const { NW, HH, PS } = SYN;
  const L = n.ports.filter((p) => p.side === 'l'), R = n.ports.filter((p) => p.side !== 'l');
  const h = HH + Math.max(L.length, R.length, 1) * PS + 12;
  const col = n.function ? familyColor(n.function, { screen: false }) : n.kind === 'panel' ? HEX.mute : FAMILY_NONE;
  const [t1, t2] = nodeLines(n);
  const title = fitText(t1 || 'Équipement', NW - 44, TITLE_F), sub = fitText(t2, NW - 44, SUB_F);
  const o: string[] = [];
  o.push(`<g data-block="${esc(t1)}" transform="translate(${x} ${y})">`);
  o.push(`<rect width="${NW}" height="${h}" rx="10" fill="#FFFFFF" stroke="#CEC9BF" stroke-width="1"/>`);
  o.push(`<path d="M0 10a10 10 0 0 1 10-10h${NW - 20}a10 10 0 0 1 10 10v${HH - 10}H0z" fill="${esc(col)}" fill-opacity="0.09"/>`);
  o.push(`<rect width="${NW}" height="4" rx="2" fill="${esc(col)}"/><line x1="0" x2="${NW}" y1="${HH}" y2="${HH}" stroke="#E2DED6" stroke-width="1"/>`);
  o.push(`<circle cx="16" cy="${HH / 2 + 1}" r="4.5" fill="${esc(col)}"/>`);
  o.push(`<text x="30" y="25" font-family="${F}" font-size="15" font-weight="bold" fill="${HEX.ink}">${esc(title)}</text>`);
  o.push(`<text x="30" y="42" font-family="${SVG_MONO}" font-size="11" fill="${HEX.dark}">${esc(sub)}</text>`);
  for (const [list, out] of [[L, false], [R, true]] as const) {
    list.forEach((p: any, i: number) => {
      const px = out ? NW : 0, py = HH + i * PS + PS / 2 + 4, mk = out ? NW - 19 : 12, tx = out ? NW - 24 : 24;
      const c = esc(familyColor(p.family, { screen: false }));
      o.push(`<circle cx="${px}" cy="${py}" r="4.5" fill="#FFFFFF" stroke="${c}" stroke-width="2"/>`);
      if (p.dir === 'in' || p.dir === 'out') o.push(`<path d="M${mk} ${py - 4.5}l7 4.5-7 4.5z" fill="${c}"/>`);
      else o.push(`<path d="M${mk + 3.5} ${py - 4.5}l-3.5 4.5 3.5 4.5z" fill="${c}"/><path d="M${mk + 3.5} ${py - 4.5}l3.5 4.5-3.5 4.5z" fill="${HEX.mute}"/>`);
      const anchor = out ? ' text-anchor="end"' : '';
      o.push(`<text x="${tx}" y="${py - 0.5}" font-family="${F}" font-size="12" font-weight="bold" fill="${HEX.ink}"${anchor}>${esc((out ? fitted[i]?.r : fitted[i]?.l) ?? p.label)}</text>`);
      o.push(`<text x="${tx}" y="${py + 10}" font-family="${SVG_MONO}" font-size="8.5" letter-spacing="0.25" fill="${HEX.dark}"${anchor}>${esc(portChip(p))}</text>`);
    });
  }
  o.push('</g>');
  return o.join('');
}

/** Synoptique d'une vue, au rendu de l'écran : positions enregistrées (sinon la disposition
 *  automatique de l'écran), câbles routés comme à l'écran (couloirs sans chevauchement, tracés
 *  posés à la main), étiquettes connecteur et numéro à chaque extrémité, renvoi vers l'autre
 *  bout pour un câble qui sort de la vue. Couleur d'un câble et d'un port : sa famille de signal
 *  uniquement ; motif de trait par famille (lisible en noir et blanc). `families` : familles des
 *  câbles et des ports dessinés (légende). */
export function synopticSvg(syn: { nodes: any[]; cables: any[]; layout: Record<string, { x: number; y: number }> }): Rendered | null {
  const { NW, HH, PS, TAG_GAP } = SYN;
  const nodes = (syn.nodes as SynNodeData[]).filter((n) => n.ports.length);
  if (!nodes.length) return null;
  const left = (n: SynNodeData) => n.ports.filter((p) => p.side === 'l'), right = (n: SynNodeData) => n.ports.filter((p) => p.side !== 'l');
  const nh = (n: SynNodeData) => HH + Math.max(left(n).length, right(n).length, 1) * PS + 12;
  const pos: Pos = { ...synLayout(nodes, nh), ...syn.layout };
  const byRef = new Map<string, { n: SynNodeData; p: any }>();
  for (const n of nodes) for (const p of n.ports) byRef.set(p.ref, { n, p });
  const portXY = (ref: string): [number, number, 'l' | 'r'] | null => {
    const e = byRef.get(ref); if (!e) return null;
    const list = e.p.side === 'l' ? left(e.n) : right(e.n), i = list.indexOf(e.p), at = pos[e.n.key] ?? { x: 0, y: 0 };
    return [e.p.side === 'l' ? at.x : at.x + NW, at.y + HH + i * PS + PS / 2 + 4, e.p.side === 'l' ? 'l' : 'r'];
  };
  const famOf = (c: any): string => byRef.get(c.a)?.p.family ?? byRef.get(c.b)?.p.family ?? '';
  const colOf = (f: string) => esc(familyColor(f || null, { screen: false }));
  const dash = (f: string, sw: number) => { const d = familyDash(f || null); return d.length ? ` stroke-dasharray="${d.map((x) => +(x * sw).toFixed(2)).join(' ')}"${Math.min(...d) <= 1 ? ' stroke-linecap="round"' : ''}` : ''; };
  const connOf = (c: any) => connShort(byRef.get(c.a)?.p.connector ?? byRef.get(c.b)?.p.connector ?? c.a_info?.connector ?? '');
  const tagW = (c: any) => Math.ceil(TAG_L + textWidth(connOf(c), MONO_S) + TAG_SEP + textWidth(c.fields?.number || '—', MONO_N) + TAG_R);
  const farName = (info: any) => (info ? (info.external ? info.label : `${info.label} › ${info.port}`) : 'Extrémité manquante');
  const boxes = nodes.map((n) => { const p = pos[n.key] ?? { x: 0, y: 0 }; return [p.x - 6, p.y - 8, p.x + NW + 6, p.y + nh(n) + 8] as [number, number, number, number]; });
  const cables = syn.cables.filter((c) => !c.fields?.attach && (byRef.has(c.a) || byRef.has(c.b)));
  // Tracés posés à la main (sortie horizontale, points de passage, entrée), sinon routage global.
  const manual = (c: any, A: [number, number, string], B: [number, number, string]): [number, number][] | null => {
    const wp: [number, number][] = c.fields?.waypoints ?? []; if (!wp.length) return null;
    const out = TAG_GAP * 2 + tagW(c);
    const exA: [number, number] = [A[0] + (A[2] === 'r' ? out : -out), A[1]], exB: [number, number] = [B[0] + (B[2] === 'r' ? out : -out), B[1]];
    const raw: [number, number][] = [[A[0], A[1]], ...(wp[0]![1] === A[1] ? [] : [exA]), ...wp, ...(wp[wp.length - 1]![1] === B[1] ? [] : [exB]), [B[0], B[1]]];
    const pts: [number, number][] = [raw[0]!];
    for (let i = 1; i < raw.length; i++) { const p = pts[pts.length - 1]!, q = raw[i]!; if (p[0] !== q[0] && p[1] !== q[1]) pts.push([q[0], p[1]]); pts.push(q); }
    return pts.filter((q, i) => i === 0 || q[0] !== pts[i - 1]![0] || q[1] !== pts[i - 1]![1]);
  };
  const reqs: RouteReq[] = [], fixed: [number, number][][] = [];
  for (const c of cables) {
    const A = portXY(c.a), B = portXY(c.b); if (!A || !B || c.fields?.style === 'arrow') continue;
    const m = manual(c, A, B); if (m) { fixed.push(m); continue; }
    reqs.push({ id: c.id, a: { x: A[0], y: A[1], side: A[2] }, b: { x: B[0], y: B[1], side: B[2] }, out: tagW(c) + TAG_GAP * 2 + 12, tag: TAG_GAP + tagW(c) / 2 });
  }
  const routed = routeAll(reqs, boxes, fixed);
  const xs: number[] = [], ys: number[] = [];
  for (const n of nodes) { const p = pos[n.key]!; xs.push(p.x - 40, p.x + NW + 40); ys.push(p.y - 30, p.y + nh(n) + 30); }
  const fams = new Set<string>();
  for (const n of nodes) for (const p of n.ports) fams.add(p.family ?? '');
  const lines: string[] = [], tags: string[] = [];
  const tag = (c: any, P: [number, number, string], col: string) => {
    const w = tagW(c), x = P[2] === 'r' ? P[0] + TAG_GAP : P[0] - TAG_GAP - w, cw = textWidth(connOf(c), MONO_S);
    xs.push(x - 4, x + w + 4);
    const num = c.fields?.number;
    tags.push(`<g transform="translate(${x} ${P[1] - 9})"><rect width="${w}" height="18" rx="4" fill="${num ? '#FFFFFF' : '#F6EED9'}" stroke="${col}" stroke-width="1"${num ? '' : ' stroke-dasharray="3 2"'}/><rect x="3.5" y="3.5" width="3" height="11" rx="1" fill="${col}"/>`
      + `<text x="${TAG_L}" y="12.5" font-family="${SVG_MONO}" font-size="9.5" font-weight="bold" fill="${HEX.dark}">${esc(connOf(c))}</text>`
      + `<text x="${TAG_L + cw + TAG_SEP}" y="13" font-family="${SVG_MONO}" font-size="11" font-weight="bold" fill="${num ? HEX.ink : HEX.dark}"${num ? '' : ' font-style="italic"'}>${esc(num || '—')}</text></g>`);
  };
  const far = (x: number, y: number, dx: number, text: string, col: string) => {
    const ft = fitText(text, 260, FAR_F), fw = Math.ceil(textWidth(ft, FAR_F)) + 18, x0 = dx > 0 ? x + 4 : x - 4 - fw;
    xs.push(x0 - 4, x0 + fw + 4);
    tags.push(`<g transform="translate(${x0} ${y - 11})"><rect width="${fw}" height="22" rx="6" fill="#FFFFFF" stroke="${col}" stroke-width="1.2"/><text x="9" y="15" font-family="${F}" font-size="11" font-weight="bold" fill="${HEX.dark}">${esc(ft)}</text></g>`);
  };
  const arrowHead = (x2: number, y2: number, a: number) => { const L = 11, W = 5; return `M${x2} ${y2}L${x2 - Math.cos(a) * L + Math.sin(a) * W} ${y2 - Math.sin(a) * L - Math.cos(a) * W}L${x2 - Math.cos(a) * L - Math.sin(a) * W} ${y2 - Math.sin(a) * L + Math.cos(a) * W}Z`; };
  const arrowTo = FONT.unicode ? '→' : '»';
  for (const c of cables) {
    const fam = famOf(c), col = colOf(fam), A = portXY(c.a), B = portXY(c.b);
    fams.add(fam);
    if (A && B && c.fields?.style === 'arrow') {
      // Flèche : un renvoi à chaque extrémité, vers l'équipement et le port d'en face.
      for (const [P, other] of [[A, c.b], [B, c.a]] as const) {
        const dx = P[2] === 'r' ? 1 : -1, end = P[0] + dx * (tagW(c) + TAG_GAP * 2 + 50), o = byRef.get(other);
        lines.push(`<path d="M${P[0]} ${P[1]}H${end}" fill="none" stroke="${col}" stroke-width="2.6"${dash(fam, 2.6)}/><path d="${arrowHead(end + dx * 8, P[1], dx > 0 ? 0 : Math.PI)}" fill="${col}"/>`);
        far(end + dx * 8, P[1], dx, `${arrowTo} ${o ? `${nodeLines(o.n)[0]} › ${o.p.label}` : 'Extrémité manquante'}`, col);
        tag(c, P, col);
      }
    } else if (A && B) {
      const pts = manual(c, A, B) ?? routed.get(c.id)?.pts ?? [[A[0], A[1]], [B[0], B[1]]];
      for (const q of pts) { xs.push(q[0]); ys.push(q[1]); }
      lines.push(`<path d="${c.fields?.style === 'rounded' ? rounded(pts, 12) : rounded(pts, 4)}" fill="none" stroke="${col}" stroke-width="2.2" stroke-linejoin="round"${dash(fam, 2.2)}/>`);
      tag(c, A, col); tag(c, B, col);
    } else {
      // Une seule extrémité dans la vue : renvoi vers l'autre bout.
      const P = (A ?? B)!, info = A ? c.b_info : c.a_info, dx = P[2] === 'r' ? 1 : -1, end = P[0] + dx * (tagW(c) + TAG_GAP * 2 + 60);
      lines.push(`<path d="M${P[0]} ${P[1]}H${end}" fill="none" stroke="${col}" stroke-width="2.2"${dash(fam, 2.2)}/>`);
      far(end, P[1], dx, `${info?.external ? '↗' : arrowTo} ${farName(info)}`, col);
      tag(c, P, col);
    }
  }
  const body: string[] = [];
  for (const n of nodes) {
    const p = pos[n.key]!, L = left(n), R = right(n), rows: { l: string; r: string }[] = [];
    for (let i = 0; i < Math.max(L.length, R.length); i++) {
      const a = L[i], b = R[i];
      const [wl, wr] = shareRow(a ? textWidth(a.label) : null, b ? textWidth(b.label) : null, NW - 48);
      rows.push({ l: a ? fitText(a.label, wl) : '', r: b ? fitText(b.label, wr) : '' });
    }
    body.push(synNodeSvg(n, p.x, p.y, rows));
  }
  const x0 = Math.min(...xs), y0 = Math.min(...ys), W = Math.max(...xs) - x0, H = Math.max(...ys) - y0;
  const blocks = nodes.map((n) => { const p = pos[n.key]!; return { box: [p.x - x0, p.y - y0, p.x + NW - x0, p.y + nh(n) - y0] as [number, number, number, number], title: nodeLines(n)[0] }; });
  return { svg: `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${x0} ${y0} ${W} ${H}" width="${W}" height="${H}"><rect x="${x0}" y="${y0}" width="${W}" height="${H}" fill="#ffffff"/>${lines.join('')}${body.join('')}${tags.join('')}</svg>`, w: W, h: H, families: [...fams], blocks };
}
