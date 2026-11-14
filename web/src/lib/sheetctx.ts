// Données d'une planche d'élévations : élévations des baies et casquettes, dimensions du
// mobilier, et rendus 3D sans fond (hors écran) pour les vues 3D.
import { api } from './api';
import type { ProjectView } from './projects';
import { boxSpec, deskSpec, toEl3D } from './el3d';
import { groupLayout, type Ctx, type DeskData, type RackData, type Sheet, type SheetItem } from './sheet';
import type { RackScene } from './rack3d';

export const isDesk = (c: { kind: string }) => c.kind === 'desk' || c.kind === 'furniture';

export async function loadCtx(view: ProjectView): Promise<Ctx> {
  const racks: Record<string, RackData> = {};
  await Promise.all(view.containers.filter((c) => c.rackable).map(async (c) => {
    let elements: any[] = [], vpdus: any[] = [];
    try { const r = await api.get<{ elements: any[]; vpdus?: any[] }>(`/containers/${c.id}/rack`); elements = r.elements; vpdus = r.vpdus ?? []; } catch { /* élévation illisible */ }
    racks[c.id] = { name: c.name, units: c.height_u ?? 42, elements, depth_mm: c.depth_mm ?? c.length_mm ?? 800, c, vpdus };
  }));
  const desks: Record<string, DeskData> = {};
  for (const d of view.containers.filter(isDesk)) {
    const s = deskSpec(d);
    desks[d.id] = {
      name: d.name, length_mm: s.length_mm, depth_mm: s.depth_mm, height_mm: s.height_mm, c: d,
      casq: view.containers.filter((c) => c.kind === 'casquette' && c.parent_id === d.id).sort((a, b) => (a.offset_mm ?? 0) - (b.offset_mm ?? 0))
        .map((c) => ({ id: c.id, name: c.name, offset_mm: c.offset_mm ?? 0, units: c.height_u ?? 6, depth_mm: c.depth_mm ?? 350, tilt_deg: c.tilt_deg ?? 0 })),
    };
  }
  return { racks, desks };
}

// ----- Rendus 3D hors écran -----
let scene: RackScene | null = null;
let host: HTMLDivElement | null = null;
let queue: Promise<unknown> = Promise.resolve();
const cache = new Map<string, string>();
/** Pixels par millimètre papier (≈ 250 dpi), plafonnés. */
const PX_MM = 10, MAX_PX = 1800;

/** Construit dans `sc` la baie ou le mobilier d'un élément de planche. */
export async function build3D(sc: RackScene, it: SheetItem, ctx: Ctx) {
  if (it.kind === 'rack') {
    const r = ctx.racks[it.container_id ?? '']; if (!r) return;
    sc.build(boxSpec(r.c), await Promise.all(r.elements.map(toEl3D)), (r.vpdus ?? []).map((v) => v.accessory));
    sc.setDoors(it.doors ?? 'removed'); sc.setSides(it.sides ?? true);
  } else {
    const d = ctx.desks[it.container_id ?? '']; if (!d) return;
    const cs = await Promise.all(d.casq.map(async (c) => ({ ...c, els: await Promise.all((ctx.racks[c.id]?.elements ?? []).map(toEl3D)) })));
    sc.buildDesk(deskSpec(d.c), cs); sc.setSides(it.sides ?? true);
  }
  if (it.cam?.length === 6) sc.setPose(it.cam);
}

function keyOf(it: SheetItem, ctx: Ctx, w: number, h: number) {
  const src = it.kind === 'rack' ? ctx.racks[it.container_id ?? ''] : ctx.desks[it.container_id ?? ''];
  const els = it.kind === 'rack' ? (src as RackData | undefined)?.elements : (src as DeskData | undefined)?.casq.map((c) => [c, ctx.racks[c.id]?.elements]);
  return JSON.stringify([it.kind, it.container_id, it.cam, it.doors, it.sides, w, h, (src as any)?.c?.rev, els]);
}

/** Image PNG sans fond de la vue 3D d'un élément, au format du cadre sur la feuille. */
export function shot3D(it: SheetItem, ctx: Ctx, k: number): Promise<string> {
  const box = groupLayout(it, k, ctx).views.find((v) => v.v === '3d');
  if (!box) return Promise.resolve('');
  const f = Math.min(PX_MM, MAX_PX / Math.max(box.w, box.h));
  const w = Math.round(box.w * f), h = Math.round(box.h * f);
  const key = keyOf(it, ctx, w, h);
  if (cache.has(key)) return Promise.resolve(cache.get(key)!);
  const job = queue.then(async () => {
    if (!scene) {
      host = document.createElement('div'); host.style.cssText = 'position:fixed;left:-10000px;top:0;width:10px;height:10px;pointer-events:none';
      document.body.appendChild(host);
      const { RackScene } = await import('./rack3d');
      scene = new RackScene(host, { transparent: true, still: true });
    }
    scene.setSize(w, h);
    const saved = it.cam; it = { ...it, cam: null };
    await build3D(scene, it, ctx);
    if (saved?.length === 6) scene.setPose(saved);
    const url = scene.snapshot();
    cache.set(key, url);
    return url;
  });
  queue = job.catch(() => undefined);
  return job;
}

/** Rendus 3D de toutes les vues 3D des feuilles données. */
export async function shotsFor(sheet: Sheet, ctx: Ctx, pages = sheet.pages): Promise<Record<string, string>> {
  const out: Record<string, string> = {};
  for (const p of pages) for (const it of p.items) if ((it.kind === 'rack' || it.kind === 'desk') && it.views?.includes('3d')) {
    try { out[it.id] = await shot3D(it, ctx, sheet.scale); } catch { /* WebGL indisponible */ }
  }
  return out;
}
