// Étiquettes de câbles (section 11.7) : seuls les câbles numérotés ; numéro, équipement et
// connecteur côté A et côté B ; couleur du câble, à défaut celle du signal ; mise en page
// sur la planche choisie (zone imprimable en haut, zone transparente laissée vierge).

import { newDoc, addPage, txt, FONT, type LabelSheet } from './pdfkit';

export interface LabelCable { number: string; color: string | null; a: { equipment: string; tag: string; port: string; connector: string }; b: { equipment: string; tag: string; port: string; connector: string } }

function rgb(hex: string | null): [number, number, number] | null {
  if (!hex || !/^#[0-9a-f]{6}$/i.test(hex)) return null;
  return [parseInt(hex.slice(1, 3), 16), parseInt(hex.slice(3, 5), 16), parseInt(hex.slice(5, 7), 16)];
}

/** Une étiquette par extrémité (deux par câble), ou une seule si `perEnd` est faux. */
export async function cableLabels(cables: LabelCable[], s: LabelSheet, opts: { perEnd: boolean; skip: number } = { perEnd: true, skip: 0 }): Promise<Blob> {
  const doc = await newDoc(s.paper, false);
  const per = s.cols * s.rows;
  const items: { c: LabelCable; end: 'A' | 'B' | null }[] = [];
  for (const c of cables) { if (opts.perEnd) { items.push({ c, end: 'A' }, { c, end: 'B' }); } else items.push({ c, end: null }); }
  let slot = Math.max(0, Math.min(opts.skip, per - 1));
  for (const it of items) {
    if (slot >= per) { addPage(doc, s.paper, false); slot = 0; }
    const col = slot % s.cols, row = Math.floor(slot / s.cols);
    const x = s.margin_left + col * (s.label_w + s.gap_x), y = s.margin_top + row * (s.label_h + s.gap_y);
    const h = s.print_h > 0 ? s.print_h : s.label_h, w = s.label_w;
    // Bande de couleur à gauche, numéro en grand, puis les deux extrémités.
    const band = rgb(it.c.color);
    const bw = Math.min(3, w * 0.08);
    if (band) { doc.setFillColor(...band); doc.rect(x, y, bw, h, 'F'); }
    const ix = x + (band ? bw + 1.2 : 1.2), iw = w - (ix - x) - 1;
    const big = Math.min(h * 0.42, 14) * 2.6;
    doc.setFont(FONT.sans, 'bold'); doc.setFontSize(Math.max(6, Math.min(big, (iw / Math.max(it.c.number.length, 3)) * 4.2)));
    doc.setTextColor(21, 23, 26);
    doc.text(txt(it.c.number), ix, y + Math.min(h * 0.42, 6.5));
    doc.setFont(FONT.sans, 'normal');
    const small = Math.max(6, Math.min(6.5, h * 0.75)); // jamais sous 6 pt
    doc.setFontSize(small);
    const line = (e: LabelCable['a'], tag: 'A' | 'B') => `${tag} ${e.equipment} ${e.tag} › ${e.port}${e.connector ? ` (${e.connector})` : ''}`;
    const lines = it.end === 'B' ? [line(it.c.b, 'B'), line(it.c.a, 'A')] : [line(it.c.a, 'A'), line(it.c.b, 'B')];
    let ly = y + Math.min(h * 0.42, 6.5) + small * 0.42;
    for (const l of lines) {
      if (ly > y + h - 0.6) break;
      doc.text(doc.splitTextToSize(txt(l), iw)[0] ?? '', ix, ly);
      ly += small * 0.4;
    }
    // Repère de la zone transparente (non imprimée) : un filet léger en pointillés.
    if (s.print_h > 0 && s.print_h < s.label_h) { doc.setDrawColor(200, 205, 212); doc.setLineWidth(0.1); doc.setLineDashPattern([0.6, 0.6], 0); doc.line(x, y + s.print_h, x + w, y + s.print_h); doc.setLineDashPattern([], 0); }
    slot++;
  }
  return doc.output('blob');
}
