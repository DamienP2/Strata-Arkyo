// Comportement commun des boîtes de dialogue : seule la boîte du dessus réagit à Échap, la
// saisie commencée n'est jamais perdue par un geste involontaire (Échap, clic à côté, sélection
// de texte relâchée hors de la boîte), le focus reste dans la boîte et revient à l'élément
// d'origine à la fermeture (ou, s'il a disparu, au titre de la page, jamais à body).

import { tick } from 'svelte';

interface Entry { id: symbol; box: () => HTMLElement }
const stack: Entry[] = [];
/** Une boîte de dialogue est-elle ouverte ? (les raccourcis des écrans s'effacent derrière). */
export const modalOpen = () => stack.length > 0;

export interface ModalOptions {
  /** Demande de fermeture (Échap, fond, croix). */
  close: () => void;
  /** Fermeture interdite par Échap et par le fond (information à ne pas perdre, envoi en cours). */
  locked?: () => boolean;
  /** Saisie en cours : on demande avant de fermer. Sans cette option, la boîte se ferme. */
  ask?: () => void;
  /** Saisie commencée, fournie par la boîte (remplace le suivi automatique des champs). */
  dirty?: () => boolean;
}

const FOCUSABLE = 'a[href],button:not([disabled]),input:not([disabled]):not([type=hidden]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])';
const visible = (x: HTMLElement) => x.offsetParent !== null || x.getClientRects().length > 0;
const focusables = (b: HTMLElement) => [...b.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((x) => visible(x) && x.getAttribute('tabindex') !== '-1');
const isDanger = (x: Element) => x.matches('.danger');

/** Champ qui ne compte pas comme une saisie (recherche, filtre) : `type=search` ou `[data-no-dirty]`. */
export function ignoredInput(t: EventTarget | null): boolean {
  const el = t as HTMLElement | null;
  if (!el || !el.closest) return true;
  return (el as HTMLInputElement).type === 'search' || !!el.closest('[data-no-dirty]');
}

/** Focus de repli quand l'élément d'origine a disparu : titre de la page, sinon la zone principale. */
function focusPage() {
  const h = document.querySelector<HTMLElement>('#view h1') ?? document.querySelector<HTMLElement>('h1');
  if (h) { if (!h.hasAttribute('tabindex')) h.setAttribute('tabindex', '-1'); h.focus({ preventScroll: true }); return; }
  document.getElementById('view')?.focus({ preventScroll: true });
}

/** Action Svelte à poser sur le fond (.modal). Le premier enfant .dialog (ou [role=dialog]) est la boîte. */
export function modal(node: HTMLElement, opts: ModalOptions) {
  const id = Symbol('modal');
  const box = () => node.querySelector<HTMLElement>('.dialog, [role="dialog"]') ?? node;
  stack.push({ id, box });
  const before = document.activeElement as HTMLElement | null;
  // Clé stable du déclencheur, pour le retrouver s'il a été recréé entre-temps.
  const beforeKey = before?.id ? `#${CSS.escape(before.id)}` : before?.dataset?.focusKey ? `[data-focus-key="${CSS.escape(before.dataset.focusKey)}"]` : null;
  let touched = false, downOnBackdrop = false;
  const onInput = (e: Event) => { if (!ignoredInput(e.target)) touched = true; };
  const request = () => {
    if (opts.locked?.()) return;
    const dirty = opts.dirty ? opts.dirty() : touched;
    if (dirty && opts.ask) opts.ask(); else opts.close();
  };
  const onKey = (e: KeyboardEvent) => {
    if (stack[stack.length - 1]?.id !== id) return;
    if (e.key === 'Escape') {
      // Un menu ouvert dans la boîte se ferme d'abord (il gère Échap lui-même).
      if ((e.target as Element | null)?.closest?.('[role="menu"], [aria-haspopup][aria-expanded="true"]')) return;
      e.preventDefault(); e.stopPropagation(); request(); return;
    }
    if (e.key === 'Tab') {
      // Le focus tourne dans la boîte, et y revient s'il en est sorti.
      const b = box(), f = focusables(b);
      if (!f.length) { e.preventDefault(); b.focus(); return; }
      const first = f[0]!, last = f[f.length - 1]!;
      if (!b.contains(document.activeElement)) { e.preventDefault(); (e.shiftKey ? last : first).focus(); }
      else if (e.shiftKey && (document.activeElement === first || document.activeElement === b)) { e.preventDefault(); last.focus(); }
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
  // Focus initial : [data-autofocus], sinon premier champ, sinon bouton principal, sinon premier
  // élément focalisable, sinon la boîte elle-même — jamais un bouton de suppression (on ne
  // confirme pas une destruction en appuyant sur Entrée).
  queueMicrotask(() => {
    const b = box();
    if (!b.hasAttribute('tabindex')) b.setAttribute('tabindex', '-1');
    const field = [...b.querySelectorAll<HTMLElement>('.dlg-body :is(input,select,textarea), .wz-body :is(input,select,textarea)')]
      .find((x) => !(x as HTMLInputElement).disabled && !['checkbox', 'radio', 'hidden'].includes((x as HTMLInputElement).type) && visible(x));
    const primary = [...b.querySelectorAll<HTMLElement>('button.primary:not([disabled])')].find((x) => !isDanger(x) && visible(x));
    const safe = focusables(b).find((x) => !isDanger(x) && !x.classList.contains('icon-btn'));
    const el = b.querySelector<HTMLElement>('[data-autofocus]') ?? field ?? primary ?? safe ?? b;
    if (!b.contains(document.activeElement) || document.activeElement === b) el.focus();
  });
  return {
    update(o: ModalOptions) { opts = o; },
    destroy() {
      const i = stack.findIndex((s) => s.id === id); if (i >= 0) stack.splice(i, 1);
      node.removeEventListener('input', onInput, true);
      node.removeEventListener('change', onInput, true);
      node.removeEventListener('pointerdown', onDown);
      node.removeEventListener('click', onClick);
      window.removeEventListener('keydown', onKey, true);
      // Rendre le focus : au déclencheur s'il est toujours là, sinon à son remplaçant (même id
      // ou même data-focus-key), sinon au titre de la page. Si une autre boîte reste ouverte,
      // on ne lui vole pas le focus.
      void tick().then(() => {
        const top = stack[stack.length - 1];
        const target = before && before.isConnected && visible(before) ? before : beforeKey ? document.querySelector<HTMLElement>(beforeKey) : null;
        if (top) { const b = top.box(); if (target && b.contains(target) && !b.contains(document.activeElement)) target.focus(); return; }
        if (target && target !== document.body) target.focus();
        else if (!document.activeElement || document.activeElement === document.body) focusPage();
      });
    },
  };
}
