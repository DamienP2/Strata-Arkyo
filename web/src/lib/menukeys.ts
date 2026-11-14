// Clavier d'un menu déroulant (role="menu") : focus sur le premier article à l'ouverture,
// flèches haut/bas (et Début/Fin) pour parcourir, Échap ou Tab pour fermer. Échap rend le
// focus au bouton qui a ouvert le menu.

export interface MenuKeysOptions { close: () => void }

export function menuKeys(node: HTMLElement, opts: MenuKeysOptions) {
  const trigger = document.activeElement as HTMLElement | null;
  const items = () => [...node.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])')].filter((x) => x.offsetParent !== null);
  queueMicrotask(() => items()[0]?.focus());
  const onKey = (e: KeyboardEvent) => {
    const l = items();
    const i = l.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'ArrowDown') { e.preventDefault(); l[(i + 1) % l.length]?.focus(); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); l[(i - 1 + l.length) % l.length]?.focus(); }
    else if (e.key === 'Home') { e.preventDefault(); l[0]?.focus(); }
    else if (e.key === 'End') { e.preventDefault(); l[l.length - 1]?.focus(); }
    else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); opts.close(); trigger?.focus(); }
    else if (e.key === 'Tab') opts.close();
  };
  node.addEventListener('keydown', onKey);
  return {
    update(o: MenuKeysOptions) { opts = o; },
    destroy() { node.removeEventListener('keydown', onKey); },
  };
}
