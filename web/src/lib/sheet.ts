// Planche d'élévations (section 9.8) : composition libre des baies (vues avant, arrière, côté et
// 3D), du mobilier (face, côté, dessus, 3D, casquettes), de titres de lieux et d'emplacements
// réservés, sur une ou plusieurs feuilles avec cartouche. Toutes les cotes sont en mm papier.

export type Paper = 'A4' | 'A3' | 'A2' | 'A1' | 'A0' | 'Letter' | 'Legal';
export type Face = 'front' | 'rear' | 'rear_inner';
export type RackView = 'front' | 'rear' | 'side' | '3d';
export type DeskView = 'front' | 'side' | 'top' | '3d' | 'casq';
export const PAPERS: Record<Paper, [number, number]> = {
  A4: [210, 297], A3: [297, 420], A2: [420, 594], A1: [594, 841], A0: [841, 1189], Letter: [215.9, 279.4], Legal: [215.9, 355.6],
};
export const SCALES = [5, 10, 15, 20, 25, 30, 50];
export const FACE_LABEL: Record<Face, string> = { front: 'Avant', rear: 'Arrière', rear_inner: 'Arrière intérieure' };
export const RACK_VIEWS: Record<RackView, string> = { front: 'Avant', rear: 'Arrière', side: 'Côté', '3d': '3D' };
export const DESK_VIEWS: Record<DeskView, string> = { front: 'Face', side: 'Côté', top: 'Dessus', '3d': '3D', casq: 'Casquettes' };

export interface SheetItem {
  id: string; kind: 'rack' | 'desk' | 'location' | 'reserved'; x: number; y: number;
  container_id?: string; views?: string[]; location_id?: string; label?: string; w?: number; h?: number;
  /** Ancien format : une seule face par rack. */
  face?: Face;
  /** Vue 3D : angle (caméra et cible), proportion largeur/hauteur du cadre, portes. */
  cam?: number[] | null; r3d?: number; doors?: 'closed' | 'removed'; sides?: boolean;
}
/** Ce que la planche dessine d'une baie (vue /containers/{id}/rack). */
export interface RackData { name: string; units: number; elements: any[]; depth_mm: number; c: any; vpdus?: any[] }
export interface CasqData { id: string; name: string; offset_mm: number; units: number; depth_mm: number; tilt_deg: number }
/** Mobilier : dimensions et casquettes. */
export interface DeskData { name: string; length_mm: number; depth_mm: number; height_mm: number; casq: CasqData[]; c: any }
export interface Ctx { racks: Record<string, RackData>; desks: Record<string, DeskData> }
export interface SheetPage { id: string; items: SheetItem[] }
export interface TitleBlock { title: string; reference: string; author: string; checker: string; version: string; standards: string; notes: string }
export interface Sheet {
  paper: Paper; orientation: 'portrait' | 'landscape'; scale: number; format: 'pdf' | 'png' | 'jpg';
  titleBlock: boolean; block: TitleBlock; pages: SheetPage[];
}

/** Cadre à 10 mm du bord de la feuille ; contenu à 4 mm à l'intérieur du cadre. */
export const FRAME = 10;
export const MARGIN = 14;
export const BLOCK: [number, number] = [180, 44];
export const U_MM = 44.45;
/** Encombrement réel d'une baie dessinée : panneau 19″ et montants. */
const RACK_W = 482.6 + 2 * 32;
const RACK_PAD = 30;
/** Nom du groupe, libellé de chaque vue, colonne de numéros à gauche (mm papier). */
export const GROUP_HEAD = 5.5;
export const VIEW_HEAD = 4.5;
export const RACK_NUM = 7;
export const VIEW_GAP = 7;
/** Casquette : joues et capot autour du 19″ (mm réels). */
export const CASQ_W = 482.6 + 50;
export const CASQ_EXTRA = 50;

export const uid = () => Math.random().toString(36).slice(2, 10);

export function pageSize(s: Sheet): [number, number] {
  const [w, h] = PAPERS[s.paper];
  return s.orientation === 'landscape' ? [h, w] : [w, h];
}
export const rackGeom = (k: number) => ({ u: U_MM / k, pad: RACK_PAD / k, inner: 482.6 / k, rail: 32 / k });
/** Hauteur dessinée d'une élévation de `units` U (sans libellés). */
export const rackBodyH = (units: number, k: number) => (units * U_MM + 2 * RACK_PAD) / k;

