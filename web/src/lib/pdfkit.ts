// Boîte à outils PDF commune (sections 11.6 et 11.7) : en-tête projet, tableaux paginés,
// cartouches selon les modèles du site, pagination continue « n / N ». Polices standard :
// pas de flèche « → » (remplacée par « » »).

import type { jsPDF } from 'jspdf';
import { api } from './api';
import { session } from './session.svelte';

export type Doc = jsPDF;
export type PaperKey = 'A4' | 'A3' | 'A2' | 'A1' | 'A0' | 'Letter' | 'Legal';
export const PAPER_MM: Record<PaperKey, [number, number]> = {
  A4: [210, 297], A3: [297, 420], A2: [420, 594], A1: [594, 841], A0: [841, 1189], Letter: [215.9, 279.4], Legal: [215.9, 355.6],
};
export const pageMm = (p: PaperKey, landscape: boolean): [number, number] => { const [a, b] = PAPER_MM[p]; return landscape ? [b, a] : [a, b]; };

export interface Zone { x: number; y: number; w: number; h: number; kind: 'text' | 'variable' | 'logo'; value: string; label: string | null; size: number; bold: boolean }
export interface TitleBlock { key: string; name: string; paper: string; landscape: boolean; width: number; height: number; zones: Zone[]; active: boolean; builtin: boolean }
export interface LabelSheet { key: string; name: string; reference: string; brand: string | null; paper: PaperKey; margin_top: number; margin_left: number; cols: number; rows: number; gap_x: number; gap_y: number; label_w: number; label_h: number; print_h: number; diam_min: number | null; diam_max: number | null; active: boolean; builtin: boolean }
export type Vars = Partial<Record<'title' | 'project' | 'reference' | 'client' | 'address' | 'date' | 'sheet' | 'sheets' | 'author' | 'checker' | 'version' | 'standards' | 'notes' | 'site' | 'view' | 'format' | 'logo', string>>;

export const txt = (s: unknown) => String(s ?? '').replace(/→/g, '»').replace(/↔/g, '<>').replace(/[  ]/g, ' ');

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

