// Référentiels et fiches du catalogue, tels que les renvoie /api/catalog.

import { api } from './api';

export type RefPath = 'functions' | 'signals' | 'connectors' | 'brands' | 'system-categories' | 'categories' | 'types' | 'phases' | 'dimensions';
export type ItemPath = 'equipment' | 'cables' | 'cable-models' | 'adapters';

export interface RefItem {
  kind: string; id: string; name: string; parent_id: string | null; data: any; active: boolean;
  position: number; builtin: boolean; protected: boolean; rev: number; created_at: string; updated_at: string; uses: number;
}
export interface CatalogItem {
  id: string; kind: 'equipment' | 'cable' | 'cable_model' | 'adapter'; name: string; reference: string | null; brand_id: string | null;
  data: any; active: boolean; rev: number; created_at: string; updated_at: string;
}
export interface Duplicate { id: string; name: string; reference: string | null; reason: string }

export const ITEM_KINDS: { path: ItemPath; kind: CatalogItem['kind']; label: string; one: string; newLabel: string }[] = [
  { path: 'equipment', kind: 'equipment', label: 'Équipements', one: 'Équipement', newLabel: 'Nouvel équipement' },
  { path: 'cables', kind: 'cable', label: 'Câbles', one: 'Câble', newLabel: 'Nouveau câble' },
  { path: 'cable-models', kind: 'cable_model', label: 'Types de câble', one: 'Type de câble', newLabel: 'Nouveau type de câble' },
  { path: 'adapters', kind: 'adapter', label: 'Adaptateurs', one: 'Adaptateur', newLabel: 'Nouvel adaptateur' },
];
export const pathOf = (kind: CatalogItem['kind']) => ITEM_KINDS.find((k) => k.kind === kind)!.path;

export const REF_KINDS: { path: RefPath; label: string; one: string; help: string }[] = [
  { path: 'functions', label: 'Fonctions', one: 'Fonction', help: 'Grandes familles métier (vidéo, audio…). La clé sert aux numéros de câble.' },
  { path: 'signals', label: 'Signaux', one: 'Signal', help: 'Types de signaux, leurs connecteurs compatibles et leur préfixe de numérotation.' },
  { path: 'connectors', label: 'Connecteurs', one: 'Connecteur', help: 'Connecteurs disponibles sur les ports et les câbles.' },
  { path: 'brands', label: 'Marques', one: 'Marque', help: 'Fabricants du matériel.' },
  { path: 'categories', label: 'Catégories', one: 'Catégorie', help: 'Classement libre des équipements, sur un ou plusieurs niveaux, que vous gérez entièrement.' },
  { path: 'types', label: 'Types', one: 'Type', help: 'Types d’équipement, avec un niveau de sous-type qui hérite de la fonction.' },
  { path: 'phases', label: 'Phases', one: 'Phase', help: 'Étapes de la production où le matériel intervient.' },
  { path: 'dimensions', label: 'Dimensions', one: 'Préréglage', help: 'Valeurs proposées dans les formulaires : hauteurs, profondeurs, longueurs de câble…' },
  { path: 'system-categories', label: 'Catégories système', one: 'Catégorie système', help: 'Catégories protégées, fixées par l’application et utilisées par le moteur (calculs, numérotation) : elles ne se modifient pas. Pour votre propre classement, utilisez les Catégories.' },
];

export const FACES: [string, string][] = [['front', 'Avant'], ['rear', 'Arrière'], ['left', 'Gauche'], ['right', 'Droite'], ['top', 'Dessus'], ['bottom', 'Dessous']];
export const DIRECTIONS: [string, string][] = [['in', 'Entrée'], ['out', 'Sortie'], ['bidir', 'Bidirectionnel']];
export const WIDTHS: [string, string][] = [['full', 'Pleine'], ['half', '1/2'], ['third', '1/3'], ['quarter', '1/4']];

/** Référentiels chargés une fois, rechargés après une modification. */
export const refs = $state<{ byKind: Record<string, RefItem[]>; loaded: boolean }>({ byKind: {}, loaded: false });

