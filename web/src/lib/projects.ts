// Projets et inventaire, tels que les renvoie l'API.

export type Status = 'preparation' | 'ready' | 'in_progress' | 'done' | 'cancelled';
export type Intervention = 'prestation' | 'integration';
export type Regie = 'flight_case' | 'car' | 'fixed';
export type ContainerKind = 'rack' | 'flight_case' | 'hard_case' | 'waterproof_case' | 'furniture'
  | 'monitor_wall' | 'desk' | 'operator_position' | 'casquette'
  | 'zone' | 'camera_position' | 'field_box' | 'rf_mast'
  | 'site_point' | 'power' | 'external_feed'
  | 'display' | 'cable_drum';

/** Tuile d'un mur d'écrans : un écran de l'élément et la source qu'il affiche. */
export interface WallTile { row: number; col: number; w: number; h: number; instance_id: string | null; source: string; label: string }
export interface Wall { rows: number; cols: number; tiles: WallTile[]; support: string | null }
/** Poste d'un pupitre : un rôle et le matériel qui lui revient. */
export interface DeskPosition { id: string; role: string; name: string; instance_ids: string[] }
export interface ElementDetails {
  surface?: string | null; area_m2?: number | null; wall?: Wall | null; positions?: DeskPosition[];
  camera_number?: number | null; mount?: string | null; link?: string | null;
  mast_height_m?: number | null; frequencies?: string | null;
  power_kva?: number | null; phases?: number | null; outlet?: string | null;
  provider?: string | null; feed?: string | null; circuit_ref?: string | null; point_kind?: string | null;
  display_kind?: string | null; diagonal_in?: number | null; resolution?: string | null;
  cable_length_m?: number | null; drum_count?: number | null;
}

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
  details: ElementDetails;
  /** Casquette : mobilier qui la porte, position depuis son bord gauche, inclinaison. */
  parent_id?: string | null; offset_mm?: number | null; tilt_deg?: number | null;
}
export interface Container extends ContainerData { id: string; project_id: string; location_id: string; name: string; position: number; rev: number; totals: ContainerTotals }
export type Accessory = { kind: 'plate'; variant: 'blank' | 'brush' | 'vented'; height_u: number } | { kind: 'shelf'; depth_mm: number; height_u: number } | { kind: 'drawer'; depth_mm: number; height_u: number } | { kind: 'lacing_bar'; depth_mm: number; height_u: number } | { kind: 'panel'; panel: import('./panel').PanelData };
export interface Article { id: string; project_id: string; location_id: string; container_id: string | null; position: number; nature: 'equipment' | 'adapter' | 'accessory'; item_id: string | null; quantity: number; accessory: Accessory | null; host?: { article_id: string; slot: number } | null; rev: number; label: string }
export interface Instance { id: string; project_id: string; article_id: string; tag: string; serial: string | null; mac: string | null; inventory_tag: string | null; rev: number }
export interface ItemView { id: string; kind: string; name: string; reference: string | null; brand: string | null; category: string | null; rackable: boolean; height_u: number | null; depth_mm: number | null; weight_kg: number | null; power_w: number | null; modular: boolean; host_slots?: number | null; module_slots?: number | null; active: boolean }
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
export type ElementFamily = 'contenants' | 'regie' | 'terrain' | 'site' | 'diffusion';
export const ELEMENT_FAMILIES: { key: ElementFamily; label: string; help: string }[] = [
  { key: 'contenants', label: 'Contenants', help: 'Baies, flight-cases et valises : élévation, panels, circuits.' },
  { key: 'regie', label: 'Postes de régie', help: 'Monitoring et postes de travail.' },
  { key: 'terrain', label: 'Terrain et plateau', help: 'Ce qui se pose au plus près de l’action.' },
  { key: 'site', label: 'Infrastructure du site', help: 'Raccordements, énergie et liaisons extérieures.' },
  { key: 'diffusion', label: 'Diffusion et logistique', help: 'Écrans publics et câbles à prévoir.' },
];
/** Types d'éléments : libellé, famille, rackable par défaut, mobile (roulettes possibles). */
export const CONTAINER_KINDS: Record<ContainerKind, { label: string; family: ElementFamily; rackable: boolean; mobile: boolean; canRack?: boolean; hidden?: boolean; help: string }> = {
  rack: { label: 'Baie 19″', family: 'contenants', rackable: true, mobile: false, canRack: true, help: 'Baie fixe ou sur roulettes' },
  flight_case: { label: 'Flight-case', family: 'contenants', rackable: true, mobile: true, canRack: true, help: 'Rack de tournée' },
  hard_case: { label: 'Valise rigide', family: 'contenants', rackable: true, mobile: true, canRack: true, help: 'Petit rack de transport' },
  waterproof_case: { label: 'Valise étanche', family: 'contenants', rackable: false, mobile: true, help: 'Matériel en mousse' },
  furniture: { label: 'Meuble / armoire', family: 'contenants', rackable: false, mobile: false, help: 'Meuble, armoire, étagère ; casquettes possibles' },
  monitor_wall: { label: 'Mur d’écrans', family: 'regie', rackable: false, mobile: false, help: 'Grille d’écrans et sources affichées' },
  desk: { label: 'Mobilier', family: 'regie', rackable: false, mobile: false, help: 'Pupitre, console : postes et casquettes rackables' },
  operator_position: { label: 'Poste opérateur', family: 'regie', rackable: false, mobile: false, help: 'Commentateur, HF, intercom…' },
  casquette: { label: 'Casquette', family: 'regie', rackable: true, mobile: false, canRack: true, hidden: true, help: 'Bloc rackable posé sur un mobilier' },
  zone: { label: 'Emplacement libre', family: 'terrain', rackable: false, mobile: false, help: 'Table, sol ou loge, sans rack' },
  camera_position: { label: 'Position caméra', family: 'terrain', rackable: false, mobile: false, help: 'Caméra, support et liaison' },
  field_box: { label: 'Boîtier de terrain', family: 'terrain', rackable: false, mobile: true, help: 'Stagebox, éclateur fibre' },
  rf_mast: { label: 'Mât HF', family: 'terrain', rackable: false, mobile: false, help: 'Antennes et récepteurs' },
  site_point: { label: 'Point de raccordement', family: 'site', rackable: false, mobile: false, help: 'Boîte de sol, plaque murale, patch' },
  power: { label: 'Énergie', family: 'site', rackable: false, mobile: true, help: 'Groupe, armoire, distribution' },
  external_feed: { label: 'Arrivée externe', family: 'site', rackable: false, mobile: false, help: 'Fibre opérateur, satellite, 4G/5G' },
  display: { label: 'Diffusion', family: 'diffusion', rackable: false, mobile: false, help: 'Écran géant, LED, vidéoprojecteur' },
  cable_drum: { label: 'Touret', family: 'diffusion', rackable: false, mobile: true, help: 'Câble sur enrouleur' },
};
/** Suggestions des champs libres propres aux types. */
export const ELEMENT_HINTS: Record<string, string[]> = {
  surface: ['Table', 'Sol', 'Loge', 'Tribune', 'Bord terrain', 'Plateau'],
  support: ['Structure alu', 'Pieds', 'Bras articulés', 'Accroche plafond', 'Sur pupitre'],
  mount: ['Pied', 'Tête fluide', 'Grue', 'Steadicam', 'Épaule', 'PTZ', 'Rail', 'Drone', 'Câblecam'],
  link: ['Fibre SMPTE', 'Triax', 'HF', 'IP', 'SDI', 'Fibre monomode'],
  outlet: ['P17 16 A mono', 'P17 32 A tri', 'P17 63 A tri', 'P17 125 A tri', 'Powerlock 400 A'],
  feed: ['Fibre opérateur', 'Fibre noire', 'Satellite', '4G / 5G', 'Internet', 'Liaison HF'],
  point_kind: ['Boîte de sol', 'Plaque murale', 'Patch de site', 'Armoire de brassage', 'Stagebox', 'Éclateur fibre'],
  display_kind: ['Écran LED', 'Écran géant', 'Moniteur public', 'Vidéoprojecteur'],
  roles: ['Réalisateur', 'Scripte', 'Truquiste', 'Ingé son', 'Vision', 'Ralentis', 'Graphiste', 'Coordinateur', 'Commentateur', 'Intercom'],
};
export const TASK_STATUS: Record<TaskStatus, { label: string; chip: string }> = {
  todo: { label: 'À faire', chip: 'info' }, in_progress: { label: 'En cours', chip: 'accent' }, done: { label: 'Terminé', chip: 'ok' }, validated: { label: 'Validé', chip: 'ok' },
};
export const PRIORITY: Record<Priority, { label: string; chip: string }> = {
  low: { label: 'Basse', chip: 'info' }, normal: { label: 'Normale', chip: 'info' }, high: { label: 'Haute', chip: 'warn' }, urgent: { label: 'Urgente', chip: 'err' },
};
export const U_PRESETS = [6, 9, 12, 18, 24, 30, 36, 42, 45, 48];

