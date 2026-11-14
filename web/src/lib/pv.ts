// Procès-verbal de recette (section 11.5) : réutilisé par l'écran Recette et par le dossier.
// Un tableau par section : statut, point (liaison avec ses deux extrémités), colonne
// Observations vide et case à cocher pour le contrôle sur papier ; rappel du projet en tête
// des pages de suite ; signatures jamais coupées.
import { DARK, FONT, INK, MUTE, drawLogo, ink, txt, type Doc } from './pdfkit';

type St = 'to_test' | 'ok' | 'reserve' | 'ko';
export interface PvPoint { key: string; label: string; detail: string; deleted: boolean; status: St; comment: string | null; by: string | null; at: string | null }
export interface PvGroup { title: string; points: PvPoint[] }
export interface PvData { progress: number; total: number; counts: Record<St, number>; cables: PvGroup[]; equipment: PvGroup[]; free: PvPoint[]; deleted: PvPoint[]; reserves: PvPoint[] }
const ST: Record<St, { label: string }> = { to_test: { label: 'À tester' }, ok: { label: 'OK' }, reserve: { label: 'Réserve' }, ko: { label: 'KO' } };
/** Extrémités d'une liaison (liste des câbles), par identifiant de câble. */
export interface PvEnd { element: string; equipment: string; tag: string; port: string }
export type PvEnds = Map<string, { a: PvEnd; b: PvEnd; signal?: string }>;

/** Extrémités lisibles d'une liaison : « élément › équipement repère › port ». */
const endLabel = (e: PvEnd) => [e.element, `${e.equipment} ${e.tag}`.trim(), e.port].filter(Boolean).join(' › ');
export async function pvEnds(pid: string): Promise<PvEnds> {
  const { api } = await import('./api');
  try {
    const t = await api.get<{ rows: { id: string; signal: string; a: PvEnd; b: PvEnd }[] }>(`/projects/${pid}/cables/table`);
    return new Map(t.rows.map((r) => [r.id, { a: r.a, b: r.b, signal: r.signal }]));
  } catch { return new Map(); }
}

