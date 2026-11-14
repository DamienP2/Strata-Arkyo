import { api, ApiError, onUnauthenticated } from './api';
import type { Domain, Level, Me } from './types';

const ORDER: Level[] = ['aucun', 'lecture', 'ecriture', 'admin'];

export const session = $state<{ me: Me | null; ready: boolean; setupNeeded: boolean; error: string | null; expired: boolean }>({
  me: null, ready: false, setupNeeded: false, error: null, expired: false,
});

export async function loadSession() {
  try {
    session.me = await api.get<Me>('/me');
    session.error = null;
  } catch (e) {
    session.me = null;
    if (e instanceof ApiError && e.status === 401) {
      try {
        session.setupNeeded = (await api.get<{ needed: boolean }>('/setup')).needed;
      } catch (x) {
        session.error = (x as Error).message;
      }
    } else {
      session.error = (e as Error).message;
    }
  }
  session.ready = true;
}

// Session expirée en cours de travail : l'écran reste en place (saisies comprises) et une
// fenêtre de reconnexion s'affiche par-dessus.
onUnauthenticated(() => { if (session.me) session.expired = true; else session.me = null; });

/** Le rôle de la personne connectée atteint-il ce niveau sur ce domaine ? */
export function can(domain: Domain, level: Level): boolean {
  const me = session.me;
  if (!me) return false;
  if (me.is_owner) return true;
  return ORDER.indexOf(me.permissions[domain]) >= ORDER.indexOf(level);
}