/** Profil d'une casquette inclinée dans le repère du mobilier (z depuis l'avant, y depuis le sol). */
export function casqProfile(d: DeskData, c: CasqData): [number, number][] {
  const dC = Math.max(150, Math.min(d.depth_mm - 50, c.depth_mm)), Hc = Math.max(1, c.units) * U_MM + CASQ_EXTRA;
  const th = (c.tilt_deg * Math.PI) / 180, pz = d.depth_mm - dC - 20; // pivot : bas avant
  // Coins (y, z vers l'arrière) d'un boîtier dC × Hc basculé vers l'arrière autour du pivot.
  return ([[0, 0], [Hc, 0], [Hc, dC], [0, dC]] as [number, number][]).map(([y, z]) => [pz + z * Math.cos(th) + y * Math.sin(th), d.height_mm + y * Math.cos(th) - z * Math.sin(th)]);
}
/** Encombrement réel du mobilier : longueur, profondeur hors tout, hauteur hors tout. */
export function deskExtent(d: DeskData): { L: number; D: number; H: number } {
  let D = d.depth_mm, H = d.height_mm;
  for (const c of d.casq) for (const [z, y] of casqProfile(d, c)) { D = Math.max(D, z); H = Math.max(H, y); }
  return { L: d.length_mm, D, H };
}

export interface ViewBox { v: string; x: number; y: number; w: number; h: number; cid?: string }
export interface GroupLayout { w: number; h: number; views: ViewBox[] }

/** Disposition d'un groupe (baie ou mobilier) : vues côte à côte, alignées en bas, sur
 *  plusieurs rangées quand elles dépassent `maxW` (largeur utile de la feuille). */
export function groupLayout(it: SheetItem, k: number, ctx: Ctx, maxW = Infinity): GroupLayout {
  const parts: { v: string; w: number; h: number; cid?: string }[] = [];
  if (it.kind === 'rack') {
    const r = ctx.racks[it.container_id ?? ''];
    const units = r?.units ?? 42, depth = r?.depth_mm ?? 800, hb = rackBodyH(units, k);
    for (const v of viewsOf(it)) {
      if (v === 'front' || v === 'rear') parts.push({ v, w: RACK_NUM + RACK_W / k, h: hb });
      else if (v === 'side') parts.push({ v, w: (depth + 60) / k, h: hb });
      else if (v === '3d') parts.push({ v, w: hb * (it.r3d ?? 0.8), h: hb });
    }
  } else {
    const d = ctx.desks[it.container_id ?? ''];
    const e = d ? deskExtent(d) : { L: 2000, D: 900, H: 750 };
    for (const v of viewsOf(it)) {
      if (v === 'front') parts.push({ v, w: e.L / k, h: e.H / k });
      else if (v === 'side') parts.push({ v, w: e.D / k, h: e.H / k });
      else if (v === 'top') parts.push({ v, w: e.L / k, h: e.D / k });
      else if (v === '3d') parts.push({ v, w: (e.H / k) * (it.r3d ?? 1.8), h: e.H / k });
      else if (v === 'casq') for (const c of d?.casq ?? []) parts.push({ v, cid: c.id, w: RACK_NUM + RACK_W / k, h: rackBodyH(c.units, k) });
    }
  }
  if (!parts.length) parts.push({ v: 'none', w: 30, h: 10 });
  const rows: (typeof parts)[] = [[]];
  let x = 0;
  for (const p of parts) {
    if (rows.at(-1)!.length && x + p.w > maxW) { rows.push([]); x = 0; }
    rows.at(-1)!.push(p); x += p.w + VIEW_GAP;
  }
  const views: ViewBox[] = [];
  let y = GROUP_HEAD, w = 0;
  for (const row of rows) {
    const rh = Math.max(...row.map((p) => p.h));
    let rx = 0;
    for (const p of row) { views.push({ ...p, x: rx, y: y + VIEW_HEAD + rh - p.h }); rx += p.w + VIEW_GAP; }
    w = Math.max(w, rx - VIEW_GAP); y += VIEW_HEAD + rh + VIEW_GAP;
  }
  return { w, h: y - VIEW_GAP, views };
}
/** Largeur utile d'une feuille pour un groupe. */
export const usableW = (s: Sheet) => pageSize(s)[0] - 2 * MARGIN;