/** Date clé : début de l'exploitation (ou ancienne date de mise en service). */
export const keyDate = (p: ProjectData) => p.shoot_start ?? p.commissioning;
/** Période d'exploitation lisible : « 15 nov. → 16 nov. », un seul jour si identiques. */
export function period(p: ProjectData): string {
  const a = p.shoot_start ?? p.commissioning, b = p.shoot_end ?? a;
  if (!a) return 'dates à fixer';
  return a === b ? fdate(a) : `${fdate(a)} → ${fdate(b)}`;
}

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
  const empty: ElementDetails = { surface: null, area_m2: null, camera_number: null, mount: null, link: null, mast_height_m: null, frequencies: null,
    power_kva: null, phases: null, outlet: null, provider: null, feed: null, circuit_ref: null, point_kind: null, display_kind: null, diagonal_in: null,
    resolution: null, cable_length_m: null, drum_count: null };
  const details: ElementDetails = { ...empty, ...(kind === 'monitor_wall' ? { wall: { rows: 2, cols: 3, tiles: [], support: null } }
    : kind === 'desk' ? { positions: [{ id: 'p1', role: 'Réalisateur', name: '', instance_ids: [] }, { id: 'p2', role: 'Scripte', name: '', instance_ids: [] }] }
    : kind === 'operator_position' ? { positions: [{ id: 'p1', role: 'Commentateur', name: '', instance_ids: [] }] }
    : kind === 'power' ? { phases: 3 } : {}) };
  if (!k.canRack && !['waterproof_case', 'furniture'].includes(kind)) return { kind, rackable: false, height_u: null, depth_mm: null, rear_rails_mm: null, length_mm: null, width_mm: null, outer_height_mm: null, wheels: false, empty_weight_kg: null, max_weight_kg: null, max_power_w: null, cable_code: '', notes: null, details };
  return { details, kind, rackable: k.rackable, height_u: k.rackable ? (kind === 'rack' ? 42 : 12) : null, depth_mm: k.rackable ? 600 : 400, rear_rails_mm: null,
    length_mm: k.rackable ? null : 600, width_mm: k.rackable ? null : 400, outer_height_mm: null, wheels: false, empty_weight_kg: null, max_weight_kg: null, max_power_w: null, cable_code: '', notes: null };
}
