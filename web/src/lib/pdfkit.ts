// Boîte à outils PDF commune (sections 11.6 et 11.7) : polices Unicode incorporées, bloc
// d'identité (logo, titre, projet, date, version), tableaux paginés, cartouches selon les
// modèles du site, pages graphiques (tuiles quand le texte deviendrait illisible), pagination
// continue « n / N ». Règles d'impression : marges d'au moins 10 mm, aucun texte sous 6 pt,
// tout texte de moins de 8 pt en gris foncé (#333) ou noir.

import type { jsPDF } from 'jspdf';
import { api } from './api';
import { session } from './session.svelte';

export type Doc = jsPDF;
export type PaperKey = 'A4' | 'A3' | 'A2' | 'A1' | 'A0' | 'Letter' | 'Legal';
export const PAPER_MM: Record<PaperKey, [number, number]> = {
  A4: [210, 297], A3: [297, 420], A2: [420, 594], A1: [594, 841], A0: [841, 1189], Letter: [215.9, 279.4], Legal: [215.9, 355.6],
};
export const pageMm = (p: PaperKey, landscape: boolean): [number, number] => { const [a, b] = PAPER_MM[p]; return landscape ? [b, a] : [a, b]; };

// ----- Polices -----

/** Polices en cours : Inter et JetBrains Mono incorporées (Unicode), sinon les polices
 * standard du PDF (WinAnsi, sans flèches). */
export const FONT = { sans: 'helvetica', mono: 'courier', unicode: false };
/** Familles à écrire dans les SVG passés à svg2pdf (repli sur les polices standard). */
export const SVG_SANS = 'Inter, Helvetica, Arial, sans-serif';
export const SVG_MONO = 'JetBrainsMono, Courier, monospace';
const FONT_FILES: [string, string, string, string][] = [
  ['Inter-Regular.ttf', 'Inter', 'normal', new URL('../assets/fonts/Inter-Regular.ttf', import.meta.url).href],
  ['Inter-Bold.ttf', 'Inter', 'bold', new URL('../assets/fonts/Inter-Bold.ttf', import.meta.url).href],
  ['Inter-Italic.ttf', 'Inter', 'italic', new URL('../assets/fonts/Inter-Italic.ttf', import.meta.url).href],
  ['Inter-BoldItalic.ttf', 'Inter', 'bolditalic', new URL('../assets/fonts/Inter-BoldItalic.ttf', import.meta.url).href],
  ['JetBrainsMono-Regular.ttf', 'JetBrainsMono', 'normal', new URL('../assets/fonts/JetBrainsMono-Regular.ttf', import.meta.url).href],
  ['JetBrainsMono-Bold.ttf', 'JetBrainsMono', 'bold', new URL('../assets/fonts/JetBrainsMono-Bold.ttf', import.meta.url).href],
];
/** Pas d'italique à chasse fixe : le droit le remplace (le texte reste en Unicode). */
const FONT_ALIASES: [string, string, string][] = [['JetBrainsMono-Regular.ttf', 'JetBrainsMono', 'italic'], ['JetBrainsMono-Bold.ttf', 'JetBrainsMono', 'bolditalic']];
let fontsP: Promise<boolean> | null = null;
const b64 = (u: Uint8Array) => { let s = ''; for (let i = 0; i < u.length; i += 0x8000) s += String.fromCharCode(...u.subarray(i, i + 0x8000)); return btoa(s); };
/** Charge une fois (à la première impression) les polices incorporées ; chaque document créé
 * ensuite les reçoit. En cas d'échec, repli silencieux sur les polices standard. */
export function pdfFonts(): Promise<boolean> {
  fontsP ??= (async () => {
    try {
      const { jsPDF } = await import('jspdf');
      const files = await Promise.all(FONT_FILES.map(async ([file, family, style, url]) => {
        const r = await fetch(url); if (!r.ok) throw new Error(`${file} : ${r.status}`);
        return [file, family, style, b64(new Uint8Array(await r.arrayBuffer()))] as const;
      }));
      (jsPDF.API as any).events.push(['addFonts', function (this: any) { for (const [file, family, style, data] of files) { this.addFileToVFS(file, data); this.addFont(file, family, style); } for (const [file, family, style] of FONT_ALIASES) this.addFont(file, family, style); }]);
      Object.assign(FONT, { sans: 'Inter', mono: 'JetBrainsMono', unicode: true });
      return true;
    } catch (e) {
      console.warn('Polices PDF indisponibles, repli sur Helvetica :', e);
      fontsP = null;
      return false;
    }
  })();
  return fontsP;
}
/** Texte imprimable : avec les polices standard (repli), les flèches sont remplacées. */
export const txt = (s: unknown) => {
  const t = String(s ?? '').replace(/[  ]/g, ' ');
  return FONT.unicode ? t : t.replace(/→/g, '»').replace(/←/g, '«').replace(/↑/g, '^').replace(/↓/g, 'v').replace(/[↔⇄]/g, '<>').replace(/[↕↗]/g, '»').replace(/✓/g, 'x').replace(/[□▢]/g, '[ ]');
};

