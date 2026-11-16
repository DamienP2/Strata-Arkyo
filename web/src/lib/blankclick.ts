// Clic dans le vide d'un canevas : désélectionner (charte § 5, « Sélectionner »). Un clic sans
// mouvement (moins de 5 px entre l'appui et le relâchement) hors de tout élément interactif
// (élément du dessin, bouton, lien, champ), hors du panneau de droite, quand aucune boîte ni
// aucun menu n'est ouvert. Un glisser (orbite 3D, sélection de texte) ne désélectionne pas.
// Usage : `$effect(() => blankClick({ enabled: () => !!sel, onBlank: () => (sel = null) }))`.

import { modalOpen } from './modal';

/** Ce qui garde la sélection : éléments interactifs, panneau d'inspection, menus, notifications. */
const KEEP = 'button, a[href], input, select, textarea, label, summary, [contenteditable="true"], [role="button"], [role="tab"], [role="menu"], [role="menuitem"], [role="option"], [role="dialog"], [role="slider"], .pane.right, .toasts, [data-keep-sel]';

export interface BlankClickOptions {
  /** Écoute active ? (une sélection existe, pas de pose en cours…) */
  enabled: () => boolean;
  /** Clic dans le vide : désélectionner. */
  onBlank: () => void;
}

/** Branche l'écoute sur le document (phase de capture, avant la fermeture des menus) et renvoie la fonction qui la retire. */
export function blankClick(o: BlankClickOptions): () => void {
  let start: { x: number; y: number } | null = null;
  const down = (e: PointerEvent) => {
    start = null;
    if (e.button !== 0 || !o.enabled() || modalOpen() || document.querySelector('[role="menu"]')) return;
    const t = e.target as Element | null;
    if (!t?.closest || t.closest(KEEP)) return;
    start = { x: e.clientX, y: e.clientY };
  };
  const up = (e: PointerEvent) => {
    const s = start;
    start = null;
    if (!s || Math.hypot(e.clientX - s.x, e.clientY - s.y) > 5 || !o.enabled() || modalOpen()) return;
    o.onBlank();
  };
  const cancel = () => { start = null; };
  document.addEventListener('pointerdown', down, true);
  document.addEventListener('pointerup', up, true);
  document.addEventListener('pointercancel', cancel, true);
  return () => {
    document.removeEventListener('pointerdown', down, true);
    document.removeEventListener('pointerup', up, true);
    document.removeEventListener('pointercancel', cancel, true);
  };
}
