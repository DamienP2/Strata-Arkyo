// Raccourcis communs des canevas (charte § 5) : Échap désélectionne, Suppr ou Retour arrière
// supprime la sélection, Ctrl+Z / Ctrl+Y (ou Ctrl+Maj+Z) annulent et rétablissent, Ctrl+A
// sélectionne tout. Rien ne se déclenche quand une boîte est ouverte (modalOpen) ni pendant
// une saisie dans un champ. Usage : `$effect(() => canvasKeys({ onEscape, onDelete, onUndo, onRedo }))`.

import { modalOpen } from './modal';

export interface CanvasKeysOptions {
  /** Désélectionner et revenir à l'outil Sélection. */
  onEscape?: () => void;
  /** Supprimer la sélection (Suppr, Retour arrière). */
  onDelete?: () => void;
  onUndo?: () => void;
  onRedo?: () => void;
  onSelectAll?: () => void;
  /** Autres touches propres à l'écran ; renvoyer true si la touche a été traitée (preventDefault). */
  onKey?: (e: KeyboardEvent) => boolean | void;
  /** Raccourcis actifs ? (par exemple : seulement en écriture, ou si le canevas est visible). */
  enabled?: () => boolean;
}

/** La touche vient-elle d'une saisie (champ, zone de texte, liste, contenu éditable) ? */
export function isTyping(e: Event): boolean {
  const t = e.target as HTMLElement | null;
  if (!t || !t.closest) return false;
  return !!t.closest('input, textarea, select, [contenteditable=""], [contenteditable="true"]');
}

/** Le raccourci d'écran doit-il être ignoré ? (boîte ouverte, saisie en cours, menu ouvert). */
export function shortcutBlocked(e: KeyboardEvent): boolean {
  if (modalOpen() || e.defaultPrevented || isTyping(e)) return true;
  return !!(e.target as HTMLElement | null)?.closest?.('[role="menu"]');
}

/** Branche les raccourcis sur la fenêtre et renvoie la fonction qui les retire. */
export function canvasKeys(o: CanvasKeysOptions): () => void {
  const on = (e: KeyboardEvent) => {
    if (shortcutBlocked(e) || (o.enabled && !o.enabled())) return;
    const mod = e.ctrlKey || e.metaKey, k = e.key.toLowerCase();
    let done = true;
    if (e.key === 'Escape' && o.onEscape) o.onEscape();
    else if ((e.key === 'Delete' || e.key === 'Backspace') && !mod && o.onDelete) o.onDelete();
    else if (mod && k === 'z' && !e.shiftKey && o.onUndo) o.onUndo();
    else if (mod && (k === 'y' || (k === 'z' && e.shiftKey)) && o.onRedo) o.onRedo();
    else if (mod && k === 'a' && o.onSelectAll) o.onSelectAll();
    else done = !!o.onKey?.(e);
    if (done) e.preventDefault();
  };
  addEventListener('keydown', on);
  return () => removeEventListener('keydown', on);
}
