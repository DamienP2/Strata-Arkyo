export type ToastKind = 'ok' | 'info' | 'warn' | 'err';
export interface Toast { id: number; kind: ToastKind; text: string }

export const toasts = $state<Toast[]>([]);
let n = 0;

export function toast(text: string, kind: ToastKind = 'ok') {
  const id = ++n;
  toasts.push({ id, kind, text });
  setTimeout(() => {
    const i = toasts.findIndex((t) => t.id === id);
    if (i >= 0) toasts.splice(i, 1);
  }, kind === 'err' ? 6000 : 3600);
}

/** Affiche l'erreur d'une action, sans détail technique. */
export function fail(e: unknown) {
  toast(e instanceof Error ? e.message : 'Une erreur est survenue.', 'err');
}
