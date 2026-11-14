// Conversion d'un élément posé (vue /containers/{id}/rack) vers le modèle de la scène 3D, avec
// les connecteurs du catalogue (mis en cache).
import { api } from './api';
import { fnColor, refById } from './catalog.svelte';
import { ALU, accessoryFace, visualOf } from './faceplate';
import type { El3D } from './rack3d';

const portsCache = new Map<string, Promise<any[]>>();
function ports(itemId: string | null): Promise<any[]> {
  if (!itemId) return Promise.resolve([]);
  if (!portsCache.has(itemId)) portsCache.set(itemId, api.get<{ item: { data: { ports?: any[] } } }>(`/catalog/items/${itemId}`).then((r) => r.item.data.ports ?? [], () => []));
  return portsCache.get(itemId)!;
}

export async function toEl3D(e: any): Promise<El3D> {
  const kind = e.nature === 'accessory' ? (e.accessory?.kind === 'panel' ? 'panel' : e.accessory?.kind === 'plate' ? (e.accessory.variant === 'brush' ? 'brush' : e.accessory.variant === 'vented' ? 'vent' : 'blank') : accessoryFace(e.label)) : visualOf(e);
  const ps = (await ports(e.item_id)).map((p: any) => ({ face: p.face ?? 'rear', connector: p.connector_id ?? '', quantity: p.quantity ?? 1, color: p.signal_id ? (refById('signals', p.signal_id)?.data?.color ?? null) : null, label: p.label ?? null }));
  return { id: e.placement.id, label: e.label, tag: e.tag, nature: e.nature, height_u: e.height_u, depth_mm: e.depth_mm, function: e.function, brand: e.brand ?? null, face: e.placement.face, u_start: e.placement.u_start, width: e.placement.width, col: e.placement.col, kind, alu: ALU.has(kind), accessory: e.accessory, ports: ps, fnColor: fnColor(e.function), shelf_id: e.placement.shelf_id ?? null } as El3D;
}

/** Spécification 3D d'une baie ou d'un flight-case. */
export const boxSpec = (c: any) => ({ kind: c.kind ?? 'rack', name: c.name, units: c.height_u ?? 42, width_mm: c.width_mm ?? null, depth_mm: c.depth_mm ?? c.length_mm ?? null, height_mm: c.outer_height_mm ?? null, rear_rails_mm: c.rear_rails_mm ?? null, wheels: !!c.wheels });
/** Spécification 3D d'un mobilier. */
export const deskSpec = (d: any) => ({ name: d.name, length_mm: d.length_mm ?? 2400, depth_mm: d.width_mm ?? 900, height_mm: d.outer_height_mm ?? 750 });
