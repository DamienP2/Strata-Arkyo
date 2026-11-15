<script lang="ts">
  // Indicateur d'enregistrement (aria-live) « Modifié / Enregistrement / Enregistré » à largeur réservée : `status` d'un autosave, sinon les écritures de l'onglet.
  import { onDestroy } from 'svelte';
  import { saving } from '../lib/api';
  import type { SaveStatus } from '../lib/autosave';
  let { status, onretry }: {
    /** État fourni par l'écran (`$saver` d'un autosave) ; sans lui, suit toutes les écritures de l'onglet (api.ts). */
    status?: SaveStatus;
    /** Nouvel essai proposé quand l'enregistrement a échoué. */
    onretry?: () => void;
  } = $props();
  let global = $state<SaveStatus>('idle');
  const off = saving.subscribe((s) => { global = s.pending > 0 ? 'saving' : s.failed ? 'error' : s.at ? 'saved' : 'idle'; });
  onDestroy(off);
  const cur = $derived(status ?? global);
  const LABEL: Record<SaveStatus, string> = { idle: '', dirty: 'Modifié', saving: 'Enregistrement', saved: 'Enregistré', error: 'Non enregistré', conflict: 'Modifié ailleurs' };
  const all = Object.values(LABEL).filter(Boolean);
</script>

<!-- Tous les libellés occupent la même cellule : la largeur ne change jamais (la barre ne saute pas). -->
<span class="save-state {cur}" role={cur === 'error' || cur === 'conflict' ? 'alert' : 'status'} aria-live="polite">
  {#each all as l}<span class="ss-l" aria-hidden={l !== LABEL[cur]} class:on={l === LABEL[cur]}>{#if l === LABEL.saving}<span class="spin" aria-hidden="true"></span>{/if}{l}</span>{/each}
  {#if cur === 'error' && onretry}<button type="button" class="ss-retry" onclick={onretry}>Réessayer</button>{/if}
</span>

<style>
  .save-state { display: inline-grid; align-items: center; gap: 6px; grid-template-columns: auto auto; font-size: var(--fs-sm, 12.5px); color: var(--fg-3); white-space: nowrap; }
  .ss-l { grid-row: 1; grid-column: 1; display: inline-flex; align-items: center; gap: 6px; visibility: hidden; }
  .ss-l.on { visibility: visible; }
  .ss-l .spin { width: 12px; height: 12px; border-width: 1.5px; }
  .save-state.error, .save-state.conflict { color: var(--warn); }
  .save-state.saved .ss-l.on { color: var(--fg-3); }
  .ss-retry { grid-row: 1; grid-column: 2; border: 0; background: none; padding: 0 2px; font: 600 var(--fs-sm, 12.5px) var(--font); color: var(--accent); text-decoration: underline; text-underline-offset: 3px; cursor: pointer; border-radius: 4px; }
  .ss-retry:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