let pending: Promise<void> | null = null;
export function loadRefs(force = false): Promise<void> {
  if (refs.loaded && !force) return Promise.resolve();
  if (!pending || force) {
    pending = api.get<Record<string, RefItem[]>>('/catalog/refs').then((r) => { refs.byKind = r; refs.loaded = true; }).finally(() => { pending = null; });
  }
  return pending;
}

export const refList = (k: RefPath, activeOnly = false) => (refs.byKind[k] ?? []).filter((r) => !activeOnly || r.active);
export const refName = (k: RefPath, id: string | null | undefined) => (id ? refs.byKind[k]?.find((r) => r.id === id)?.name ?? id : '');
export const refById = (k: RefPath, id: string | null | undefined) => (id ? refs.byKind[k]?.find((r) => r.id === id) : undefined);
/** Couleur d'une fonction (par sa clé) ou d'un signal. */
export function fnColor(key: string | null | undefined): string {
  return refById('functions', key)?.data?.color ?? 'var(--fg-3)';
}

/** Nouvelle fiche vide, conforme au modèle du moteur. */
export function blank(kind: CatalogItem['kind']): any {
  switch (kind) {
    case 'equipment':
      return { name: '', reference: null, brand_id: null, category_id: null, type_id: null, functions: [], phases: [], tags: [], notes: null, active: true,
        format: { kind: 'rack', height_u: 1, width: 'full', depth_mm: 0, rear_reserve_mm: null }, weight_kg: null, power_w: null, dual_psu: false, btu_h: null,
        ip: null, dmx: null, modular: null, ports: [], custom: [], documents: [] };
    case 'cable':
      return { name: '', reference: null, function: null, brand_id: null, model_id: null, left: { connector_id: 'bnc', gender: 'M' }, right: { connector_id: 'bnc', gender: 'M' },
        usages: [], lengths_m: [], custom_min_m: null, custom_max_m: null, notes: null, active: true };
    case 'cable_model':
      return { name: '', reference: null, function: null, brand_id: null, impedance_ohm: null, bandwidth_mhz: null, diameter_mm: null, conductor: null, shielding: null,
        jacket: null, fire_rating: null, custom: [], datasheet_url: null, active: true };
    case 'adapter':
      return { name: '', reference: null, function: null, notes: null, active: true, input: { connector_id: 'bnc', gender: 'F', label: null }, outputs: [{ connector_id: 'hd-bnc', gender: 'M', quantity: 1, label: null }] };
  }
}

/** Résumé d'une fiche pour les listes. */
export function summary(i: CatalogItem): string {
  const d = i.data;
  const n = (v: number | null | undefined, u: string) => (v == null ? '' : `${String(v).replace('.', ',')} ${u}`);
  switch (i.kind) {
    case 'equipment': {
      const f = d.format?.kind === 'rack' ? `${String(d.format.height_u).replace('.', ',')} U${d.format.width && d.format.width !== 'full' ? ' · ' + WIDTHS.find((w) => w[0] === d.format.width)?.[1] : ''}` : 'Hors rack';
      const ports = (d.ports ?? []).reduce((s: number, p: any) => s + (p.quantity ?? 1), 0);
      return [f, n(d.weight_kg, 'kg'), n(d.power_w, 'W'), ports ? `${ports} port${ports > 1 ? 's' : ''}` : ''].filter(Boolean).join(' · ');
    }
    case 'cable':
      return [refName('connectors', d.left?.connector_id) + ' ' + (d.left?.gender ?? '') + ' ↔ ' + refName('connectors', d.right?.connector_id) + ' ' + (d.right?.gender ?? ''), (d.lengths_m ?? []).map((l: number) => String(l).replace('.', ',') + ' m').join(', ')].filter(Boolean).join(' · ');
    case 'cable_model':
      return [n(d.impedance_ohm, 'Ω'), n(d.bandwidth_mhz, 'MHz'), n(d.diameter_mm, 'mm')].filter(Boolean).join(' · ');
    case 'adapter':
      return `${refName('connectors', d.input?.connector_id)} ${d.input?.gender ?? ''} → ${(d.outputs ?? []).map((o: any) => `${o.quantity > 1 ? o.quantity + ' × ' : ''}${refName('connectors', o.connector_id)} ${o.gender}`).join(', ')}`;
  }
}