// ----- Couleurs -----
export const INK: [number, number, number] = [21, 23, 26];
/** Gris foncé pour les petits textes (< 8 pt) : au moins aussi foncé que #333. */
export const DARK: [number, number, number] = [51, 51, 51];
/** Gris moyen réservé aux textes de 8 pt et plus. */
export const MUTE: [number, number, number] = [91, 99, 109];
/** Couleur d'un texte selon sa taille : jamais de gris clair sous 8 pt. */
export const ink = (doc: Doc, size: number, muted = true) => { doc.setFontSize(size); doc.setTextColor(...(!muted ? INK : size < 8 ? DARK : MUTE)); };

// ----- Site, cartouches -----

export interface Zone { x: number; y: number; w: number; h: number; kind: 'text' | 'variable' | 'logo'; value: string; label: string | null; size: number; bold: boolean }
export interface TitleBlock { key: string; name: string; paper: string; landscape: boolean; width: number; height: number; zones: Zone[]; active: boolean; builtin: boolean }
export interface LabelSheet { key: string; name: string; reference: string; brand: string | null; paper: PaperKey; margin_top: number; margin_left: number; cols: number; rows: number; gap_x: number; gap_y: number; label_w: number; label_h: number; print_h: number; diam_min: number | null; diam_max: number | null; active: boolean; builtin: boolean }
export type Vars = Partial<Record<'title' | 'project' | 'reference' | 'client' | 'address' | 'date' | 'sheet' | 'sheets' | 'author' | 'checker' | 'version' | 'standards' | 'notes' | 'site' | 'view' | 'format' | 'logo', string>>;

let tbCache: TitleBlock[] | null = null;
let siteCache: { name: string; logo: string | null } | null = null;
export async function titleBlocks(force = false): Promise<TitleBlock[]> {
  if (!tbCache || force) tbCache = (await api.get<{ items: TitleBlock[] }>('/print/title-blocks')).items;
  return tbCache;
}
export function forgetTitleBlocks() { tbCache = null; siteCache = null; }
export async function siteInfo(): Promise<{ name: string; logo: string | null }> {
  if (!siteCache) siteCache = await api.get('/site');
  return siteCache!;
}
/** Cartouche choisi, sinon le premier actif. */
export async function titleBlock(key?: string | null): Promise<TitleBlock | null> {
  const list = await titleBlocks();
  return list.find((t) => t.key === key && t.active) ?? list.find((t) => t.active) ?? null;
}

// ----- Documents et pages -----

/** Pages portant leur propre cadre et cartouche : le pied de page générique les saute. */
const framed = new WeakMap<Doc, Set<number>>();
export const markFramed = (doc: Doc, page = doc.getNumberOfPages()) => { const s = framed.get(doc) ?? new Set<number>(); s.add(page); framed.set(doc, s); };

/** Nouveau document en mm : polices incorporées chargées, identité du site en cache. */
export async function newDocMm(w: number, h: number): Promise<Doc> {
  // svg2pdf greffe doc.svg à la création du document : il se charge avant.
  await import('svg2pdf.js');
  await pdfFonts();
  try { await siteInfo(); } catch { /* en-tête sans logo */ }
  const { jsPDF } = await import('jspdf');
  const doc = new jsPDF({ unit: 'mm', format: [w, h], orientation: w > h ? 'landscape' : 'portrait' });
  doc.setFont(FONT.sans, 'normal');
  return doc;
}
export async function newDoc(paper: PaperKey, landscape: boolean): Promise<Doc> {
  const [w, h] = pageMm(paper, landscape);
  return newDocMm(w, h);
}
export function addPage(doc: Doc, paper: PaperKey, landscape: boolean) {
  const [w, h] = pageMm(paper, landscape);
  doc.addPage([w, h], landscape ? 'landscape' : 'portrait');
}
export const size = (doc: Doc): [number, number] => [doc.internal.pageSize.getWidth(), doc.internal.pageSize.getHeight()];