/** Écrit le procès-verbal à partir de la page courante du document (vierge). */
export function drawPV(doc: Doc, r: PvData, view: { project: { name: string; client: string | null; venue: string | null } }, site: string, logo: string | null, paper: 'A4' | 'A3' = 'A4', ends: PvEnds = new Map()) {
  const fmt: [number, number] = paper === 'A3' ? [297, 420] : [210, 297];
  const W = doc.internal.pageSize.getWidth(), H = doc.internal.pageSize.getHeight(), m = 14, bottom = H - 18;
  let y = m;
  // Colonnes : statut, point, observations (vide, à remplir), case de contrôle.
  const cSt = m, cPt = m + 21, cBox = W - m - 7, cObs = cBox - 52, ptW = cObs - cPt - 3;
  const runHead = () => {
    doc.setFont(FONT.sans, 'bold'); ink(doc, 8, false); doc.text(txt(`PV de recette · ${view.project.name}`), m, m + 2);
    doc.setFont(FONT.sans, 'normal'); ink(doc, 8); doc.text(txt(`${site} · édité le ${new Date().toLocaleDateString('fr-FR')}`), W - m, m + 2, { align: 'right' });
    doc.setDrawColor(...INK); doc.setLineWidth(0.3); doc.line(m, m + 4, W - m, m + 4);
    y = m + 9;
  };
  const colHead = () => {
    doc.setFont(FONT.sans, 'bold'); ink(doc, 7.5);
    doc.text('STATUT', cSt, y + 3); doc.text('POINT CONTRÔLÉ', cPt, y + 3); doc.text('OBSERVATIONS', cObs + 1.5, y + 3); doc.text('CONTRÔLE', W - m, y + 3, { align: 'right' });
    doc.setDrawColor(...INK); doc.setLineWidth(0.3); doc.line(m, y + 4.6, W - m, y + 4.6);
    y += 5.6;
  };
  let inTable = false;
  const room = (h: number) => { if (y + h > bottom) { doc.addPage(fmt, 'portrait'); runHead(); if (inTable) colHead(); } };

  // En-tête de la première page.
  drawLogo(doc, logo, W - m - 50, y, 50, 14, 'right');
  doc.setFont(FONT.sans, 'bold'); ink(doc, 9); doc.text(txt(site.toUpperCase()), m, y + 4);
  ink(doc, 20, false); doc.text('Procès-verbal de recette', m, y + 14);
  doc.setFont(FONT.sans, 'normal'); doc.setFontSize(10.5);
  doc.text(txt(`${view.project.name}${view.project.client ? ` · ${view.project.client}` : ''}`), m, y + 21);
  doc.text(txt(`${view.project.venue ?? ''}${view.project.venue ? ' · ' : ''}édité le ${new Date().toLocaleDateString('fr-FR')}`), m, y + 26.5);
  y += 34;
  // Récapitulatif
  doc.setDrawColor(...INK); doc.setLineWidth(0.3); doc.rect(m, y, W - 2 * m, 16);
  const cells: [string, string][] = [['Avancement', `${r.progress} %`], ['Points', String(r.total)], ['OK', String(r.counts.ok)], ['Réserves', String(r.counts.reserve)], ['KO', String(r.counts.ko)], ['À tester', String(r.counts.to_test)]];
  const cw = (W - 2 * m) / cells.length;
  cells.forEach(([l, v], i) => { doc.setFont(FONT.sans, 'bold'); ink(doc, 8); doc.setTextColor(...DARK); doc.text(txt(l.toUpperCase()), m + i * cw + 3, y + 5.2); doc.setFontSize(13); doc.setTextColor(...INK); doc.text(v, m + i * cw + 3, y + 12.8); doc.setFont(FONT.sans, 'normal'); });
  y += 24;
  const color: Record<St, [number, number, number]> = { to_test: DARK, ok: [14, 120, 72], reserve: [150, 105, 0], ko: [190, 45, 35] };
  const line = (p: PvPoint) => {
    // Liaison : numéro, signal et les deux extrémités, d'après la liste des câbles.
    const id = p.key.startsWith('cable:') ? p.key.slice(6) : null, e = id ? ends.get(id) : undefined;
    const head = e ? `${p.label}${e.signal ? ` · ${e.signal}` : ''}` : p.label;
    const body = e ? [`De : ${endLabel(e.a)}`, `Vers : ${endLabel(e.b)}`] : p.detail ? [p.detail] : [];
    doc.setFont(FONT.sans, 'bold'); doc.setFontSize(9);
    const t1 = doc.splitTextToSize(txt(head), ptW) as string[];
    doc.setFont(FONT.sans, 'normal');
    const t2 = body.flatMap((b) => doc.splitTextToSize(txt(b), ptW) as string[]);
    const com = p.comment ? (doc.splitTextToSize(txt(p.comment), ptW) as string[]) : [];
    const lh = 4.2, h = Math.max(9, lh * (t1.length + t2.length + com.length) + 3.2);
    room(h);
    doc.setFont(FONT.sans, 'bold'); doc.setFontSize(8); doc.setTextColor(...color[p.status]); doc.text(txt(ST[p.status].label.toUpperCase()), cSt, y + 4);
    doc.setFontSize(9); doc.setTextColor(...INK); doc.text(t1, cPt, y + 4, { lineHeightFactor: lh / (9 * 25.4 / 72) });
    doc.setFont(FONT.sans, 'normal'); doc.text(t2, cPt, y + 4 + lh * t1.length, { lineHeightFactor: lh / (9 * 25.4 / 72) });
    if (com.length) { doc.setTextColor(...MUTE); doc.text(com, cPt, y + 4 + lh * (t1.length + t2.length), { lineHeightFactor: lh / (9 * 25.4 / 72) }); }
    // Observations (vide, filet de séparation) et case à cocher.
    doc.setDrawColor(190, 195, 201); doc.setLineWidth(0.2); doc.line(cObs, y + 1, cObs, y + h - 1);
    doc.setDrawColor(...INK); doc.setLineWidth(0.3); doc.rect(W - m - 4.5, y + 1.3, 3.8, 3.8);
    if (p.status === 'ok') { doc.setLineWidth(0.4); doc.line(W - m - 3.9, y + 3.3, W - m - 2.9, y + 4.4); doc.line(W - m - 2.9, y + 4.4, W - m - 1.2, y + 1.9); }
    y += h; doc.setDrawColor(205, 209, 214); doc.setLineWidth(0.2); doc.line(m, y, W - m, y);
  };
  const section = (title: string, groups: PvGroup[]) => {
    if (!groups.some((g) => g.points.length)) return;
    inTable = false;
    // Titre, en-tête de colonnes, sous-titre et deux points au moins ensemble.
    room(9 + 5.6 + 6 + 2 * 13); doc.setFont(FONT.sans, 'bold'); ink(doc, 13, false); doc.text(txt(title), m, y + 5); y += 8;
    colHead(); inTable = true;
    for (const g of groups) {
      if (!g.points.length) continue;
      if (g.title) { room(6 + 2 * 13); doc.setFontSize(9.5); doc.setFont(FONT.sans, 'bold'); doc.setTextColor(...MUTE); doc.text(txt(g.title), m, y + 4.5); y += 6.5; }
      for (const p of g.points) line(p);
      y += 2;
    }
    inTable = false; y += 3;
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
    doc.setDrawColor(...INK); doc.setLineWidth(0.3); doc.rect(x, y, bw, 40);
    doc.setFont(FONT.sans, 'bold'); ink(doc, 9, false); doc.text(t, x + 3, y + 6);
    doc.setFont(FONT.sans, 'normal'); ink(doc, 8); doc.text('Nom, date et signature', x + 3, y + 11);
  }
  doc.setTextColor(...INK);
}
