// Libellés des ports d'un nœud : sur une même ligne, l'entrée (à gauche) et la sortie (à
// droite) se partagent la largeur libre ; un libellé trop long est raccourci par « … »,
// mesuré en pixels avec la police réelle, pour ne jamais chevaucher son voisin.

let ctx: CanvasRenderingContext2D | null = null;
const cache = new Map<string, number>();

export const PORT_FONT = '600 12px "Inter Variable", Inter, system-ui, sans-serif';

/** Largeur d'un texte en pixels (approximation si le canvas est indisponible). */
export function textWidth(text: string, font = PORT_FONT): number {
  const k = font + '\u0000' + text;
  const hit = cache.get(k);
  if (hit != null) return hit;
  if (!ctx && typeof document !== 'undefined') ctx = document.createElement('canvas').getContext('2d');
  let w: number;
  if (ctx) { ctx.font = font; w = ctx.measureText(text).width; } else w = text.length * 6.9;
  if (cache.size > 4000) cache.clear();
  cache.set(k, w);
  return w;
}

/** Encre d'un texte au-dessus et au-dessous de sa ligne de base, en pixels (approximation
 *  sans canvas : capitales et descendantes d'Inter). */
export function textInk(text: string, font = PORT_FONT): [number, number] {
  const k = font + '\u0001' + text;
  const hit = inks.get(k);
  if (hit) return hit;
  if (!ctx && typeof document !== 'undefined') ctx = document.createElement('canvas').getContext('2d');
  const px = Number(/(\d+(?:\.\d+)?)px/.exec(font)?.[1] ?? 12);
  let v: [number, number] = [px * 0.94, px * 0.24];
  if (ctx) { ctx.font = font; const m = ctx.measureText(text); if (m.actualBoundingBoxAscent || m.actualBoundingBoxDescent) v = [m.actualBoundingBoxAscent, m.actualBoundingBoxDescent]; }
  if (inks.size > 4000) inks.clear();
  inks.set(k, v);
  return v;
}
const inks = new Map<string, [number, number]>();

/** Oublie les mesures (à appeler quand une police web vient d'arriver). */
export function resetTextWidths(): void { cache.clear(); inks.clear(); }

/** Raccourcit `text` pour tenir dans `max` pixels, en gardant le numéro final (« IN… 12 »). */
export function fitText(text: string, max: number, font = PORT_FONT): string {
  if (max <= 0) return '';
  if (textWidth(text, font) <= max) return text;
  const m = /^(.*\S)(\s+\d+(?:\s*↔)?|\s*↔)$/.exec(text);
  if (m && m[1]!.length > 1) {
    const tail = m[2]!, room = max - textWidth(tail, font);
    if (room > textWidth('M…', font)) return cut(m[1]!, room, font) + tail;
  }
  return cut(text, max, font);
}

/** Sous-titre d'un nœud (« marque modèle · ID · élément ») : on raccourcit le premier morceau
 *  (le modèle) et on garde la fin entière, plutôt que d'en couper la fin (« VIDEOHU… 3 »). */
export function fitSub(text: string, max: number, font = PORT_FONT): string {
  if (max <= 0) return '';
  if (textWidth(text, font) <= max) return text;
  const i = text.indexOf(' · ');
  if (i > 0) {
    const tail = text.slice(i), room = max - textWidth(tail, font);
    if (room > textWidth('MMM…', font)) return cut(text.slice(0, i), room, font) + tail;
  }
  return cut(text, max, font);
}

function cut(text: string, max: number, font: string): string {
  let lo = 0, hi = text.length;
  while (lo < hi) {
    const mid = Math.ceil((lo + hi) / 2);
    if (textWidth(text.slice(0, mid).trimEnd() + '…', font) <= max) lo = mid; else hi = mid - 1;
  }
  return lo ? text.slice(0, lo).trimEnd() + '…' : '…';
}

/**
 * Largeurs accordées aux libellés gauche et droit d'une ligne qui dispose de `space` pixels
 * entre les deux étiquettes de connecteur. Chacun garde ce dont il a besoin si tout tient ;
 * sinon le plus court est servi d'abord et le plus long prend le reste.
 */
export function shareRow(left: number | null, right: number | null, space: number, gap = 12): [number, number] {
  if (left == null) return [0, Math.max(0, space)];
  if (right == null) return [Math.max(0, space), 0];
  const free = Math.max(0, space - gap);
  if (left + right <= free) return [left, right];
  const half = free / 2;
  if (left <= half) return [left, free - left];
  if (right <= half) return [free - right, right];
  return [half, half];
}
