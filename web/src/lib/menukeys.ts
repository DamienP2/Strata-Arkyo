// Clavier d'un menu déroulant (role="menu") : focus sur l'article courant (aria-current ou
// aria-checked) ou le premier à l'ouverture, flèches haut/bas (et Début/Fin) pour parcourir,
// Échap ou Tab pour fermer. Échap rend le focus au bouton qui a ouvert le menu.
// Le composant Menu.svelte l'utilise ; le poser directement n'est utile que pour un menu maison.

export interface MenuKeysOptions {
  close: () => void;
  /** Bouton d'ouverture, à refocaliser après Échap (par défaut : l'élément actif à l'ouverture). */
  trigger?: HTMLElement | null;
}

export function menuKeys(node: HTMLElement, opts: MenuKeysOptions) {
  const initial = document.activeElement as HTMLElement | null;
  const items = () => [...node.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled]),[role="menuitemradio"]:not([disabled]),[role="menuitemcheckbox"]:not([disabled])')].filter((x) => x.offsetParent !== null);
  queueMicrotask(() => {
    const l = items();
    (l.find((x) => x.getAttribute('aria-current') === 'page' || x.getAttribute('aria-checked') === 'true') ?? l[0])?.focus();
  });
  const onKey = (e: KeyboardEvent) => {
    const l = items();
    const i = l.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'ArrowDown') { e.preventDefault(); l[(i + 1) % l.length]?.focus(); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); l[(i - 1 + l.length) % l.length]?.focus(); }
    else if (e.key === 'Home') { e.preventDefault(); l[0]?.focus(); }
    else if (e.key === 'End') { e.preventDefault(); l[l.length - 1]?.focus(); }
    else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); opts.close(); (opts.trigger ?? initial)?.focus(); }
    else if (e.key === 'Tab') opts.close();
  };
  node.addEventListener('keydown', onKey);
  return {
    update(o: MenuKeysOptions) { opts = o; },
    destroy() { node.removeEventListener('keydown', onKey); },
  };
}
