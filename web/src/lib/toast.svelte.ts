import { ApiError } from './api';

export type ToastKind = 'ok' | 'info' | 'warn' | 'err';
/** `undo` : action proposée dans la notification (par exemple annuler une suppression). */
export interface Toast { id: number; kind: ToastKind; text: string; undo?: { label: string; run: () => void } }

export const toasts = $state<Toast[]>([]);
let n = 0;

export function dismiss(id: number) { const i = toasts.findIndex((t) => t.id === id); if (i >= 0) toasts.splice(i, 1); }

/** Notification : les confirmations s'effacent seules ; les alertes et erreurs restent jusqu'à
 *  ce qu'on les ferme (une consigne longue n'est jamais perdue), sauf les plus courtes. */
export function toast(text: string, kind: ToastKind = 'ok', undo?: Toast['undo']) {
  const id = ++n;
  toasts.push({ id, kind, text, undo });
  const sticky = (kind === 'err' || kind === 'warn') && text.length > 60;
  if (!sticky) setTimeout(() => dismiss(id), undo ? 7000 : kind === 'err' ? 8000 : kind === 'warn' ? 6000 : 3600);
  return id;
}

/** Affiche l'erreur d'une action, sans détail technique. */
export function fail(e: unknown) {
  if (e instanceof ApiError) { toast(e.message, 'err'); return; }
  console.error(e);
  toast('Une erreur inattendue est survenue. Réessayez ; si elle persiste, rechargez la page.', 'err');
}
