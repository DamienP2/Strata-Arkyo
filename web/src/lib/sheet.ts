// Planche d'élévations (section 9.8) : composition libre de racks (faces au choix), de titres
// de lieux et d'emplacements réservés sur une ou plusieurs feuilles, avec cartouche.
// Toutes les cotes sont en millimètres papier.

export type Paper = 'A4' | 'A3' | 'A2' | 'A1' | 'A0' | 'Letter' | 'Legal';
export type Face = 'front' | 'rear' | 'rear_inner';
export const PAPERS: Record<Paper, [number, number]> = {
  A4: [210, 297], A3: [297, 420], A2: [420, 594], A1: [594, 841], A0: [841, 1189], Letter: [215.9, 279.4], Legal: [215.9, 355.6],
};
export const SCALES = [5, 10, 15, 20, 25];
export const FACE_LABEL: Record<Face, string> = { front: 'Avant', rear: 'Arrière', rear_inner: 'Arrière intérieure' };

export interface SheetItem {
  id: string; kind: 'rack' | 'location' | 'reserved'; x: number; y: number;
  container_id?: string; face?: Face; location_id?: string; label?: string; w?: number; h?: number;
}
/** Ce que la planche dessine d'un rack : nom, hauteur et éléments posés (vue /containers/{id}/rack). */
export interface RackData { name: string; units: number; elements: any[] }
export interface SheetPage { id: string; items: SheetItem[] }
export interface TitleBlock { title: string; reference: string; author: string; checker: string; version: string; standards: string; notes: string }
export interface Sheet {
  paper: Paper; orientation: 'portrait' | 'landscape'; scale: number; format: 'pdf' | 'png' | 'jpg';
  titleBlock: boolean; block: TitleBlock; pages: SheetPage[];
}

export const MARGIN = 10;
export const BLOCK: [number, number] = [180, 44];
export const U_MM = 44.45;
/** Encombrement réel d'une baie dessinée : panneau 19″ et montants. */
const RACK_W = 482.6 + 2 * 32;
const RACK_PAD = 30;
/** Bandeau de titre au-dessus du rack et colonne de numéros à gauche (mm papier). */
export const RACK_HEAD = 9;
export const RACK_NUM = 7;

export const uid = () => Math.random().toString(36).slice(2, 10);

export function pageSize(s: Sheet): [number, number] {
  const [w, h] = PAPERS[s.paper];
  return s.orientation === 'landscape' ? [h, w] : [w, h];
}

/** Taille d'un rack de `units` U à l'échelle 1:k, titre et numéros compris. */
export function rackSize(units: number, k: number): [number, number] {
  return [RACK_NUM + RACK_W / k, RACK_HEAD + (units * U_MM + 2 * RACK_PAD) / k];
}
export const rackGeom = (k: number) => ({ u: U_MM / k, pad: RACK_PAD / k, inner: 482.6 / k, rail: 32 / k });

export function itemSize(it: SheetItem, k: number, unitsOf: (cid: string) => number): [number, number] {
  if (it.kind === 'rack') return rackSize(unitsOf(it.container_id ?? ''), k);
  if (it.kind === 'location') return [it.w ?? 120, 10];
  return [it.w ?? 60, it.h ?? 60];
}

export function blankSheet(): Sheet {
  return {
    paper: 'A3', orientation: 'landscape', scale: 10, format: 'pdf', titleBlock: true,
    block: { title: 'Élévations des racks', reference: '', author: '', checker: '', version: 'A', standards: '', notes: '' },
    pages: [],
  };
}

interface RackRef { id: string; location_id: string; units: number }
interface LocRef { id: string; name: string }

/** Range les racks lieu par lieu, de gauche à droite, en évitant le cartouche ; une page de plus quand ça déborde. */
export function autoLayout(s: Sheet, racks: RackRef[], locations: LocRef[], faces: Face[]): SheetPage[] {
  const [W, H] = pageSize(s);
  const k = s.scale, gap = 8;
  const pages: SheetPage[] = [];
  let page: SheetPage = { id: uid(), items: [] };
  let x = MARGIN, y = MARGIN, rowH = 0;
  const block = s.titleBlock ? { x: W - MARGIN - BLOCK[0], y: H - MARGIN - BLOCK[1] } : null;
  const hitsBlock = (x0: number, y0: number, w: number, h: number) => !!block && x0 + w > block.x - 4 && y0 + h > block.y - 4;
  const newPage = () => { if (page.items.length) pages.push(page); page = { id: uid(), items: [] }; x = MARGIN; y = MARGIN; rowH = 0; };
  const newRow = () => { x = MARGIN; y += rowH + gap; rowH = 0; };
  const place = (w: number, h: number): [number, number] => {
    for (let guard = 0; guard < 50; guard++) {
      if (x + w > W - MARGIN) { newRow(); continue; }
      if (y + h > H - MARGIN) { newPage(); continue; }
      if (hitsBlock(x, y, w, h)) { if (x === MARGIN) { newPage(); continue; } newRow(); continue; }
      const at: [number, number] = [x, y];
      x += w + gap; rowH = Math.max(rowH, h);
      return at;
    }
    return [MARGIN, MARGIN];
  };
  for (const l of locations) {
    const mine = racks.filter((r) => r.location_id === l.id);
    if (!mine.length) continue;
    if (x !== MARGIN) newRow();
    const sizes = mine.flatMap((r) => faces.map(() => rackSize(r.units, k)));
    const firstH = sizes[0]![1];
    if (y + 12 + firstH > H - MARGIN) newPage();
    page.items.push({ id: uid(), kind: 'location', x: MARGIN, y, location_id: l.id, w: Math.min(W - 2 * MARGIN, 160) });
    y += 13;
    for (const r of mine) for (const f of faces) {
      const [w, h] = rackSize(r.units, k);
      const [px, py] = place(w, h);
      page.items.push({ id: uid(), kind: 'rack', x: px, y: py, container_id: r.id, face: f });
    }
  }
  if (page.items.length || !pages.length) pages.push(page);
  return pages;
}

export function fileName(project: string, ext: string, page?: number): string {
  const p = project.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^A-Za-z0-9]+/g, '_').replace(/^_|_$/g, '');
  const d = new Date();
  const day = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  return `Racks_${p}_${day}${page ? `_p${page}` : ''}.${ext}`;
}
