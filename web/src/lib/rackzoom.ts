// Zoom des élévations (vue d'un rack, racks côte à côte, casquette) : paliers de 25 à 400 %,
// ou « Ajuster » (toutes les vues dans la place disponible). Niveau mémorisé par écran.

export const ZMIN = 0.25;
export const ZMAX = 4;
/** Paliers des boutons − / + et des raccourcis Ctrl+= / Ctrl+-. */
export const ZOOM_STEPS = [0.25, 0.33, 0.5, 0.67, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4];

/** Palier suivant (dir = 1) ou précédent (dir = -1) depuis un zoom quelconque. */
export function zoomStep(z: number, dir: 1 | -1): number {
  if (dir > 0) return ZOOM_STEPS.find((s) => s > z + 0.005) ?? ZMAX;
  return [...ZOOM_STEPS].reverse().find((s) => s < z - 0.005) ?? ZMIN;
}

/** Niveau mémorisé (`fit` ou un nombre) ; à défaut, l'ancien réglage « Ajuster » des racks. */
export function readZoom(key: string): number | 'fit' {
  try {
    const s = localStorage.getItem(key);
    if (s === 'fit') return 'fit';
    const n = Number(s);
    if (s && Number.isFinite(n) && n >= ZMIN && n <= ZMAX) return n;
    if (key === 'rack-zoom' && localStorage.getItem('rack-fit') === '0') return 1;
  } catch { /* stockage indisponible */ }
  return 'fit';
}

export function saveZoom(key: string, z: number | 'fit') {
  try { localStorage.setItem(key, String(z)); } catch { /* stockage indisponible */ }
}
