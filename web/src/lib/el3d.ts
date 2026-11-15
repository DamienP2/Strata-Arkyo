// Conversion d'un élément posé (vue /containers/{id}/rack) vers le modèle de la scène 3D, avec
// les connecteurs du catalogue (mis en cache). Couleurs : famille de signal (familyColor,
// papier) ; nommage commun aux rendus (rackLines) ; polices de l'interface chargées avant le
// premier rendu des façades.
import { api } from './api';
import { familyColor, refById } from './catalog.svelte';
import { modelOf, rackLines } from './racktext.svelte';
import { accessoryFace, visualOf } from './faceplate';
import type { El3D } from './rack3d';

const portsCache = new Map<string, Promise<any[]>>();
function ports(itemId: string | null): Promise<any[]> {
  if (!itemId) return Promise.resolve([]);
  if (!portsCache.has(itemId)) portsCache.set(itemId, api.get<{ item: { data: { ports?: any[] } } }>(`/catalog/items/${itemId}`).then((r) => r.item.data.ports ?? [], () => []));
  return portsCache.get(itemId)!;
}

let fontsReady: Promise<unknown> | null = null;
/** Inter et JetBrains Mono chargées (sinon la façade peinte retombe sur une police système). */
function fonts(): Promise<unknown> {
  if (typeof document === 'undefined' || !document.fonts) return Promise.resolve();
  fontsReady ??= Promise.all([document.fonts.load("600 12px 'JetBrains Mono'"), document.fonts.load('600 12px Inter'), document.fonts.ready]).catch(() => undefined);
  return fontsReady;
}

export async function toEl3D(e: any): Promise<El3D> {
  await fonts();
  const kind = e.nature === 'accessory' ? (e.accessory?.kind === 'panel' ? 'panel' : e.accessory?.kind === 'plate' ? (e.accessory.variant === 'brush' ? 'brush' : e.accessory.variant === 'vented' ? 'vent' : 'blank') : accessoryFace(e.label)) : visualOf(e);
  const ps = (await ports(e.item_id)).map((p: any) => ({ face: p.face ?? 'rear', connector: p.connector_id ?? '', quantity: p.quantity ?? 1, color: p.signal_id ? familyColor(refById('signals', p.signal_id)?.data?.function, { screen: false }) : null, label: p.label ?? null }));
  const panel = e.accessory?.kind === 'panel' ? e.accessory.panel : null;
  const slotColors: Record<number, string> = {};
  for (const s of panel?.slots ?? []) {
    const per = panel.per_u, row = Math.floor((s.index - 1) / per) + 1, col = ((s.index - 1) % per) + 1;
    const g = (panel.groups ?? []).find((x: any) => x.row === row && col >= x.start && col < x.start + x.len);
    slotColors[s.index] = familyColor(s.function ?? g?.function ?? e.function, { screen: false });
  }
  return { id: e.placement.id, label: e.label, tag: e.tag, nature: e.nature, height_u: e.height_u, depth_mm: e.depth_mm, function: e.function, brand: e.brand ?? null, face: e.placement.face, u_start: e.placement.u_start, width: e.placement.width, col: e.placement.col, kind, alu: false, // façades toutes foncées
    accessory: e.accessory, ports: ps, fnColor: familyColor(e.function, { screen: false }), shelf_id: e.placement.shelf_id ?? null,
    title: rackLines(e, 'both')[0] || e.tag || '', model: modelOf(e), slotColors } as El3D;
}

/** Spécification 3D d'une baie ou d'un flight-case. */
export const boxSpec = (c: any) => ({ kind: c.kind ?? 'rack', name: c.name, units: c.height_u ?? 42, width_mm: c.width_mm ?? null, depth_mm: c.depth_mm ?? c.length_mm ?? null, height_mm: c.outer_height_mm ?? null, rear_rails_mm: c.rear_rails_mm ?? null, wheels: !!c.wheels });
/** Spécification 3D d'un mobilier. */
export const deskSpec = (d: any) => ({ name: d.name, length_mm: d.length_mm ?? 2400, depth_mm: d.width_mm ?? 900, height_mm: d.outer_height_mm ?? 750 });