export function viewsOf(it: SheetItem): string[] {
  if (it.views?.length) return it.views;
  if (it.kind === 'rack') return [it.face === 'rear' || it.face === 'rear_inner' ? 'rear' : 'front'];
  return it.kind === 'desk' ? ['front', 'side'] : [];
}

export function itemSize(it: SheetItem, k: number, ctx: Ctx, maxW = Infinity): [number, number] {
  if (it.kind === 'rack' || it.kind === 'desk') { const g = groupLayout(it, k, ctx, maxW); return [g.w, g.h]; }
  if (it.kind === 'location') return [it.w ?? 120, 10];
  return [it.w ?? 60, it.h ?? 60];
}

/** Remet une planche enregistrée au format courant (vues multiples). */
export function normalize(s: Sheet): Sheet {
  for (const p of s.pages) for (const it of p.items) if (it.kind === 'rack' && !it.views?.length) { it.views = viewsOf(it); delete it.face; }
  return s;
}

export function blankSheet(): Sheet {
  return {
    paper: 'A3', orientation: 'landscape', scale: 20, format: 'pdf', titleBlock: true,
    block: { title: 'Élévations', reference: '', author: '', checker: '', version: 'A', standards: '', notes: '' },
    pages: [],
  };
}

interface Ref { id: string; location_id: string }
interface LocRef { id: string; name: string }

/** Range baies puis mobilier lieu par lieu, de gauche à droite, en évitant le cartouche ; une page de plus quand ça déborde. */
export function autoLayout(s: Sheet, ctx: Ctx, racks: Ref[], desks: Ref[], locations: LocRef[], rackViews: string[], deskViews: string[]): SheetPage[] {
  const [W, H] = pageSize(s);
  const k = s.scale, gap = 10;
  const pages: SheetPage[] = [];
  let page: SheetPage = { id: uid(), items: [] };
  let x = MARGIN, y = MARGIN, rowH = 0;
  const block = s.titleBlock ? { x: W - FRAME - BLOCK[0], y: H - FRAME - BLOCK[1] } : null;
  const hitsBlock = (x0: number, y0: number, w: number, h: number) => !!block && x0 + w > block.x - 4 && y0 + h > block.y - 4;
  const newPage = () => { if (page.items.length) pages.push(page); page = { id: uid(), items: [] }; x = MARGIN; y = MARGIN; rowH = 0; };
  const newRow = () => { x = MARGIN; y += rowH + gap; rowH = 0; };
  const place = (w: number, h: number): [number, number] => {
    for (let guard = 0; guard < 50; guard++) {
      if (x + w > W - MARGIN && x !== MARGIN) { newRow(); continue; }
      if (y + h > H - MARGIN && y !== MARGIN) { newPage(); continue; }
      if (hitsBlock(x, y, w, h) && !(x === MARGIN && y === MARGIN)) { if (x === MARGIN) { newPage(); continue; } newRow(); continue; }
      const at: [number, number] = [x, y];
      x += w + gap; rowH = Math.max(rowH, h);
      return at;
    }
    return [MARGIN, MARGIN];
  };
  for (const l of locations) {
    const items: SheetItem[] = [
      ...racks.filter((r) => r.location_id === l.id).map((r) => ({ id: uid(), kind: 'rack' as const, x: 0, y: 0, container_id: r.id, views: [...rackViews] })),
      ...desks.filter((r) => r.location_id === l.id).map((r) => ({ id: uid(), kind: 'desk' as const, x: 0, y: 0, container_id: r.id, views: [...deskViews] })),
    ];
    if (!items.length) continue;
    if (x !== MARGIN) newRow();
    const firstH = itemSize(items[0]!, k, ctx, W - 2 * MARGIN)[1];
    if (y + 13 + firstH > H - MARGIN && y !== MARGIN) newPage();
    page.items.push({ id: uid(), kind: 'location', x: MARGIN, y, location_id: l.id, w: Math.min(W - 2 * MARGIN, 160) });
    y += 13;
    for (const it of items) { const [w, h] = itemSize(it, k, ctx, W - 2 * MARGIN); [it.x, it.y] = place(w, h); page.items.push(it); }
  }
  if (page.items.length || !pages.length) pages.push(page);
  return pages;
}

export function fileName(project: string, ext: string, page?: number): string {
  const p = project.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^A-Za-z0-9]+/g, '_').replace(/^_|_$/g, '');
  const d = new Date();
  const day = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  return `Elevations_${p}_${day}${page ? `_p${page}` : ''}.${ext}`;
}
