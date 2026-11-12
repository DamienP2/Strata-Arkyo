// Routage orthogonal des câbles du synoptique : tous les câbles d'une vue sont tracés
// ensemble pour qu'ils ne se chevauchent pas. Chaque câble reçoit son propre couloir
// vertical, au pas constant `SP` ; les câbles d'un même faisceau restent parallèles et
// ordonnés pour ne pas se croiser ; aucun segment ne traverse un nœud. Quand le passage
// direct est impossible (câble qui revient en arrière, même côté, nœud au milieu), le câble
// fait un détour par un couloir horizontal libre, lui aussi attribué sans chevauchement.

export type Side = 'l' | 'r';
export interface End { x: number; y: number; side: Side }
export interface Req { id: string; a: End; b: End; out: number }
export type Box = [number, number, number, number];
export interface Routed { d: string; pts: [number, number][] }

export const SP = 10; // écart entre deux câbles parallèles
/** Marge autour d'un nœud : un segment qui part d'un port ne « traverse » pas son propre nœud. */
const PAD = 8;

interface V { x: number; y0: number; y1: number; id: string }
interface H { y: number; x0: number; x1: number; id: string }

export function routeAll(reqs: Req[], boxes: Box[]): Map<string, Routed> {
  const out = new Map<string, Routed>();
  const vs: V[] = [], hs: H[] = [];
  const lo = (a: number, b: number) => Math.min(a, b), hi = (a: number, b: number) => Math.max(a, b);
  // Un segment vertical en x entre ya et yb traverse-t-il un nœud ?
  const vHitsNode = (x: number, ya: number, yb: number) => boxes.some(([x0, y0, x1, y1]) => x > x0 && x < x1 && hi(ya, yb) > y0 && lo(ya, yb) < y1);
  const hHitsNode = (y: number, xa: number, xb: number) => boxes.some(([x0, y0, x1, y1]) => y > y0 && y < y1 && hi(xa, xb) > x0 && lo(xa, xb) < x1);
  // Index par couloir : un contrôle ne regarde que les segments des couloirs voisins.
  const vIdx = new Map<number, V[]>(), hIdx = new Map<number, H[]>();
  const near = <T,>(idx: Map<number, T[]>, c: number): T[] => { const k = Math.floor(c / SP); return [...(idx.get(k - 1) ?? []), ...(idx.get(k) ?? []), ...(idx.get(k + 1) ?? [])]; };
  const vClash = (x: number, ya: number, yb: number, id: string) => near(vIdx, x).some((v) => v.id !== id && Math.abs(v.x - x) < SP - 0.5 && hi(ya, yb) > v.y0 - SP / 2 && lo(ya, yb) < v.y1 + SP / 2);
  const hClash = (y: number, xa: number, xb: number, id: string) => near(hIdx, y).some((h) => h.id !== id && Math.abs(h.y - y) < SP - 0.5 && hi(xa, xb) > h.x0 - SP / 2 && lo(xa, xb) < h.x1 + SP / 2);
  const addV = (x: number, ya: number, yb: number, id: string) => { const v = { x, y0: lo(ya, yb), y1: hi(ya, yb), id }; vs.push(v); const k = Math.floor(x / SP); vIdx.set(k, [...(vIdx.get(k) ?? []), v]); };
  const addH = (y: number, xa: number, xb: number, id: string) => { const h = { y, x0: lo(xa, xb), x1: hi(xa, xb), id }; hs.push(h); const k = Math.floor(y / SP); hIdx.set(k, [...(hIdx.get(k) ?? []), h]); };
  const path = (pts: [number, number][]) => {
    // Supprime les points alignés, puis écrit le chemin en segments droits.
    const p = pts.filter((q, i) => i === 0 || q[0] !== pts[i - 1]![0] || q[1] !== pts[i - 1]![1]);
    const keep = p.filter((q, i) => i === 0 || i === p.length - 1 || !((p[i - 1]![0] === q[0] && q[0] === p[i + 1]![0]) || (p[i - 1]![1] === q[1] && q[1] === p[i + 1]![1])));
    return { d: keep.map((q, i) => `${i ? 'L' : 'M'}${q[0]} ${q[1]}`).join(''), pts: keep };
  };

  // 1. Passages directs : sortie à droite vers une entrée plus à droite, dans l'espace libre
  //    entre les deux. On les regroupe par colonne de départ (même x de sortie).
  const direct: { r: Req; ax: number; ay: number; bx: number; by: number; lo: number; hi: number }[] = [];
  const rest: Req[] = [];
  for (const r of reqs) {
    const [A, B] = r.a.side === 'l' && r.b.side === 'r' ? [r.b, r.a] : [r.a, r.b];
    const lo0 = A.x + r.out, hi0 = B.x - r.out;
    if (A.side === 'r' && B.side === 'l' && hi0 - lo0 >= 0) direct.push({ r, ax: A.x, ay: A.y, bx: B.x, by: B.y, lo: lo0, hi: hi0 });
    else rest.push(r);
  }
  const groups = new Map<number, typeof direct>();
  for (const c of direct) { const k = Math.round(c.ax); groups.set(k, [...(groups.get(k) ?? []), c]); }
  for (const g of [...groups.values()].sort((a, b) => a[0]!.ax - b[0]!.ax)) {
    // Ordre des couloirs : les câbles qui montent à gauche (départ croissant), ceux qui
    // descendent à droite (départ croissant = couloir le plus à droite) : un faisceau reste
    // parallèle et ne se croise pas.
    const up = g.filter((c) => c.by < c.ay).sort((a, b) => a.ay - b.ay || a.by - b.by);
    const down = g.filter((c) => c.by >= c.ay).sort((a, b) => b.ay - a.ay || b.by - a.by);
    const flat = g.filter((c) => Math.abs(c.by - c.ay) < 1);
    const ordered = [...up, ...down.filter((c) => !flat.includes(c))];
    for (const c of flat) {
      // Même hauteur : un trait droit, si rien ne le gêne.
      if (!hHitsNode(c.ay, c.ax + PAD, c.bx - PAD) && !hClash(c.ay, c.ax, c.bx, c.r.id)) { addH(c.ay, c.ax, c.bx, c.r.id); out.set(c.r.id, path([[c.ax, c.ay], [c.bx, c.by]])); }
      else ordered.push(c);
    }
    let slot = 0;
    for (const c of ordered) {
      let placed = false;
      for (let x = Math.ceil(c.lo / SP) * SP + slot * SP; x <= c.hi; x += SP) {
        if (vHitsNode(x, c.ay, c.by) || vClash(x, c.ay, c.by, c.r.id)) continue;
        if (hHitsNode(c.ay, c.ax + PAD, x) || hHitsNode(c.by, x, c.bx - PAD) || hClash(c.ay, c.ax, x, c.r.id) || hClash(c.by, x, c.bx, c.r.id)) continue;
        addV(x, c.ay, c.by, c.r.id); addH(c.ay, c.ax, x, c.r.id); addH(c.by, x, c.bx, c.r.id);
        out.set(c.r.id, path([[c.ax, c.ay], [x, c.ay], [x, c.by], [c.bx, c.by]]));
        slot = Math.round((x - Math.ceil(c.lo / SP) * SP) / SP) + 1; placed = true; break;
      }
      if (!placed) rest.push(c.r);
    }
  }

  // 2. Détours : couloir vertical près de chaque extrémité, couloir horizontal libre entre.
  const lane = (e: End, outD: number, ya: number, yb: number, id: string): number => {
    const dir = e.side === 'r' ? 1 : -1;
    // Couloirs calés sur une grille commune : écart constant d'un câble à l'autre.
    const x0 = e.x + dir * outD, start = dir > 0 ? Math.ceil(x0 / SP) * SP : Math.floor(x0 / SP) * SP;
    for (let k = 0; k < 120; k++) {
      const x = start + dir * k * SP;
      if (!vHitsNode(x, ya, yb) && !vClash(x, ya, yb, id) && !hHitsNode(e.y, e.x + dir * PAD, x) && !hClash(e.y, e.x + dir * PAD, x, id)) return x;
    }
    return e.x + dir * outD;
  };
  const track = (xa: number, xb: number, base: number, dir: number, id: string): number => {
    for (let k = 0; k < 400; k++) {
      const y = base + dir * k * SP;
      if (!hHitsNode(y, xa, xb) && !hClash(y, xa, xb, id)) return y;
    }
    return base;
  };
  // Côté du détour (dessous ou dessus) : le plus court des deux couloirs libres de nœuds.
  const plan = rest.map((r) => {
    const xa = r.a.x + (r.a.side === 'r' ? r.out : -r.out), xb = r.b.x + (r.b.side === 'r' ? r.out : -r.out);
    const yMax = hi(r.a.y, r.b.y), yMin = lo(r.a.y, r.b.y);
    let below = Math.ceil((yMax + 3 * SP) / SP) * SP; while (hHitsNode(below, xa, xb) && below < yMax + 20000) below += SP;
    let above = Math.floor((yMin - 3 * SP) / SP) * SP; while (hHitsNode(above, xa, xb) && above > yMin - 20000) above -= SP;
    const dir = below - yMax <= yMin - above ? 1 : -1;
    return { r, dir, base: dir > 0 ? below : above };
  });
  // Boucles imbriquées : les câbles les plus proches du couloir passent à l'intérieur.
  plan.sort((p, q) => (p.dir !== q.dir ? p.dir - q.dir : p.dir > 0 ? hi(q.r.a.y, q.r.b.y) - hi(p.r.a.y, p.r.b.y) : lo(p.r.a.y, p.r.b.y) - lo(q.r.a.y, q.r.b.y)));
  for (const { r, dir, base } of plan) {
    const { a, b } = r;
    let xa = lane(a, r.out, a.y, base, r.id), xb = lane(b, r.out, b.y, base, r.id);
    const y = track(lo(xa, xb), hi(xa, xb), base, dir, r.id);
    xa = lane(a, r.out, a.y, y, r.id); xb = lane(b, r.out, b.y, y, r.id);
    addV(xa, a.y, y, r.id); addV(xb, b.y, y, r.id); addH(a.y, a.x, xa, r.id); addH(b.y, b.x, xb, r.id); addH(y, xa, xb, r.id);
    out.set(r.id, path([[a.x, a.y], [xa, a.y], [xa, y], [xb, y], [xb, b.y], [b.x, b.y]]));
  }
  return out;
}

/** Arrondit les angles d'un chemin orthogonal (rayon r). */
export function rounded(pts: [number, number][], r = 6): string {
  if (pts.length < 3) return pts.map((q, i) => `${i ? 'L' : 'M'}${q[0]} ${q[1]}`).join('');
  let d = `M${pts[0]![0]} ${pts[0]![1]}`;
  for (let i = 1; i < pts.length - 1; i++) {
    const [px, py] = pts[i - 1]!, [x, y] = pts[i]!, [nx, ny] = pts[i + 1]!;
    const l1 = Math.hypot(x - px, y - py), l2 = Math.hypot(nx - x, ny - y), k = Math.min(r, l1 / 2, l2 / 2);
    const ax = x - ((x - px) / (l1 || 1)) * k, ay = y - ((y - py) / (l1 || 1)) * k;
    const bx = x + ((nx - x) / (l2 || 1)) * k, by = y + ((ny - y) / (l2 || 1)) * k;
    d += `L${ax} ${ay}Q${x} ${y} ${bx} ${by}`;
  }
  const l = pts[pts.length - 1]!; return d + `L${l[0]} ${l[1]}`;
}