/** Variables communes d'un projet pour les cartouches et en-têtes. */
export async function projectVars(p: { name: string; client: string | null; venue: string | null; reference?: string | null }, extra: Vars = {}): Promise<Vars> {
  const s = await siteInfo();
  return {
    project: p.name, client: p.client ?? 'Projet interne', address: p.venue ?? '', reference: p.reference ?? '',
    date: new Date().toLocaleDateString('fr-FR'), author: session.me?.account.name ?? '', site: s.name, logo: s.logo ?? '', ...extra,
  };
}

/** Image (logo) ajustée dans un rectangle, alignée à gauche ou à droite ; renvoie sa largeur. */
export function drawLogo(doc: Doc, logo: string | null | undefined, x: number, y: number, maxW: number, maxH: number, align: 'left' | 'right' | 'center' = 'left'): number {
  if (!logo) return 0;
  try {
    const p = doc.getImageProperties(logo); const s = Math.min(maxW / p.width, maxH / p.height); const w = p.width * s, h = p.height * s;
    const fmt = logo.startsWith('data:image/png') ? 'PNG' : logo.startsWith('data:image/webp') ? 'WEBP' : 'JPEG';
    doc.addImage(logo, fmt, align === 'right' ? x + maxW - w : align === 'center' ? x + (maxW - w) / 2 : x, y + (maxH - h) / 2, w, h);
    return w;
  } catch { return 0; /* logo illisible */ }
}

/** Cartouche en bas à droite de la page courante, d'après un modèle. Renvoie sa hauteur. */
export function drawTitleBlock(doc: Doc, tpl: TitleBlock | null, v: Vars, margin = 10): number {
  if (!tpl || !tpl.width || !tpl.height) return 0;
  const [W, H] = size(doc);
  const bw = Math.min(tpl.width, W - 2 * margin), k = bw / tpl.width, bh = tpl.height * k;
  const bx = W - margin - bw, by = H - margin - bh;
  doc.setFillColor(255, 255, 255); doc.setDrawColor(...INK); doc.setLineWidth(0.4); doc.rect(bx, by, bw, bh, 'FD');
  for (const z of tpl.zones) {
    const x = bx + z.x * k, y = by + z.y * k, w = z.w * k, h = z.h * k;
    doc.setLineWidth(0.2); doc.rect(x, y, w, h);
    if (z.kind === 'logo' || (z.kind === 'variable' && z.value === 'logo')) { drawLogo(doc, v.logo, x + 1, y + 1, w - 2, h - 2, 'center'); continue; }
    if (z.label) { doc.setFont(FONT.sans, 'normal'); ink(doc, 6.5); doc.text(txt(z.label).toUpperCase(), x + 1.4, y + 2.9); }
    const value = z.kind === 'text' ? z.value : z.value === 'sheet' ? (v.sheet ? `${v.sheet}${v.sheets ? ` / ${v.sheets}` : ''}` : '') : (v as any)[z.value] ?? '';
    doc.setFont(FONT.sans, z.bold ? 'bold' : 'normal'); doc.setFontSize(Math.max(6.5, z.size * k)); doc.setTextColor(...INK);
    const line = doc.splitTextToSize(txt(value || '—'), w - 2.8)[0] ?? '';
    doc.text(line, x + 1.4, y + h - Math.max(1.6, h * 0.22));
  }
  doc.setTextColor(...INK);
  return bh;
}

// ----- Familles de signal : style de trait lisible en noir et blanc -----

/** Motif de trait par famille (multiples de l'épaisseur du trait) ; [] = trait plein. */
const FAMILY_DASH: Record<string, number[]> = {
  V: [], A: [6, 3], N: [1, 2], S: [6, 2, 1, 2], C: [6, 2, 1, 2, 1, 2], P: [12, 3], F: [12, 3, 3, 3], L: [3, 3], I: [1, 2, 1, 2, 5, 2], R: [9, 2, 1, 2], X: [2, 4],
};
const EXTRA_DASH = [[4, 2], [8, 2, 2, 2], [2, 2, 6, 2], [10, 5]];
export function familyDash(key: string | null | undefined): number[] {
  if (!key) return [2, 4];
  if (FAMILY_DASH[key]) return FAMILY_DASH[key]!;
  let h = 0; for (const c of key) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return EXTRA_DASH[h % EXTRA_DASH.length]!;
}
export interface LegendItem { label: string; color: string; dash: number[] }

