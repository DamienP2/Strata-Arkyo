// Textes des équipements dans les élévations : en premier le libellé personnalisé de
// l'exemplaire (inventaire), à défaut son identifiant ; en plus petit, la marque et le modèle.
// Réglage mémorisé dans le navigateur.
export type RackText = 'both' | 'label' | 'model' | 'none';
export const RACK_TEXT: Record<RackText, string> = { both: 'Libellé + modèle', label: 'Libellé seul', model: 'Modèle seul', none: 'Aucun' };

const read = (): RackText => {
  try { const v = localStorage.getItem('rack-text'); return v === 'name' ? 'model' : v === 'custom' ? 'label' : v && v in RACK_TEXT ? (v as RackText) : 'both'; } catch { return 'both'; }
};
export const rackText = $state<{ v: RackText }>({ v: read() });
export function setRackText(v: RackText) { rackText.v = v; try { localStorage.setItem('rack-text', v); } catch { /* stockage indisponible */ } }

interface Ref { label: string; custom?: string | null; tag?: string | null; brand?: string | null; nature?: string; accessory?: { kind: string } | null }
/** Marque et modèle (sans répéter la marque si le nom la contient déjà). */
export const modelOf = (e: Ref) => { const b = e.brand?.trim(); return b && !e.label.toLowerCase().startsWith(b.toLowerCase()) ? `${b} ${e.label}` : e.label; };
/** Lignes à afficher : [principale, secondaire en plus petit]. Les accessoires n'ont que leur nom. */
export function rackLines(e: Ref, mode: RackText = rackText.v): [string, string] {
  if (mode === 'none') return ['', ''];
  // Plaques, passe-câbles, étagères, tiroirs et barres de laçage : la façade suffit, pas de texte.
  if (e.nature === 'accessory') return e.accessory && e.accessory.kind !== 'panel' ? ['', ''] : [e.label, ''];
  const main = e.custom?.trim() || e.tag || '', model = modelOf(e);
  if (mode === 'model') return [model, ''];
  if (mode === 'label') return [main || model, ''];
  return main ? [main, model] : [model, ''];
}
