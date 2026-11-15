// Procès-verbal de recette (section 11.5) : réutilisé par l'écran Recette et par le dossier.
// En-tête commun aux exports (projet, client, lieu, dates, révision). Un tableau par section :
// statut, point (liaison avec ses deux extrémités), colonne Observations vide et, pour les
// points non testés, trois cases « OK · R · KO » pour noter le verdict sur papier ; un point
// déjà jugé porte son verdict imprimé. Les liaisons dont une extrémité manque sont rangées à
// la fin, sans case, sous « Anomalies à résoudre avant recette ». Rappel du projet en tête des
// pages de suite ; signatures jamais coupées.
import { DARK, FONT, INK, LW, MUTE, RULE, drawHeader, ink, projectHeader, revision, txt, type Doc } from './pdfkit';

type St = 'to_test' | 'ok' | 'reserve' | 'ko';
export interface PvPoint { key: string; label: string; detail: string; deleted: boolean; status: St; comment: string | null; by: string | null; at: string | null }
export interface PvGroup { title: string; points: PvPoint[] }
export interface PvData { progress: number; total: number; counts: Record<St, number>; cables: PvGroup[]; equipment: PvGroup[]; free: PvPoint[]; deleted: PvPoint[]; reserves: PvPoint[] }
const ST: Record<St, { label: string }> = { to_test: { label: 'À tester' }, ok: { label: 'OK' }, reserve: { label: 'Réserve' }, ko: { label: 'KO' } };
/** Groupe des liaisons dont une extrémité n'existe plus (nom du serveur, ancien et nouveau). */
const MISSING = new Set(['Extrémité manquante', 'Extrémité supprimée']);
/** Extrémités d'une liaison (liste des câbles), par identifiant de câble. */
export interface PvEnd { element: string; equipment: string; tag: string; port: string; face?: string | null }
export type PvEnds = Map<string, { a: PvEnd; b: PvEnd; signal?: string }>;

/** Extrémité lisible d'une liaison : « élément › équipement ID › port », construite à partir
 *  des seuls morceaux renseignés (jamais de séparateur orphelin). Un port de panel traversant
 *  porte sa face (« 12 avant », « 12 arrière ») : les deux faces ont le même nom. */
export const endLabel = (e: PvEnd) => [e.element, `${e.equipment ?? ''} ${e.tag ?? ''}`.trim(), [e.port, e.port ? e.face : ''].filter(Boolean).join(' ')].filter((x) => x && String(x).trim()).join(' › ');
export async function pvEnds(pid: string): Promise<PvEnds> {
  const { api } = await import('./api');
  try {
    const t = await api.get<{ rows: { id: string; signal: string; a: PvEnd; b: PvEnd }[] }>(`/projects/${pid}/cables/table`);
    return new Map(t.rows.map((r) => [r.id, { a: r.a, b: r.b, signal: r.signal }]));
  } catch { return new Map(); }
}

type PvProject = { name: string; client: string | null; venue: string | null; shoot_start?: string | null; shoot_end?: string | null; commissioning?: string | null; rev?: number };

/** Écrit le procès-verbal à partir de la page courante du document (vierge). `site` et `logo`
 *  sont repris par l'en-tête commun (identité du site). */
