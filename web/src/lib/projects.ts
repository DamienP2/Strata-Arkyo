// Projets et inventaire, tels que les renvoie l'API.

export type Status = 'preparation' | 'ready' | 'in_progress' | 'done' | 'cancelled';
export type Intervention = 'prestation' | 'integration';
export type Regie = 'flight_case' | 'car' | 'fixed';
export type ContainerKind = 'rack' | 'flight_case' | 'hard_case' | 'waterproof_case' | 'furniture';

export interface ProjectData {
  name: string; client: string | null; venue: string | null; description: string | null; service_type: string | null;
  intervention: Intervention; regie: Regie; status: Status;
  planning_start: string | null; planning_end: string | null; shoot_start: string | null; shoot_end: string | null; commissioning: string | null;
}
export interface Project extends ProjectData { id: string; rev: number; created_by: string | null; created_at: string; updated_at: string }
export interface ProjectSummary extends Project { completion: number; locations: number; containers: number; articles: number }

export interface ContainerTotals { used_u: number; total_u: number | null; weight_kg: number; power_w: number; volume_m3: number | null; over_weight: boolean; over_power: boolean; over_u: boolean }
export interface Location { id: string; project_id: string; name: string; code: string; position: number; rev: number; totals: { containers: number; free_items: number; power_w: number; weight_kg: number } }
export interface ContainerData {
  kind: ContainerKind; rackable: boolean; height_u: number | null; depth_mm: number | null; rear_rails_mm: number | null;
  length_mm: number | null; width_mm: number | null; outer_height_mm: number | null; wheels: boolean;
  empty_weight_kg: number | null; max_weight_kg: number | null; max_power_w: number | null; cable_code: string; notes: string | null;
}
export interface Container extends ContainerData { id: string; project_id: string; location_id: string; name: string; position: number; rev: number; totals: ContainerTotals }
export type Accessory = { kind: 'plate'; variant: 'blank' | 'brush' | 'vented'; height_u: number } | { kind: 'shelf'; depth_mm: number; height_u: number } | { kind: 'drawer'; depth_mm: number; height_u: number };
export interface Article { id: string; project_id: string; location_id: string; container_id: string | null; position: number; nature: 'equipment' | 'adapter' | 'accessory'; item_id: string | null; quantity: number; accessory: Accessory | null; rev: number; label: string }
export interface Instance { id: string; project_id: string; article_id: string; tag: string; serial: string | null; mac: string | null; inventory_tag: string | null; rev: number }
export interface ItemView { id: string; kind: string; name: string; reference: string | null; brand: string | null; category: string | null; rackable: boolean; height_u: number | null; depth_mm: number | null; weight_kg: number | null; power_w: number | null; modular: boolean; active: boolean }
export type TaskStatus = 'todo' | 'in_progress' | 'done' | 'validated';
export type Priority = 'low' | 'normal' | 'high' | 'urgent';
export interface Task { id: string; project_id: string; position: number; title: string; description: string | null; due: string | null; done_on: string | null; assignee_id: string | null; status: TaskStatus; priority: Priority; rev: number; created_by: string | null; created_at: string; updated_at: string }

export interface ProjectView {
  project: Project; completion: number; today: string;
  locations: Location[]; containers: Container[]; articles: Article[]; instances: Instance[]; items: Record<string, ItemView>; tasks: Task[];
  presence: { id: string; name: string; color: string }[];
}

export const STATUS: Record<Status, { label: string; color: string }> = {
  preparation: { label: 'En préparation', color: 'var(--st-prep)' },
  ready: { label: 'Prêt', color: 'var(--st-pret)' },
  in_progress: { label: 'En cours', color: 'var(--st-cours)' },
  done: { label: 'Terminé', color: 'var(--st-fini)' },
  cancelled: { label: 'Annulé', color: 'var(--st-annule)' },
};
export const STATUS_ORDER: Status[] = ['preparation', 'ready', 'in_progress', 'done', 'cancelled'];
export const REGIE: Record<Regie, { label: string; ill: 'truck' | 'case' | 'fixe' }> = {
  flight_case: { label: 'Flight-case', ill: 'case' },
  car: { label: 'Car régie', ill: 'truck' },
  fixed: { label: 'Installation fixe', ill: 'fixe' },
};
export const CONTAINER_KINDS: Record<ContainerKind, { label: string; rackable: boolean; mobile: boolean }> = {
  rack: { label: 'Baie 19″', rackable: true, mobile: false },
  flight_case: { label: 'Flight-case', rackable: true, mobile: true },
  hard_case: { label: 'Valise rigide', rackable: true, mobile: true },
  waterproof_case: { label: 'Valise étanche', rackable: false, mobile: true },
  furniture: { label: 'Mobilier', rackable: false, mobile: false },
};
export const TASK_STATUS: Record<TaskStatus, { label: string; chip: string }> = {
  todo: { label: 'À faire', chip: 'info' }, in_progress: { label: 'En cours', chip: 'accent' }, done: { label: 'Terminé', chip: 'ok' }, validated: { label: 'Validé', chip: 'ok' },
};
export const PRIORITY: Record<Priority, { label: string; chip: string }> = {
  low: { label: 'Basse', chip: 'info' }, normal: { label: 'Normale', chip: 'info' }, high: { label: 'Haute', chip: 'warn' }, urgent: { label: 'Urgente', chip: 'err' },
};
export const U_PRESETS = [6, 9, 12, 18, 24, 30, 36, 42, 45, 48];

/** Date clé : début de tournage (prestation) ou mise en service (intégration). */
export const keyDate = (p: ProjectData) => (p.intervention === 'prestation' ? p.shoot_start : p.commissioning);

export function daysBetween(from: string, to: string): number {
  return Math.round((Date.parse(to + 'T12:00:00Z') - Date.parse(from + 'T12:00:00Z')) / 86_400_000);
}

export function fdate(d: string | null | undefined, opts: Intl.DateTimeFormatOptions = { day: 'numeric', month: 'short' }): string {
  if (!d) return '—';
  return new Date(d + 'T12:00:00').toLocaleDateString('fr-FR', opts);
}

/** Couleur d'urgence d'un compte à rebours (section 7.5) : critique ≤ 7 j, alerte ≤ 14 j. */
export const urgency = (days: number) => (days <= 7 ? 'var(--urg)' : days <= 14 ? 'var(--soon)' : 'var(--fg-2)');

export function blankContainer(kind: ContainerKind = 'rack'): ContainerData {
  const k = CONTAINER_KINDS[kind];
  return { kind, rackable: k.rackable, height_u: k.rackable ? (kind === 'rack' ? 42 : 12) : null, depth_mm: k.rackable ? 600 : 400, rear_rails_mm: null,
    length_mm: k.rackable ? null : 600, width_mm: k.rackable ? null : 400, outer_height_mm: null, wheels: false, empty_weight_kg: null, max_weight_kg: null, max_power_w: null, cable_code: '', notes: null };
}
