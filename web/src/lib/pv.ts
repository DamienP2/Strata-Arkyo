// Procès-verbal de recette (section 11.5) : réutilisé par l'écran Recette et par le dossier.
import type { Doc } from './pdfkit';

type St = 'to_test' | 'ok' | 'reserve' | 'ko';
export interface PvPoint { key: string; label: string; detail: string; deleted: boolean; status: St; comment: string | null; by: string | null; at: string | null }
export interface PvGroup { title: string; points: PvPoint[] }
export interface PvData { progress: number; total: number; counts: Record<St, number>; cables: PvGroup[]; equipment: PvGroup[]; free: PvPoint[]; deleted: PvPoint[]; reserves: PvPoint[] }
const ST: Record<St, { label: string }> = { to_test: { label: 'À tester' }, ok: { label: 'OK' }, reserve: { label: 'Réserve' }, ko: { label: 'KO' } };

/** Écrit le procès-verbal à partir de la page courante du document (vierge). */
export function drawPV(doc: Doc, r: PvData, view: { project: { name: string; client: string | null; venue: string | null } }, site: string, logo: string | null, paper: 'A4' | 'A3' = 'A4') {
  const fmt: [number, number] = paper === 'A3' ? [297, 420] : [210, 297];
  const W = doc.internal.pageSize.getWidth(), H = doc.internal.pageSize.getHeight(), m = 14;
  let y = m;
  const room = (h: number) => { if (y + h > H - m - 8) { doc.addPage(fmt, 'portrait'); y = m; } };
  if (logo) { try { const p = doc.getImageProperties(logo); const lh = 14, lw = Math.min(50, (p.width / p.height) * lh); doc.addImage(logo, logo.startsWith('data:image/png') ? 'PNG' : 'JPEG', W - m - lw, y, lw, (lw * p.height) / p.width); } catch { /* logo illisible */ } }
  doc.setFont('helvetica', 'bold'); doc.setFontSize(9); doc.setTextColor(91, 99, 109); doc.text(site.toUpperCase(), m, y + 4);
  doc.setFontSize(20); doc.setTextColor(21, 23, 26); doc.text('Procès-verbal de recette', m, y + 14);
  doc.setFont('helvetica', 'normal'); doc.setFontSize(10.5);
  doc.text(`${view.project.name}${view.project.client ? ` · ${view.project.client}` : ''}`, m, y + 21);
  doc.text(`${view.project.venue ?? ''}${view.project.venue ? ' · ' : ''}édité le ${new Date().toLocaleDateString('fr-FR')}`, m, y + 26.5);
  y += 34;
  // Récapitulatif
  doc.setDrawColor(21, 23, 26); doc.setLineWidth(0.3); doc.rect(m, y, W - 2 * m, 16);
  const cells: [string, string][] = [['Avancement', `${r.progress} %`], ['Points', String(r.total)], ['OK', String(r.counts.ok)], ['Réserves', String(r.counts.reserve)], ['KO', String(r.counts.ko)], ['À tester', String(r.counts.to_test)]];
  const cw = (W - 2 * m) / cells.length;
  cells.forEach(([l, v], i) => { doc.setFontSize(7); doc.setTextColor(91, 99, 109); doc.text(l.toUpperCase(), m + i * cw + 3, y + 5); doc.setFont('helvetica', 'bold'); doc.setFontSize(13); doc.setTextColor(21, 23, 26); doc.text(v, m + i * cw + 3, y + 12.5); doc.setFont('helvetica', 'normal'); });
  y += 24;
  const color: Record<St, [number, number, number]> = { to_test: [91, 99, 109], ok: [14, 133, 80], reserve: [183, 132, 0], ko: [207, 58, 47] };
  // Les polices standard du PDF n'ont pas la flèche : « » » la remplace.
  const pdfText = (t: string) => t.replace(/→/g, '»').replace(/↔/g, '<>');
  const line = (p: PvPoint) => {
    const txt = doc.splitTextToSize(pdfText(`${p.label}${p.detail ? ` — ${p.detail}` : ''}`), W - 2 * m - 30);
    const com = p.comment ? doc.splitTextToSize(pdfText(p.comment), W - 2 * m - 30) : [];
    const h = 4.6 * (txt.length + com.length) + 2;
    room(h);
    doc.setFont('helvetica', 'bold'); doc.setFontSize(8); doc.setTextColor(...color[p.status]); doc.text(ST[p.status].label.toUpperCase(), m, y + 3.5);
    doc.setFont('helvetica', 'normal'); doc.setFontSize(9); doc.setTextColor(21, 23, 26); doc.text(txt, m + 26, y + 3.5);
    if (com.length) { doc.setTextColor(91, 99, 109); doc.text(com, m + 26, y + 3.5 + 4.6 * txt.length); }
    y += h; doc.setDrawColor(225, 228, 232); doc.line(m, y - 0.8, W - m, y - 0.8);
  };
  const section = (title: string, groups: PvGroup[]) => {
    if (!groups.some((g) => g.points.length)) return;
    room(14); doc.setFont('helvetica', 'bold'); doc.setFontSize(13); doc.setTextColor(21, 23, 26); doc.text(title, m, y + 5); y += 9;
    for (const g of groups) { if (!g.points.length) continue; room(10); doc.setFontSize(9.5); doc.setFont('helvetica', 'bold'); doc.setTextColor(91, 99, 109); doc.text(g.title || 'Sans élément', m, y + 4); y += 6; for (const p of g.points) line(p); y += 2; }
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
    doc.setDrawColor(21, 23, 26); doc.rect(x, y, bw, 40);
    doc.setFont('helvetica', 'bold'); doc.setFontSize(9); doc.setTextColor(21, 23, 26); doc.text(t, x + 3, y + 6);
    doc.setFont('helvetica', 'normal'); doc.setFontSize(8); doc.setTextColor(91, 99, 109); doc.text('Nom, date et signature', x + 3, y + 11);
  }
}