export async function newDoc(paper: PaperKey, landscape: boolean): Promise<Doc> {
  // svg2pdf greffe doc.svg à la création du document : il se charge avant.
  await import('svg2pdf.js');
  const { jsPDF } = await import('jspdf');
  const [w, h] = pageMm(paper, landscape);
  return new jsPDF({ unit: 'mm', format: [w, h], orientation: landscape ? 'landscape' : 'portrait' });
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

/** Cartouche en bas à droite de la page courante, d'après un modèle. Renvoie sa hauteur. */
export function drawTitleBlock(doc: Doc, tpl: TitleBlock | null, v: Vars, margin = 10): number {
  if (!tpl || !tpl.width || !tpl.height) return 0;
  const [W, H] = size(doc);
  const bw = Math.min(tpl.width, W - 2 * margin), k = bw / tpl.width, bh = tpl.height * k;
  const bx = W - margin - bw, by = H - margin - bh;
  doc.setDrawColor(21, 23, 26); doc.setLineWidth(0.4); doc.rect(bx, by, bw, bh);
  for (const z of tpl.zones) {
    const x = bx + z.x * k, y = by + z.y * k, w = z.w * k, h = z.h * k;
    doc.setLineWidth(0.2); doc.rect(x, y, w, h);
    if (z.kind === 'logo' || (z.kind === 'variable' && z.value === 'logo')) {
      if (v.logo) { try { const fmt = v.logo.startsWith('data:image/png') ? 'PNG' : 'JPEG'; const p = doc.getImageProperties(v.logo); const s = Math.min((w - 2) / p.width, (h - 2) / p.height); doc.addImage(v.logo, fmt, x + (w - p.width * s) / 2, y + (h - p.height * s) / 2, p.width * s, p.height * s); } catch { /* logo illisible */ } }
      continue;
    }
    if (z.label) { doc.setFont('helvetica', 'normal'); doc.setFontSize(5.5); doc.setTextColor(91, 99, 109); doc.text(txt(z.label).toUpperCase(), x + 1.4, y + 2.6); }
    const value = z.kind === 'text' ? z.value : z.value === 'sheet' ? (v.sheet ? `${v.sheet}${v.sheets ? ` / ${v.sheets}` : ''}` : '') : (v as any)[z.value] ?? '';
    doc.setFont('helvetica', z.bold ? 'bold' : 'normal'); doc.setFontSize(z.size * k); doc.setTextColor(21, 23, 26);
    const line = doc.splitTextToSize(txt(value || '—'), w - 2.8)[0] ?? '';
    doc.text(line, x + 1.4, y + h - Math.max(1.6, h * 0.22));
  }
  doc.setTextColor(21, 23, 26);
  return bh;
}

/** Une page graphique : le SVG ajusté dans la page, cadre et cartouche. */
export async function svgPage(doc: Doc, svg: SVGSVGElement, w: number, h: number, tpl: TitleBlock | null, v: Vars) {
  await import('svg2pdf.js');
  const [W, H] = size(doc);
  const m = 10, bh = tpl && tpl.height ? Math.min(tpl.height, 60) + 4 : 0;
  const aw = W - 2 * m, ah = H - 2 * m - bh;
  const k = Math.min(aw / w, ah / h);
  await doc.svg(svg, { x: m + (aw - w * k) / 2, y: m + (ah - h * k) / 2, width: w * k, height: h * k });
  doc.setDrawColor(21, 23, 26); doc.setLineWidth(0.35); doc.rect(m / 2, m / 2, W - m, H - m);
  drawTitleBlock(doc, tpl, v, m);
}

export interface Header { title: string; project: string; client: string; venue: string; dates: string }
export function projectHeader(p: { name: string; client: string | null; venue: string | null; shoot_start?: string | null; shoot_end?: string | null; commissioning?: string | null }, title: string): Header {
  const f = (d?: string | null) => (d ? new Date(d).toLocaleDateString('fr-FR') : '');
  return {
    title, project: p.name, client: p.client ?? 'Projet interne', venue: p.venue ?? '',
    dates: p.shoot_start ? `Exploitation ${f(p.shoot_start)} au ${f(p.shoot_end ?? p.shoot_start)}` : `Exploitation ${f(p.commissioning)}`,
  };
}

/** En-tête de page de tableau ; renvoie l'ordonnée sous l'en-tête. */
export function drawHeader(doc: Doc, h: Header): number {
  const [W] = size(doc), m = 12;
  doc.setFont('helvetica', 'bold'); doc.setFontSize(14); doc.setTextColor(21, 23, 26); doc.text(txt(h.title), m, m + 4);
  doc.setFont('helvetica', 'normal'); doc.setFontSize(8.5); doc.setTextColor(91, 99, 109);
  doc.text(txt([h.project, h.client, h.venue].filter(Boolean).join(' · ')), m, m + 9.5);
  doc.text(txt(h.dates), W - m, m + 9.5, { align: 'right' });
  doc.setDrawColor(21, 23, 26); doc.setLineWidth(0.4); doc.line(m, m + 12, W - m, m + 12);
  doc.setTextColor(21, 23, 26);
  return m + 18;
}

export interface Col { label: string; w: number; align?: 'left' | 'right' }
/** Tableau paginé à partir de la page courante ; chaque nouvelle page reprend l'en-tête. */
export function drawTable(doc: Doc, h: Header, cols: Col[], rows: string[][], paper: PaperKey, landscape: boolean, opts: { section?: string; groups?: { at: number; title: string }[] } = {}) {
  const [W, H] = size(doc), m = 12;
  const total = cols.reduce((a, c) => a + c.w, 0), k = (W - 2 * m) / total;
  const ws = cols.map((c) => c.w * k);
  let y = drawHeader(doc, h);
  const head = () => {
    doc.setFont('helvetica', 'bold'); doc.setFontSize(7.5); doc.setTextColor(91, 99, 109);
    let x = m; cols.forEach((c, i) => { doc.text(txt(c.label).toUpperCase(), c.align === 'right' ? x + ws[i]! - 1 : x, y, { align: c.align === 'right' ? 'right' : 'left' }); x += ws[i]!; });
    doc.setDrawColor(21, 23, 26); doc.setLineWidth(0.3); doc.line(m, y + 1.6, W - m, y + 1.6); y += 6;
    doc.setFont('helvetica', 'normal'); doc.setFontSize(8.5); doc.setTextColor(21, 23, 26);
  };
  if (opts.section) { doc.setFont('helvetica', 'bold'); doc.setFontSize(11); doc.text(txt(opts.section), m, y); y += 6; }
  head();
  const groups = new Map((opts.groups ?? []).map((g) => [g.at, g.title]));
  rows.forEach((r, ri) => {
    const g = groups.get(ri);
    const cells = r.map((c, i) => doc.splitTextToSize(txt(c), ws[i]! - 2) as string[]);
    const lh = Math.max(1, ...cells.map((c) => Math.min(c.length, 3))) * 3.8 + 1.6;
    if (y + lh + (g ? 8 : 0) > H - m - 8) { addPage(doc, paper, landscape); y = drawHeader(doc, h); head(); }
    if (g) { doc.setFont('helvetica', 'bold'); doc.setFontSize(9.5); doc.text(txt(g), m, y + 1); y += 6; doc.setFont('helvetica', 'normal'); doc.setFontSize(8.5); }
    let x = m;
    cells.forEach((c, i) => { const lines = c.slice(0, 3); doc.text(lines, cols[i]!.align === 'right' ? x + ws[i]! - 1 : x, y, { align: cols[i]!.align === 'right' ? 'right' : 'left' }); x += ws[i]!; });
    y += lh; doc.setDrawColor(225, 228, 232); doc.setLineWidth(0.2); doc.line(m, y - 2.4, W - m, y - 2.4);
  });
  if (!rows.length) { doc.setTextColor(91, 99, 109); doc.text('Aucune ligne.', m, y + 2); doc.setTextColor(21, 23, 26); }
}

/** Pied de page sur les pages [from, to] : « n / N » et date (pagination continue). */
export function paginate(doc: Doc, from = 1, to?: number, label = '') {
  const n = doc.getNumberOfPages(), last = to ?? n, date = new Date().toLocaleDateString('fr-FR');
  for (let i = from; i <= last; i++) {
    doc.setPage(i);
    const [W, H] = size(doc);
    doc.setFont('helvetica', 'normal'); doc.setFontSize(7.5); doc.setTextColor(91, 99, 109);
    doc.text(txt(`${label}${label ? ' · ' : ''}${date}`), 12, H - 5);
    doc.text(`${i - from + 1} / ${last - from + 1}`, W - 12, H - 5, { align: 'right' });
  }
  doc.setTextColor(21, 23, 26);
}

/** Nom de fichier à préfixe stable : {Préfixe}_{Projet}_{AAAA-MM-JJ}.{ext} */
export function fileStem(prefix: string, project: string): string {
  const safe = project.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^A-Za-z0-9-]+/g, '_').replace(/^_|_$/g, '');
  return `${prefix}_${safe}_${new Date().toLocaleDateString('sv-SE')}`;
}