export function drawPV(doc: Doc, r: PvData, view: { project: PvProject }, site: string, _logo: string | null, paper: 'A4' | 'A3' = 'A4', ends: PvEnds = new Map()) {
  const fmt: [number, number] = paper === 'A3' ? [297, 420] : [210, 297];
  const W = doc.internal.pageSize.getWidth(), H = doc.internal.pageSize.getHeight(), m = 12, bottom = H - 18;
  const head = projectHeader(view.project, 'Procès-verbal de recette');
  let y = m;
  // Colonnes : statut, point, observations (vide, à remplir), verdict (trois cases).
  const BOX = 3.6, cVer = W - m - 24, cSt = m, cPt = m + 21, cObs = cVer - 50, ptW = cObs - cPt - 3;
  const boxX = (i: number) => cVer + 1.5 + i * 8;
  const runHead = () => {
    doc.setFont(FONT.sans, 'bold'); ink(doc, 8, false); doc.text(txt(`PV de recette · ${view.project.name}`), m, m + 2);
    doc.setFont(FONT.sans, 'normal'); ink(doc, 7.5);
    doc.text(txt([site, `édité le ${new Date().toLocaleDateString('fr-FR')}`, revision(view.project)].filter(Boolean).join(' · ')), W - m, m + 2, { align: 'right' });
    doc.setDrawColor(...INK); doc.setLineWidth(LW.frame); doc.line(m, m + 4, W - m, m + 4);
    y = m + 9;
  };
  const colHead = (verdict: boolean) => {
    doc.setFont(FONT.sans, 'bold'); ink(doc, 7.5);
    doc.text('STATUT', cSt, y + 3); doc.text('POINT CONTRÔLÉ', cPt, y + 3); doc.text('OBSERVATIONS', cObs + 1.5, y + 3);
    if (verdict) ['OK', 'R', 'KO'].forEach((l, i) => doc.text(l, boxX(i) + BOX / 2, y + 3, { align: 'center' }));
    doc.setDrawColor(...INK); doc.setLineWidth(LW.frame); doc.line(m, y + 4.6, W - m, y + 4.6);
    y += 5.6;
  };
  let table: false | 'verdict' | 'plain' = false;
  const room = (h: number) => { if (y + h > bottom) { doc.addPage(fmt, 'portrait'); runHead(); if (table) colHead(table === 'verdict'); } };

  // En-tête de la première page : le même bloc d'identité que les autres exports.
  y = drawHeader(doc, head);
  // Câbles sans numéro : avertissement avant tout le reste.
  const unnumbered = r.cables.filter((g) => !MISSING.has(g.title)).flatMap((g) => g.points).filter((p) => p.detail === 'non numéroté').length;
  if (unnumbered) {
    doc.setFillColor(253, 243, 224); doc.setDrawColor(150, 105, 0); doc.setLineWidth(LW.rule); doc.rect(m, y - 1, W - 2 * m, 7, 'FD');
    doc.setFont(FONT.sans, 'bold'); ink(doc, 8.5, false);
    doc.text(txt(`⚠ ${unnumbered} câble${unnumbered > 1 ? 's' : ''} sans numéro : numérotez-les avant la recette (ils sont désignés ici par leurs extrémités).`), m + 2, y + 3.6, { maxWidth: W - 2 * m - 4 });
    y += 10;
  }
  // Récapitulatif
  doc.setDrawColor(...INK); doc.setLineWidth(LW.frame); doc.rect(m, y, W - 2 * m, 16);
  const cells: [string, string][] = [['Avancement', `${r.progress} %`], ['Points', String(r.total)], ['OK', String(r.counts.ok)], ['Réserve', String(r.counts.reserve)], ['KO', String(r.counts.ko)], ['À tester', String(r.counts.to_test)]];
  const cw = (W - 2 * m) / cells.length;
  cells.forEach(([l, v], i) => { doc.setFont(FONT.sans, 'bold'); ink(doc, 8); doc.setTextColor(...DARK); doc.text(txt(l.toUpperCase()), m + i * cw + 3, y + 5.2); doc.setFontSize(13); doc.setTextColor(...INK); doc.text(v, m + i * cw + 3, y + 12.8); doc.setFont(FONT.sans, 'normal'); });
  y += 20;
  // Légende des cases, sous l'en-tête.
  doc.setFont(FONT.sans, 'normal'); ink(doc, 8);
  doc.text(txt('Points à tester : cochez une case, OK (conforme), R (réserve) ou KO (non conforme), et notez vos observations. Un point déjà jugé porte son verdict.'), m, y + 2, { maxWidth: W - 2 * m });
  y += 9;
  const color: Record<St, [number, number, number]> = { to_test: DARK, ok: [14, 120, 72], reserve: [150, 105, 0], ko: [190, 45, 35] };
  const line = (p: PvPoint, anomaly = false) => {
    // Liaison : titre (numéro, sinon le trajet), signal et les deux extrémités, d'après la liste des câbles.
    const id = p.key.startsWith('cable:') ? p.key.slice(6) : null, e = id ? ends.get(id) : undefined;
    const headTxt = `${p.label}${e?.signal ? ` · ${e.signal}` : ''}${p.detail === 'non numéroté' ? ' · non numéroté' : ''}`;
    const body = e ? [`De : ${endLabel(e.a) || 'extrémité manquante'}`, `Vers : ${endLabel(e.b) || 'extrémité manquante'}`] : p.detail && p.detail !== 'non numéroté' ? [p.detail] : [];
    doc.setFont(FONT.sans, 'bold'); doc.setFontSize(9);
    const t1 = doc.splitTextToSize(txt(headTxt), ptW) as string[];
    doc.setFont(FONT.sans, 'normal');
    const t2 = body.flatMap((b) => doc.splitTextToSize(txt(b), ptW) as string[]);
    const com = p.comment ? (doc.splitTextToSize(txt(p.comment), ptW) as string[]) : [];
    const lh = 4.2, h = Math.max(9, lh * (t1.length + t2.length + com.length) + 3.2);
    room(h);
    doc.setFont(FONT.sans, 'bold'); doc.setFontSize(8);
    if (anomaly) { doc.setTextColor(...color.ko); doc.text(txt('ANOMALIE'), cSt, y + 4); }
    else { doc.setTextColor(...color[p.status]); doc.text(txt(ST[p.status].label.toUpperCase()), cSt, y + 4); }
    doc.setFontSize(9); doc.setTextColor(...INK); doc.text(t1, cPt, y + 4, { lineHeightFactor: lh / (9 * 25.4 / 72) });
    doc.setFont(FONT.sans, 'normal'); doc.text(t2, cPt, y + 4 + lh * t1.length, { lineHeightFactor: lh / (9 * 25.4 / 72) });
    if (com.length) { doc.setTextColor(...MUTE); doc.text(com, cPt, y + 4 + lh * (t1.length + t2.length), { lineHeightFactor: lh / (9 * 25.4 / 72) }); }
    // Observations (vide, filet de séparation) et, pour un point à tester, les trois cases.
    doc.setDrawColor(...RULE); doc.setLineWidth(LW.rule); doc.line(cObs, y + 1, cObs, y + h - 1);
    if (!anomaly && p.status === 'to_test') { doc.setDrawColor(...INK); doc.setLineWidth(LW.frame); for (let i = 0; i < 3; i++) doc.rect(boxX(i), y + 1.3, BOX, BOX); }
    y += h; doc.setDrawColor(...RULE); doc.setLineWidth(LW.rule); doc.line(m, y, W - m, y);
  };
  const section = (title: string, groups: PvGroup[], anomaly = false) => {
    if (!groups.some((g) => g.points.length)) return;
    table = false;
    // Titre, en-tête de colonnes, sous-titre et deux points au moins ensemble.
    room(9 + 5.6 + 6 + 2 * 13); doc.setFont(FONT.sans, 'bold'); ink(doc, 13, false); doc.text(txt(title), m, y + 5); y += 8;
    if (anomaly) { doc.setFont(FONT.sans, 'normal'); ink(doc, 8); doc.text(txt('Ces liaisons ne comptent pas dans l’avancement : rebranchez ou supprimez le câble avant la recette.'), m, y + 2); y += 5; }
    table = anomaly ? 'plain' : 'verdict'; colHead(!anomaly);
    for (const g of groups) {
      if (!g.points.length) continue;
      if (g.title) { room(6 + 2 * 13); doc.setFontSize(9.5); doc.setFont(FONT.sans, 'bold'); doc.setTextColor(...MUTE); doc.text(txt(g.title), m, y + 4.5); y += 6.5; }
      for (const p of g.points) line(p, anomaly);
      y += 2;
    }
    table = false; y += 3;
  };
  section('Liaisons', r.cables.filter((g) => !MISSING.has(g.title)));
  section('Équipements', r.equipment);
  section('Points libres', [{ title: '', points: r.free }]);
  section('Réserves à lever', [{ title: '', points: r.reserves }]);
  section('Anomalies à résoudre avant recette', r.cables.filter((g) => MISSING.has(g.title)).map((g) => ({ ...g, title: '' })), true);
  // Signatures, jamais coupées.
  room(46);
  y += 4;
  const bw = (W - 2 * m - 8) / 2;
  for (const [i, t] of ['Prestataire', 'Client'].entries()) {
    const x = m + i * (bw + 8);
    doc.setDrawColor(...INK); doc.setLineWidth(LW.frame); doc.rect(x, y, bw, 40);
    doc.setFont(FONT.sans, 'bold'); ink(doc, 9, false); doc.text(t, x + 3, y + 6);
    doc.setFont(FONT.sans, 'normal'); ink(doc, 8); doc.text('Nom, date et signature', x + 3, y + 11);
  }
  doc.setTextColor(...INK);
}
