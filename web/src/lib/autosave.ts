// Enregistrement automatique des canevas, panneaux et cellules (extrait de Drawings.svelte) :
// `changed()` après chaque modification (sur `change`, jamais sur `input`), écriture 700 ms
// après la dernière, vidage à la destruction du composant et à la fermeture de l'onglet,
// nouvel essai toutes les 5 s si le réseau manque, choix proposé en cas de conflit (409).
// L'objet est un store Svelte de l'état : `<SaveState status={$saver} />`.

import { ApiError } from './api';
import { ask } from './ask.svelte';
import { fail } from './toast.svelte';
import { onDestroy } from 'svelte';

export type SaveStatus = 'idle' | 'dirty' | 'saving' | 'saved' | 'error' | 'conflict';

export interface AutoSaveOptions {
  /** Écrit l'état courant (lève ApiError en cas d'échec). */
  save: () => Promise<unknown>;
  /** Délai après la dernière modification, en ms (700 par défaut). */
  delay?: number;
  /** Intervalle des nouveaux essais après une panne réseau, en ms (5000 par défaut). */
  retry?: number;
  /** Conflit « garder ma version » : relire la révision du serveur avant de réécrire. Sans elle, le conflit recharge. */
  keepMine?: () => Promise<unknown>;
  /** Recharge la version du serveur (conflit « reprendre la version enregistrée », ou échec définitif). */
  reload?: () => Promise<unknown> | unknown;
}

export interface AutoSave {
  /** Contrat de store Svelte : `$saver` vaut l'état courant. */
  subscribe(fn: (s: SaveStatus) => void): () => void;
  readonly status: SaveStatus;
  /** Des modifications attendent-elles d'être écrites ? */
  readonly dirty: boolean;
  /** À appeler après chaque modification validée. */
  changed(): void;
  /** Écrit tout de suite ce qui attend (avant un export, un changement de page…). */
  flush(): Promise<void>;
  /** Oublie ce qui attend (après un rechargement). */
  reset(): void;
  /** Vide puis retire les écouteurs ; appelé seul à la destruction du composant qui l'a créé. */
  dispose(): void;
}

const retryable = (e: unknown) => e instanceof ApiError && (e.status === 0 || e.status >= 500);
const isConflict = (e: unknown) => e instanceof ApiError && (e.code === 'stale' || e.status === 409);

export function autosave(o: AutoSaveOptions): AutoSave {
  const delay = o.delay ?? 700, retryMs = o.retry ?? 5000;
  let status: SaveStatus = 'idle', dirty = false, running: Promise<void> | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null, retryT: ReturnType<typeof setTimeout> | null = null;
  const subs = new Set<(s: SaveStatus) => void>();
  const set = (s: SaveStatus) => { status = s; subs.forEach((f) => f(s)); };
  const clear = () => { if (timer) { clearTimeout(timer); timer = null; } if (retryT) { clearTimeout(retryT); retryT = null; } };

  async function conflict() {
    set('conflict');
    const mine = await ask({ title: 'Modifié ailleurs entre-temps', text: 'Quelqu’un a enregistré une autre version pendant votre modification. Gardez la vôtre (elle remplace l’autre) ou reprenez la version enregistrée.', confirmLabel: 'Garder ma version', cancelLabel: 'Reprendre la version enregistrée' });
    try {
      if (mine && o.keepMine) { await o.keepMine(); dirty = true; await persist(); }
      else { dirty = false; set('idle'); await o.reload?.(); }
    } catch (e) { fail(e); }
  }

  async function persist(): Promise<void> {
    if (running) { await running; if (!dirty) return; }
    if (!dirty) return;
    clear();
    dirty = false; set('saving');
    running = (async () => {
      try { await o.save(); set(dirty ? 'dirty' : 'saved'); }
      catch (e) {
        dirty = true;
        if (isConflict(e)) { running = null; await conflict(); return; }
        if (retryable(e)) { set('error'); retryT = setTimeout(() => { retryT = null; if (dirty) void persist(); }, retryMs); return; }
        // Refus du serveur (validation, droits) : on le dit et on revient à la version enregistrée.
        set('error'); dirty = false; fail(e); await o.reload?.();
      }
    })();
    try { await running; } finally { running = null; }
    if (dirty && status === 'dirty') schedule();
  }
  function schedule() { if (timer) clearTimeout(timer); timer = setTimeout(() => { timer = null; void persist(); }, delay); }

  const onUnload = (e: BeforeUnloadEvent) => { if (dirty || running) { e.preventDefault(); e.returnValue = ''; void persist(); } };
  addEventListener('beforeunload', onUnload);

  const api: AutoSave = {
    subscribe(fn) { fn(status); subs.add(fn); return () => { subs.delete(fn); }; },
    get status() { return status; },
    get dirty() { return dirty; },
    changed() { dirty = true; set('dirty'); schedule(); },
    flush: persist,
    reset() { clear(); dirty = false; set('idle'); },
    dispose() { removeEventListener('beforeunload', onUnload); if (timer) { clearTimeout(timer); timer = null; } if (retryT) { clearTimeout(retryT); retryT = null; } if (dirty) void persist(); },
  };
  // Créé pendant l'initialisation d'un composant : vidage automatique à sa destruction.
  try { onDestroy(() => api.dispose()); } catch { /* hors composant : dispose() à appeler soi-même */ }
  return api;
}
