// Géométrie des élévations : positions au demi-U (textes « U12 », « haut de U12 ») et vue de
// côté (profondeur de chaque élément depuis ses rails, réserve de câblage derrière). La même
// logique sert à l'écran (Racks) et aux planches (SheetPage), pour des cotes identiques.

import { nf } from './format';

/** Nombre entier (à 1e-9 près). */
export const isWhole = (x: number) => Math.abs(x - Math.round(x)) < 1e-9;
/** Hauteur ou position en U, à la française : « 12 », « 12,5 ». */
export const fmtU = (u: number) => nf(u, isWhole(u) ? 0 : 1);

/** U occupés par un élément posé en `u` (bas) sur `h` U : « U12 à U13 », « haut de U12 à bas de U13 ». */
export function occTxt(u: number, h: number): string {
  const a = u - 1, b = a + h;
  if (isWhole(a) && isWhole(b)) return b - a > 1 + 1e-9 ? `U${u} à U${Math.round(b)}` : `U${u}`;
  const lo = Math.floor(a + 1e-9) + 1, hi = Math.ceil(b - 1e-9);
  const part = (n: number) => {
    const s = Math.max(a, n - 1), e = Math.min(b, n);
    if (e - s >= 1 - 1e-9) return `U${n}`;
    return s > n - 1 + 1e-9 ? `haut de U${n}` : `bas de U${n}`;
  };
  return lo === hi ? part(lo) : `${part(lo)} à ${part(hi)}`;
}
/** Plage courte (notifications, fantôme) : « U12–U13 » au U entier, sinon le détail des demi-U. */
export const rangeTxt = (u: number, h: number) => (isWhole(u) && isWhole(h) ? (h > 1 ? `U${u}–U${u + h - 1}` : `U${u}`) : occTxt(u, h));
/** Place visée (fantôme, notifications de déplacement) : « U12–U13 » au U entier, sinon la
 *  position de départ puis le détail : « U12,5 · haut de U12 à bas de U13 ». */
export const aimTxt = (u: number, h: number) => (isWhole(u) && isWhole(h) ? rangeTxt(u, h) : `U${fmtU(u)} · ${occTxt(u, h)}`);
/** Saisie d'un U : « 12 », « 12,5 », « 12.5 », « U12,5 » ; null si ce n'est pas un nombre. */
export function parseU(s: string | number | null | undefined): number | null {
  if (typeof s === 'number') return Number.isFinite(s) ? s : null;
  const t = (s ?? '').trim().replace(/^u\s*/i, '').replace(',', '.').replace(/\s+/g, '');
  if (!t || !/^[+-]?(\d+([.]\d*)?|[.]\d+)$/.test(t)) return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
}

// ----- Vue de côté -----
/** Rail avant : à 70 mm de la face avant de la baie ; rail arrière : `rear_rails_mm` (70 par défaut) avant le fond. */
export const FRONT_RAIL_MM = 70;
interface SideEl { nature: string; depth_mm?: number | null; rear_reserve_mm?: number | null; placement: { face: string } }
export interface SideBar<E> {
  e: E;
  /** Profondeur du corps (renseignée et non nulle, sinon 300 mm pour un équipement et 30 mm pour un accessoire). */
  d: number;
  /** Réserve de câblage (secours) derrière l'équipement : `rear_reserve_mm`, 80 mm par défaut ; rien pour un accessoire. */
  res: number;
  /** Corps et réserve, en mm depuis la face avant de la baie. */
  x: number; w: number; rx: number; rw: number;
  /** Corps + réserve plus profonds que la baie (même règle que l'avertissement du serveur). */
  over: boolean;
  front: boolean;
  frontRail: number; rearRail: number;
}
/** Corps et réserve de chaque élément : face avant depuis le rail avant, faces arrière depuis le rail arrière. */
export function sideGeom<E extends SideEl>(els: E[], depthMm: number, rearRailsMm?: number | null): SideBar<E>[] {
  const D = depthMm, fr = FRONT_RAIL_MM, rr = D - (rearRailsMm ?? 70);
  return els.map((e) => {
    const acc = e.nature === 'accessory';
    const d = e.depth_mm || (acc ? 30 : 300);
    const res = acc ? 0 : (e.rear_reserve_mm ?? 80);
    const front = e.placement.face === 'front';
    const w = Math.max(0, Math.min(d, front ? D - fr : rr));
    const x = front ? fr : rr - w;
    const rw = Math.max(0, Math.min(res, front ? D - (x + w) : x));
    const rx = front ? x + w : x - rw;
    return { e, d, res, x, w, rx, rw, over: d + res > D, front, frontRail: fr, rearRail: rr };
  });
}
