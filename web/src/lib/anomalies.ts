// Anomalies regroupées par type : un compteur lisible (« 56 câbles ont une extrémité
// manquante ») et, dépliée, une ligne par objet nommé (câble, port, nœud…).

export type Severity = 'error' | 'warning' | 'info';
export interface Anomaly<T = unknown> {
  /** Type d'anomalie (titre côté serveur) : sert à regrouper. */
  group: string;
  severity: Severity | string;
  /** Ce dont il s'agit : numéro du câble, à défaut « Câble non numéroté », port, nœud… */
  subject: string;
  /** Où le trouver : extrémité connue « Équipement › port », modèle… */
  place?: string | null;
  detail: string;
  href?: string;
  /** Objet d'origine, rendu au clic. */
  src?: T;
}
export interface AnomalyGroup<T = unknown> { key: string; severity: Severity; label: string; items: Anomaly<T>[] }

/** Phrase du compteur, au singulier et au pluriel. */
const PHRASES: Record<string, [string, string]> = {
  'Extrémité manquante': ['câble a une extrémité manquante', 'câbles ont une extrémité manquante'],
  'Numéro en double': ['numéro est porté par plusieurs câbles', 'numéros sont portés par plusieurs câbles'],
  'Port surutilisé': ['port porte plusieurs câbles', 'ports portent plusieurs câbles'],
  'Incompatibilité': ['câble relie des ports incompatibles', 'câbles relient des ports incompatibles'],
  'Attache incompatible': ['attache d’adaptateur est incompatible', 'attaches d’adaptateur sont incompatibles'],
  'Câble sans numéro': ['câble n’a pas de numéro', 'câbles n’ont pas de numéro'],
  'Emplacement vide': ['emplacement est vide', 'emplacements sont vides'],
  'Équipement non câblé': ['équipement n’est pas câblé', 'équipements ne sont pas câblés'],
  'Panel non câblé': ['panel n’est pas câblé', 'panels ne sont pas câblés'],
  'Panel mal câblé': ['panel n’est câblé que d’un côté', 'panels ne sont câblés que d’un côté'],
  'Adaptateur orphelin': ['adaptateur n’est rattaché à rien', 'adaptateurs ne sont rattachés à rien'],
  'Zone sans nom': ['zone n’a pas de nom', 'zones n’ont pas de nom'],
  'Zone vide': ['zone est vide', 'zones sont vides'],
  'Non posé en rack': ['exemplaire n’est pas encore posé en rack', 'exemplaires ne sont pas encore posés en rack'],
  'À vérifier': ['câble à vérifier', 'câbles à vérifier'],
  'Réseau': ['point réseau à corriger', 'points réseau à corriger'],
  'DMX': ['conflit d’adresses DMX', 'conflits d’adresses DMX'],
  'Fiche': ['fiche à compléter', 'fiches à compléter'],
};
export const groupLabel = (key: string, n: number) => {
  const p = PHRASES[key];
  return p ? `${n} ${n > 1 ? p[1] : p[0]}` : `${key} (${n})`;
};

const RANK: Record<string, number> = { error: 0, warning: 1, info: 2 };
/** Regroupe en gardant l'ordre de gravité, puis d'apparition. */
export function groupAnomalies<T>(list: Anomaly<T>[]): AnomalyGroup<T>[] {
  const m = new Map<string, Anomaly<T>[]>();
  for (const a of list) { const g = m.get(a.group); if (g) g.push(a); else m.set(a.group, [a]); }
  return [...m.entries()]
    .map(([key, items]) => {
      const severity = items.reduce<Severity>((s, i) => ((RANK[i.severity] ?? 2) < RANK[s]! ? (i.severity as Severity) : s), 'info');
      return { key, severity, label: groupLabel(key, items.length), items };
    })
    .sort((a, b) => RANK[a.severity]! - RANK[b.severity]!);
}
