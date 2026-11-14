// Comportement commun des boîtes de dialogue : seule la boîte du dessus réagit à Échap, la
// saisie commencée n'est jamais perdue par un geste involontaire (Échap, clic à côté, sélection
// de texte relâchée hors de la boîte), le focus reste dans la boîte et revient à l'élément
// d'origine à la fermeture.

const stack: symbol[] = [];
/** Une boîte de dialogue est-elle ouverte ? (les raccourcis des écrans s'effacent derrière). */
export const modalOpen = () => stack.length > 0;

export interface ModalOptions {
  /** Demande de fermeture (Échap, fond, croix). */
  close: () => void;
  /** Fermeture interdite par Échap et par le fond (information à ne pas perdre). */
  locked?: () => boolean;
  /** Saisie en cours : on demande avant de fermer. */
  ask: () => void;
}

/** Action Svelte à poser sur le fond (.modal). Le premier enfant .dialog est la boîte. */
export function modal(node: HTMLElement, opts: ModalOptions) {
  const id = Symbol('modal');
  stack.push(id);
  const before = document.activeElement as HTMLElement | null;
  const box = () => node.querySelector<HTMLElement>('.dialog') ?? node;
  let touched = false, downOnBackdrop = false;
  const onInput = () => (touched = true);
  const request = () => {
    if (opts.locked?.()) return;
    if (touched) opts.ask(); else opts.close();
  };
  const onKey = (e: KeyboardEvent) => {
    if (stack[stack.length - 1] !== id) return;
    if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); request(); return; }
    if (e.key === 'Tab') {
      // Le focus tourne dans la boîte.
      const f = [...box().querySelectorAll<HTMLElement>('a[href],button:not([disabled]),input:not([disabled]):not([type=hidden]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])')].filter((x) => x.offsetParent !== null);
      if (!f.length) return;
      const first = f[0]!, last = f[f.length - 1]!;
      if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
    }
  };
  const onDown = (e: PointerEvent) => { downOnBackdrop = e.target === node; };
  const onClick = (e: MouseEvent) => { if (e.target === node && downOnBackdrop) request(); downOnBackdrop = false; };
  node.addEventListener('input', onInput, true);
  node.addEventListener('change', onInput, true);
  node.addEventListener('pointerdown', onDown);
  node.addEventListener('click', onClick);
  window.addEventListener('keydown', onKey, true);
  // Focus initial : [data-autofocus], sinon premier champ, sinon bouton principal — jamais un
  // bouton de suppression (on ne confirme pas une destruction en appuyant sur Entrée).
  queueMicrotask(() => {
    const b = box();
    const el = b.querySelector<HTMLElement>('[data-autofocus]')
      ?? b.querySelector<HTMLElement>('.dlg-body input:not([type=checkbox]):not([disabled]),.dlg-body select,.dlg-body textarea,.wz-body input:not([type=checkbox]),.wz-body select,.wz-body textarea')
      ?? (b.querySelector('.danger-btn') ? b.querySelector<HTMLElement>('.wz-foot .btn.ghost') : b.querySelector<HTMLElement>('button.primary'));
    el?.focus();
  });
  return {
    update(o: ModalOptions) { opts = o; },
    /** Remet la boîte à « pas de saisie en cours » (après enregistrement par exemple). */
    destroy() {
      const i = stack.indexOf(id); if (i >= 0) stack.splice(i, 1);
      node.removeEventListener('input', onInput, true);
      node.removeEventListener('change', onInput, true);
      node.removeEventListener('pointerdown', onDown);
      node.removeEventListener('click', onClick);
      window.removeEventListener('keydown', onKey, true);
      if (before && document.contains(before)) before.focus();
    },
  };
}
