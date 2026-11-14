// Confirmation dans une boîte de l'application (remplace window.confirm) : `await ask({...})`
// renvoie true si l'utilisateur confirme. La boîte (components/Confirm.svelte) est rendue par
// components/AskHost.svelte, posé une fois dans App.svelte.

export interface AskOptions {
  title: string;
  /** Texte d'explication (facultatif). */
  text?: string;
  /** Verbe d'action du bouton (« Supprimer », « Décâbler »…). */
  confirmLabel: string;
  danger?: boolean;
  eyebrow?: string;
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
