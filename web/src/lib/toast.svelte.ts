import { ApiError } from './api';

export type ToastKind = 'ok' | 'info' | 'warn' | 'err';
/** `undo` : annulation proposée dans la notification (par exemple après une suppression). Le
 *  bouton s'appelle toujours « Annuler » (`label` n'est gardé que pour compatibilité). `run` peut
 *  être asynchrone ; s'il renvoie `false`, le toast « Action annulée. » n'est pas affiché. */
export interface Toast { id: number; kind: ToastKind; text: string; undo?: { label?: string; run: () => unknown } }

export const toasts = $state<Toast[]>([]);
let n = 0;

export function dismiss(id: number) { const i = toasts.findIndex((t) => t.id === id); if (i >= 0) toasts.splice(i, 1); }

/** Durées (charte § 3.9) : ok 3,6 s, warn 6 s, err 8 s, avec annulation 7 s ; une alerte ou une
 *  erreur de plus de 60 caractères reste jusqu'à ce qu'on la ferme (une consigne longue n'est
 *  jamais perdue) ; `info` suit la durée de `ok`. */
export function toast(text: string, kind: ToastKind = 'ok', undo?: Toast['undo']) {
  const id = ++n;
  toasts.push({ id, kind, text, undo });
  const sticky = (kind === 'err' || kind === 'warn') && text.length > 60;
  if (!sticky) setTimeout(() => dismiss(id), undo ? 7000 : kind === 'err' ? 8000 : kind === 'warn' ? 6000 : 3600);
  return id;
}

/** Bouton « Annuler » d'une notification : exécute l'annulation, puis confirme « Action annulée. ». */
export async function runUndo(t: Toast) {
  if (!t.undo) return;
  dismiss(t.id);
  try {
    const r = await t.undo.run();
    if (r !== false) toast('Action annulée.', 'ok');
  } catch (e) { fail(e); }
}

/** Affiche l'erreur d'une action, sans détail technique. */
export function fail(e: unknown) {
  if (e instanceof ApiError) { toast(e.message, 'err'); return; }
  console.error(e);
  toast('Une erreur inattendue est survenue. Réessayez ; si elle persiste, rechargez la page.', 'err');
}
