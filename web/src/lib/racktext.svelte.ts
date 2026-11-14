// Textes des équipements dans les élévations : nom de l'équipement et libellé personnalisé de
// l'exemplaire (inventaire), jamais l'identifiant. Réglage mémorisé dans le navigateur.
export type RackText = 'both' | 'name' | 'custom' | 'none';
export const RACK_TEXT: Record<RackText, string> = { both: 'Nom + libellé', name: 'Nom', custom: 'Libellé', none: 'Aucun' };

const read = (): RackText => { try { const v = localStorage.getItem('rack-text'); return v && v in RACK_TEXT ? (v as RackText) : 'both'; } catch { return 'both'; } };
export const rackText = $state<{ v: RackText }>({ v: read() });
export function setRackText(v: RackText) { rackText.v = v; try { localStorage.setItem('rack-text', v); } catch { /* stockage indisponible */ } }

/** Lignes à afficher : [principale, secondaire]. En « Libellé », le nom remplace un libellé absent. */
export function rackLines(e: { label: string; custom?: string | null }, mode: RackText = rackText.v): [string, string] {
  const c = e.custom?.trim() || '';
  if (mode === 'none') return ['', ''];
  if (mode === 'name') return [e.label, ''];
  if (mode === 'custom') return [c || e.label, ''];
  return [e.label, c];
}
