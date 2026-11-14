// En-tête de page partagé par les espaces de travail (catalogue, administration) : la sous-page
// affichée y place son compteur et, par `use:toHead`, son action principale (à droite, sur la
// ligne du titre). Les boutons restent dans l'arbre de la sous-page : ils disparaissent avec elle.

let seq = 0;
export const pageSlot = $state<{ owner: number; count: string }>({ owner: 0, count: '' });

/** À appeler à l'initialisation d'une sous-page : `pageHead(() => ({ count: '12 fiches' }))`. */
export function pageHead(get: () => { count?: string }) {
  const id = ++seq;
  $effect.pre(() => {
    const v = get();
    pageSlot.owner = id; pageSlot.count = v.count ?? '';
  });
  $effect(() => () => {
    if (pageSlot.owner === id) { pageSlot.owner = 0; pageSlot.count = ''; }
  });
}

/** Action Svelte : déplace l'élément dans la zone d'actions de l'en-tête de page, s'il y en a une. */
export function toHead(node: HTMLElement) {
  const slot = document.querySelector('.ph-slot');
  if (slot) slot.appendChild(node);
  return { destroy() { node.remove(); } };
}

/** « 1 équipement », « 143 équipements ». */
export const plural = (n: number, one: string, many = one + 's') => `${n.toLocaleString('fr-FR')} ${n > 1 ? many : one}`;
