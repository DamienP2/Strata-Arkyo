// Clic dehors : appelle `handler` quand on appuie (pointerdown) hors du nœud, pour fermer un
// menu ou un popover. Action Svelte `use:outside={() => (open = false)}` ou fonction onOutside().

export type OutsideHandler = (e: PointerEvent) => void;

/** Branche l'écoute et renvoie la fonction qui la retire (usage hors composant, ou dans un $effect). */
export function onOutside(node: HTMLElement, handler: OutsideHandler): () => void {
  const on = (e: PointerEvent) => { if (!node.contains(e.target as Node)) handler(e); };
  document.addEventListener('pointerdown', on, true);
  return () => document.removeEventListener('pointerdown', on, true);
}

/** Action Svelte : `<div use:outside={() => (open = false)}>`. */
export function outside(node: HTMLElement, handler: OutsideHandler) {
  let h = handler;
  const off = onOutside(node, (e) => h(e));
  return {
    update(n: OutsideHandler) { h = n; },
    destroy: off,
  };
}