const hexRgb = (c: string): [number, number, number] => { const m = /^#?([0-9a-f]{6})$/i.exec(c.trim()); if (!m) return [...INK]; const n = parseInt(m[1]!, 16); return [n >> 16, (n >> 8) & 255, n & 255]; };

/** Légende des familles dans un rectangle ; renvoie la hauteur utilisée. */
function drawLegend(doc: Doc, items: LegendItem[], x: number, y: number, w: number): number {
  if (!items.length) return 0;
  doc.setFont(FONT.sans, 'bold'); ink(doc, 8, false); doc.text('Familles de signal', x, y + 3);
  const colW = 46, cols = Math.max(1, Math.floor(w / colW));
  doc.setFont(FONT.sans, 'normal');
  items.forEach((it, i) => {
    const cx = x + (i % cols) * colW, cy = y + 8 + Math.floor(i / cols) * 5;
    doc.setDrawColor(...hexRgb(it.color)); doc.setLineWidth(0.6);
    doc.setLineDashPattern(it.dash.map((d) => d * 0.6), 0); doc.line(cx, cy - 1, cx + 12, cy - 1); doc.setLineDashPattern([], 0);
    ink(doc, 8, false); doc.text(txt(it.label), cx + 14, cy);
  });
  doc.setDrawColor(...INK);
  return 8 + Math.ceil(items.length / cols) * 5;
}

// ----- Pages graphiques -----

/** Plus petite taille de texte d'un SVG (unités du dessin), textes décoratifs exclus. */
function minFontSize(svg: SVGSVGElement): number {
  let min = Infinity;
  for (const t of svg.querySelectorAll('text')) {
    if (!t.textContent?.trim()) continue;
    const raw = t.getAttribute('font-size') ?? /font-size:\s*([\d.]+)/.exec(t.getAttribute('style') ?? '')?.[1] ?? (t.closest('[font-size]') as Element | null)?.getAttribute('font-size');
    const v = parseFloat(raw ?? ''); if (v > 0 && v < min) min = v;
  }
  return Number.isFinite(min) ? min : 12;
}

export interface GraphicOpts {
  /** Découpe en tuiles quand l'ajustement ferait passer le texte sous `minPt` (défaut : 6,5 pt). */
  tile?: boolean; minPt?: number; legend?: LegendItem[];
}
/** Rectangles occupés par le dessin (unités du dessin, origine au coin du viewBox), mesurés
 * hors écran ; le fond qui couvre tout est ignoré. Sert à ne pas imprimer de tuile vide. */
function contentBoxes(svg: SVGSVGElement, w: number, h: number): [number, number, number, number][] | null {
  const host = document.createElement('div');
  host.style.cssText = 'position:absolute;left:-200000px;top:0;visibility:hidden';
  const s = svg.cloneNode(true) as SVGSVGElement;
  s.setAttribute('width', String(w)); s.setAttribute('height', String(h));
  host.appendChild(s); document.body.appendChild(host);
  try {
    const r0 = s.getBoundingClientRect(); if (!r0.width) return null;
    const kx = w / r0.width, ky = h / r0.height, out: [number, number, number, number][] = [];
    for (const el of s.querySelectorAll('text, rect, path, circle, line, polyline, polygon, image, ellipse')) {
      const r = el.getBoundingClientRect(); if (!r.width && !r.height) continue;
      const b: [number, number, number, number] = [(r.left - r0.left) * kx, (r.top - r0.top) * ky, (r.right - r0.left) * kx, (r.bottom - r0.top) * ky];
      if ((b[2] - b[0]) * (b[3] - b[1]) > 0.6 * w * h) continue;
      out.push(b);
    }
    return out;
  } catch { return null; } finally { host.remove(); }
}

const GM = 10, GPAD = 4, OV = 8;
interface Plan { W: number; H: number; ax: number; ay: number; aw: number; ah: number; band: number; lx: number; lw: number; ly: number; iy: number; tbW: number; k: number; nx: number; ny: number }
function plan(W: number, H: number, w: number, h: number, fs: number, tpl: TitleBlock | null, o: GraphicOpts): Plan {
  const m = GM, pad = GPAD, ov = OV;
  const tbW = tpl ? Math.min(tpl.width, W - 2 * m) : 0, tbH = tpl ? tpl.height * (tbW / tpl.width) : 0;
  const nl = o.legend?.length ?? 0;
  // Bandeau bas : repères de tuiles, légende, cartouche contre le cadre ; la légende passe
  // sur sa propre rangée, au-dessus, quand la place manque à côté du cartouche.
  const base = Math.max(tbH, o.tile ? 16 : 0);
  let lx = m + pad + (o.tile ? 68 : 0), lw = W - m - tbW - 4 - lx;
  const stacked = nl > 0 && lw < 50;
  if (stacked) { lx = m + pad; lw = W - 2 * (m + pad); }
  const legendH = nl ? 8 + Math.ceil(nl / Math.max(1, Math.floor(lw / 46))) * 5 : 0;
  const band = stacked ? base + legendH + 2 : Math.max(base, legendH + 2);
  const ly = H - m - band + 1, iy = H - m - (stacked ? base : band) + 1;
  const ax = m + pad, ay = m + pad, aw = W - 2 * (m + pad), ah = H - 2 * m - pad - band - 3;
  const toPt = 72 / 25.4, minPt = o.minPt ?? 6.5;
  const kFit = Math.min(aw / w, ah / h);
  let k = kFit, nx = 1, ny = 1;
  if (o.tile && fs * kFit * toPt < minPt) {
    const kNeed = minPt / (fs * toPt);
    nx = Math.max(1, Math.ceil((w * kNeed - ov) / (aw - ov))); ny = Math.max(1, Math.ceil((h * kNeed - ov) / (ah - ov)));
    while (nx * ny > 48) { if (nx >= ny) nx--; else ny--; }
    // L'échelle remplit la grille de tuiles sur l'axe le plus contraint.
    k = Math.min((nx * (aw - ov) + ov) / w, (ny * (ah - ov) + ov) / h);
    if (nx * ny === 1) k = kFit;
  }
  return { W, H, ax, ay, aw, ah, band, lx, lw, ly, iy, tbW, k, nx, ny };
}
/** Orientation qui demande le moins de feuilles pour un dessin découpé (à égalité : `landscape`). */
export function bestLandscape(svg: SVGSVGElement, w: number, h: number, paper: PaperKey, landscape: boolean, tpl: TitleBlock | null, o: GraphicOpts): boolean {
  if (!o.tile) return landscape;
  const fs = minFontSize(svg), tb = tpl && tpl.width && tpl.height ? tpl : null;
  const n = (l: boolean) => { const [W, H] = pageMm(paper, l); const p = plan(W, H, w, h, fs, tb, o); return p.nx * p.ny; };
  const a = n(landscape), b = n(!landscape);
  return b < a ? !landscape : landscape;
}

/** Une ou plusieurs pages graphiques : cadre à 10 mm du bord, dessin ajusté sur la surface
 * utile, cartouche contre le cadre, légende des familles à sa gauche. Quand le texte du dessin
 * tomberait sous `minPt`, le dessin est découpé en tuiles (recouvrement de 8 mm, repères
 * « feuille i/n » et plan d'assemblage) au lieu d'être écrasé ; les tuiles vides ne sont pas
 * imprimées. Renvoie le nombre de pages. */
export async function svgPage(doc: Doc, svg: SVGSVGElement, w: number, h: number, tpl: TitleBlock | null, v: Vars, o: GraphicOpts = {}): Promise<number> {
  await import('svg2pdf.js');
  const [W, H] = size(doc), m = GM, ov = OV;
  const tb = tpl && tpl.width && tpl.height ? tpl : null;
  const legend = o.legend ?? [];
  const { ax, ay, aw, ah, lx, lw, ly, iy, k, nx, ny } = plan(W, H, w, h, minFontSize(svg), tb, o);
  const dw = w * k, dh = h * k, all = nx * ny;
  const origin = (t: number): [number, number] => {
    const cx = t % nx, cy = Math.floor(t / nx);
    // Une seule feuille : dessin centré ; tuiles : décalage d'une tuile moins le recouvrement.
    return [all === 1 ? ax + (aw - dw) / 2 : ax - cx * (aw - ov) + (nx === 1 ? (aw - dw) / 2 : 0),
      all === 1 ? ay + (ah - dh) / 2 : ay - cy * (ah - ov) + (ny === 1 ? (ah - dh) / 2 : 0)];
  };
  // Tuiles retenues : celles où le dessin a quelque chose (toutes si la mesure échoue).
  let keep = Array.from({ length: all }, (_, t) => t);
  if (all > 1) {
    const boxes = contentBoxes(svg, w, h);
    if (boxes) {
      const kept = keep.filter((t) => { const [ox, oy] = origin(t); return boxes.some(([x0, y0, x1, y1]) => ox + x1 * k > ax + 1 && ox + x0 * k < ax + aw - 1 && oy + y1 * k > ay + 1 && oy + y0 * k < ay + ah - 1); });
      if (kept.length) keep = kept;
    }
  }
  const n = keep.length, num = (t: number) => keep.indexOf(t) + 1;
  for (const [i, t] of keep.entries()) {
    if (i > 0) doc.addPage([W, H], W > H ? 'landscape' : 'portrait');
    const cx = t % nx, cy = Math.floor(t / nx);
    const [ox, oy] = origin(t);
    doc.saveGraphicsState();
    doc.rect(ax, ay, aw, ah, null); (doc as any).clip(); (doc as any).discardPath();
    await doc.svg(svg, { x: ox, y: oy, width: dw, height: dh });
    doc.restoreGraphicsState();
    doc.setDrawColor(...INK); doc.setLineWidth(0.35); doc.setLineDashPattern([], 0); doc.rect(m, m, W - 2 * m, H - 2 * m);
    if (all > 1) {
      // Repères de recouvrement et de raccord avec les feuilles voisines (imprimées).
      const left = cx > 0 && num(t - 1), right = cx < nx - 1 && num(t + 1), up = cy > 0 && num(t - nx), down = cy < ny - 1 && num(t + nx);
      doc.setDrawColor(...MUTE); doc.setLineWidth(0.2); doc.setLineDashPattern([1.5, 1.5], 0);
      if (left) doc.line(ax + ov, ay, ax + ov, ay + ah);
      if (right) doc.line(ax + aw - ov, ay, ax + aw - ov, ay + ah);
      if (up) doc.line(ax, ay + ov, ax + aw, ay + ov);
      if (down) doc.line(ax, ay + ah - ov, ax + aw, ay + ah - ov);
      doc.setLineDashPattern([], 0);
      doc.setFont(FONT.sans, 'bold'); ink(doc, 8, false);
      const lab = (j: number) => `feuille ${j}/${n}`;
      if (left) doc.text(txt(`← ${lab(left)}`), ax + 1, ay + ah / 2, { angle: 90 });
      if (right) doc.text(txt(`${lab(right)} →`), ax + aw - 1, ay + ah / 2, { angle: 90 });
      if (up) doc.text(txt(`↑ ${lab(up)}`), ax + aw / 2, ay + 3.5, { align: 'center' });
      if (down) doc.text(txt(`↓ ${lab(down)}`), ax + aw / 2, ay + ah - 1.5, { align: 'center' });
      // Plan d'assemblage : la grille des feuilles, la courante pleine, les vides hachurées.
      const by = iy, cell = Math.min(5, 13 / ny, 30 / nx);
      doc.setFont(FONT.sans, 'bold'); ink(doc, 9, false); doc.text(txt(`Feuille ${i + 1}/${n}`), ax, by + 3.5);
      doc.setFont(FONT.sans, 'normal'); ink(doc, 8); doc.text(txt(`plan ${nx} × ${ny}`), ax, by + 7.5); doc.text(txt(`recouvrement ${ov} mm`), ax, by + 11.5);
      for (let j = 0; j < all; j++) {
        const gx = ax + 32 + (j % nx) * cell, gy = by + Math.floor(j / nx) * cell, on = keep.includes(j);
        doc.setDrawColor(...(on ? INK : [170, 175, 182] as [number, number, number])); doc.setLineWidth(0.2); doc.setFillColor(...(j === t ? INK : [255, 255, 255] as [number, number, number])); doc.rect(gx, gy, cell, cell, 'FD');
        if (!on) doc.line(gx, gy + cell, gx + cell, gy);
      }
    }
    if (legend.length) drawLegend(doc, legend, lx, ly, lw);
    drawTitleBlock(doc, tb, n > 1 ? { ...v, sheet: String(i + 1), sheets: String(n), view: `${v.view ?? v.title ?? ''} · feuille ${i + 1}/${n}` } : v, m);
    markFramed(doc);
  }
  doc.setTextColor(...INK);
  return n;
}

// ----- En-tête d'identité et tableaux -----

export interface Header { title: string; project: string; client: string; venue: string; dates: string; version?: string }
export function projectHeader(p: { name: string; client: string | null; venue: string | null; shoot_start?: string | null; shoot_end?: string | null; commissioning?: string | null; rev?: number }, title: string): Header {
  const f = (d?: string | null) => (d ? new Date(d).toLocaleDateString('fr-FR') : '');
  return {
    title, project: p.name, client: p.client ?? 'Projet interne', venue: p.venue ?? '',
    dates: p.shoot_start ? `Exploitation ${f(p.shoot_start)} au ${f(p.shoot_end ?? p.shoot_start)}` : p.commissioning ? `Exploitation ${f(p.commissioning)}` : '',
    version: p.rev != null ? `rév. ${p.rev}` : '',
  };
}

export const MARGIN = 12;
/** Bloc d'identité commun à tous les exports de listes : logo et nom du site, titre du
 * document, projet, client, lieu, date d'édition et version. Renvoie l'ordonnée dessous. */
export function drawHeader(doc: Doc, h: Header, opts: { cont?: boolean } = {}): number {
  const [W] = size(doc), m = MARGIN, top = 10;
  const site = siteCache;
  const lw = drawLogo(doc, site?.logo, m, top, 34, 11);
  const tx = m + (lw ? lw + 4 : 0);
  doc.setFont(FONT.sans, 'bold'); ink(doc, 8); doc.text(txt((site?.name ?? 'Strata Arkyo').toUpperCase()), tx, top + 3);
  doc.setFont(FONT.sans, 'bold'); ink(doc, 14, false); doc.text(txt(`${h.title}${opts.cont ? ' (suite)' : ''}`), tx, top + 9.5);
  doc.setFont(FONT.sans, 'normal'); ink(doc, 8.5);
  doc.text(txt([h.project, h.client, h.venue].filter(Boolean).join(' · ')), tx, top + 14.5);
  const right = [`Édité le ${new Date().toLocaleDateString('fr-FR')}${h.version ? ` · ${h.version}` : ''}`, h.dates].filter(Boolean);
  right.forEach((l, i) => doc.text(txt(l), W - m, top + 3 + i * 4.5, { align: 'right' }));
  doc.setDrawColor(...INK); doc.setLineWidth(0.4); doc.line(m, top + 17, W - m, top + 17);
  doc.setTextColor(...INK);
  return top + 23;
}

export interface Col { label: string; w: number; align?: 'left' | 'right' }
const GUTTER = 2.5, BODY = 8.5, HEAD = 7.5, LINE = 3.8, ROWPAD = 2.4;
/** Largeurs selon le contenu : chaque colonne reçoit au moins son en-tête et son mot le plus
 * long ; le reste va aux colonnes qui en ont besoin (indication `w` en arbitrage). */
function widths(doc: Doc, cols: Col[], rows: string[][], avail: number): number[] {
  const tw = (s: string) => doc.getTextWidth(s);
  doc.setFont(FONT.sans, 'bold'); doc.setFontSize(HEAD);
  const head = cols.map((c) => tw(txt(c.label).toUpperCase()));
  doc.setFont(FONT.sans, 'normal'); doc.setFontSize(BODY);
  const nat = cols.map((_, i) => head[i]!), word = cols.map((_, i) => head[i]!);
  for (const r of rows.slice(0, 2000)) r.forEach((c, i) => {
    if (i >= cols.length) return;
    const s = txt(c); for (const line of s.split('\n')) nat[i] = Math.max(nat[i]!, tw(line));
    for (const wd of s.split(/\s+/)) if (wd.length > 3) word[i] = Math.max(word[i]!, Math.min(tw(wd), 45));
  });
  const minw = cols.map((_, i) => Math.max(word[i]!, 6) + GUTTER), want = cols.map((_, i) => nat[i]! + GUTTER);
  const sumMin = minw.reduce((a, b) => a + b, 0), sumWant = want.reduce((a, b) => a + b, 0);
  if (sumWant <= avail) {
    // Tout tient sur une ligne : la place restante va aux colonnes de texte, selon l'indication.
    const flex = cols.map((c, i) => (c.align === 'right' ? 0 : c.w * Math.max(1, nat[i]! / 10)));
    const tf = flex.reduce((a, b) => a + b, 0) || 1;
    return want.map((x, i) => x + ((avail - sumWant) * flex[i]!) / tf);
  }
  if (sumMin >= avail) return minw.map((x) => (x * avail) / sumMin);
  const need = want.map((x, i) => x - minw[i]!), tn = need.reduce((a, b) => a + b, 0) || 1;
  return minw.map((x, i) => x + ((avail - sumMin) * need[i]!) / tn);
}

export interface TableOpts {
  section?: string; groups?: { at: number; title: string }[];
  /** Ordonnée de départ sur la page courante ; sinon nouvel en-tête en haut de page. */
  y?: number;
  /** Appelé avec le numéro de la page où commence le tableau (sommaire). */
  onStart?: (page: number) => void;
}
/** Tableau paginé à partir de la page courante ; chaque nouvelle page reprend l'en-tête
 * d'identité, le titre de section et l'en-tête de colonnes. Rien n'est tronqué. Renvoie
 * l'ordonnée sous le tableau. */
export function drawTable(doc: Doc, h: Header, cols: Col[], rows: string[][], paper: PaperKey, landscape: boolean, opts: TableOpts = {}): number {
  const [W, H] = size(doc), m = MARGIN, bottom = H - 16;
  const ws = widths(doc, cols, rows, W - 2 * m);
  let y = opts.y ?? drawHeader(doc, h);
  const head = () => {
    doc.setFont(FONT.sans, 'bold'); ink(doc, HEAD);
    let x = m; cols.forEach((c, i) => { doc.text(txt(c.label).toUpperCase(), c.align === 'right' ? x + ws[i]! - GUTTER : x, y, { align: c.align === 'right' ? 'right' : 'left' }); x += ws[i]!; });
    doc.setDrawColor(...INK); doc.setLineWidth(0.3); doc.line(m, y + 1.6, W - m, y + 1.6); y += 6;
    doc.setFont(FONT.sans, 'normal'); doc.setFontSize(BODY); doc.setTextColor(...INK);
  };
  const title = (cont: boolean) => { if (!opts.section) return; doc.setFont(FONT.sans, 'bold'); ink(doc, 11, false); doc.text(txt(`${opts.section}${cont ? ' (suite)' : ''}`), m, y); y += 6.5; };
  const newPage = () => { addPage(doc, paper, landscape); y = drawHeader(doc, h, { cont: !opts.section }); title(true); head(); };
  doc.setFont(FONT.sans, 'normal'); doc.setFontSize(BODY);
  const cells = rows.map((r) => r.slice(0, cols.length).map((c, i) => doc.splitTextToSize(txt(c), ws[i]! - GUTTER) as string[]));
  const rowH = (ri: number) => Math.max(1, ...cells[ri]!.map((c) => c.length)) * LINE + ROWPAD;
  // Pas de titre orphelin : titre, en-tête de colonnes et deux lignes au moins tiennent ensemble.
  const lead = (opts.section ? 6.5 : 0) + 6 + (rows.length ? rowH(0) + (rows.length > 1 ? rowH(1) : 0) : 6);
  if (y + lead > bottom && opts.y != null) { addPage(doc, paper, landscape); y = drawHeader(doc, h); }
  opts.onStart?.(doc.getNumberOfPages());
  title(false); head();
  const groups = new Map((opts.groups ?? []).map((g) => [g.at, g.title]));
  cells.forEach((c, ri) => {
    const g = groups.get(ri), lh = rowH(ri);
    if (y + lh + (g ? 7 + (rowH(ri + 1 < rows.length ? ri + 1 : ri)) : 0) - LINE > bottom) newPage();
    if (g) { doc.setFont(FONT.sans, 'bold'); ink(doc, 9.5, false); doc.text(txt(g), m, y + 1); y += 6; doc.setFont(FONT.sans, 'normal'); doc.setFontSize(BODY); }
    let x = m;
    c.forEach((lines, i) => { doc.text(lines, cols[i]!.align === 'right' ? x + ws[i]! - GUTTER : x, y, { align: cols[i]!.align === 'right' ? 'right' : 'left', lineHeightFactor: LINE / (BODY * 25.4 / 72) }); x += ws[i]!; });
    // Filet à mi-hauteur entre le bas de cette ligne et le haut de la suivante.
    const lastBase = y + (Math.max(1, ...c.map((l) => l.length)) - 1) * LINE;
    y += lh;
    if (ri < cells.length - 1) { doc.setDrawColor(205, 209, 214); doc.setLineWidth(0.2); doc.line(m, (lastBase + 0.8 + y - 2.2) / 2, W - m, (lastBase + 0.8 + y - 2.2) / 2); }
  });
  if (!rows.length) { ink(doc, 8.5); doc.text('Aucune ligne.', m, y + 2); y += 6; }
  doc.setTextColor(...INK);
  return y;
}

/** Pied de page sur les pages [from, to] : « n / N » et date (pagination continue), à 10 mm
 * du bas ; les pages à cartouche (qui portent déjà leur numéro) ne le reçoivent pas. */
export function paginate(doc: Doc, from = 1, to?: number, label = '') {
  const n = doc.getNumberOfPages(), last = to ?? n, date = new Date().toLocaleDateString('fr-FR');
  const skip = framed.get(doc) ?? new Set<number>();
  for (let i = from; i <= last; i++) {
    if (skip.has(i)) continue;
    doc.setPage(i);
    const [W, H] = size(doc);
    doc.setFont(FONT.sans, 'normal'); ink(doc, 7.5);
    doc.text(txt(`${label}${label ? ' · ' : ''}${date}`), MARGIN, H - 10);
    doc.text(`${i - from + 1} / ${last - from + 1}`, W - MARGIN, H - 10, { align: 'right' });
  }
  doc.setTextColor(...INK);
}

/** Nom de fichier à préfixe stable : {Préfixe}_{Projet}_{AAAA-MM-JJ}.{ext} */
export function fileStem(prefix: string, project: string): string {
  const safe = project.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^A-Za-z0-9-]+/g, '_').replace(/^_|_$/g, '');
  return `${prefix}_${safe}_${new Date().toLocaleDateString('sv-SE')}`;
}
