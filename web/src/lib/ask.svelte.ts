// Confirmation dans une boîte de l'application (remplace window.confirm) : `await ask({...})`
// renvoie true si l'utilisateur confirme. La boîte (components/Confirm.svelte) est rendue par
// components/AskHost.svelte, posé une fois dans App.svelte.
// Avec `run`, la requête part depuis la boîte : elle reste ouverte avec l'indicateur pendant
// l'envoi (Annuler et Échap bloqués) ; en cas d'échec, l'erreur s'affiche et la boîte reste
// ouverte (nouvel essai, ou Annuler qui renvoie false).

export interface AskOptions {
  title: string;
  /** Texte d'explication (facultatif). */
  text?: string;
  /** Verbe d'action du bouton (« Supprimer », « Décâbler »…). */
  confirmLabel: string;
  /** Libellé du renoncement (par défaut « Annuler »). */
  cancelLabel?: string;
  /** Validation en danger plein (destruction) ; le focus initial va alors sur le renoncement. */
  danger?: boolean;
  eyebrow?: string;
  /** Action exécutée depuis la boîte avant sa fermeture (suppression…). */
  run?: () => Promise<unknown> | unknown;
}

interface Pending extends AskOptions { resolve: (ok: boolean) => void }

export const asking = $state<{ current: Pending | null }>({ current: null });

export function ask(o: AskOptions): Promise<boolean> {
  asking.current?.resolve(false);
  return new Promise((resolve) => { asking.current = { ...o, resolve }; });
}

export function answer(ok: boolean) {
  const p = asking.current;
  asking.current = null;
  p?.resolve(ok);
}

/** Validation depuis la boîte : exécute `run` (s'il y en a un), puis ferme. Une erreur remonte à la boîte. */
export async function confirmAsk() {
  const p = asking.current;
  if (!p) return;
  if (p.run) await p.run();
  if (asking.current === p) answer(true);
}
