<script lang="ts">
  // Boîte de dialogue commune : en-tête, corps, pied standard (Annuler + bouton principal dans un <form>) ou libre.
  import { tick, type Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import { modal, ignoredInput } from '../lib/modal';
  import { fail } from '../lib/toast.svelte';
  let { title, eyebrow = '', onclose, children, footer, wide = false, xl = false, locked = false,
    onsubmit, submitLabel, cancelLabel = 'Annuler', busy = false, danger = false, submitDisabled = false, dirty }:
    { title: string; eyebrow?: string; onclose: () => void; children: Snippet;
      /** Pied libre ; avec `onsubmit`, contenu ajouté à gauche du pied standard (action destructive…). */
      footer?: Snippet; wide?: boolean; xl?: boolean;
      /** Échap et le clic à côté ne ferment pas (seuls les boutons de la boîte ferment). */
      locked?: boolean;
      /** Envoi du formulaire (Entrée ou bouton principal). Le corps est alors enveloppé dans un <form>. */
      onsubmit?: (e: SubmitEvent) => unknown;
      /** Libellé du bouton principal du pied standard (« Enregistrer », « Créer »…). */
      submitLabel?: string;
      /** Libellé du bouton de renoncement du pied standard. */
      cancelLabel?: string;
      /** Envoi en cours (en plus de celui que la boîte suit elle-même pendant `onsubmit`). */
      busy?: boolean;
      /** Bouton principal en danger plein (validation d'une destruction). */
      danger?: boolean;
      submitDisabled?: boolean;
      /** Saisie commencée, calculée par l'écran ; remplace le suivi automatique des champs. */
      dirty?: () => boolean } = $props();
  // Saisie commencée : Échap, le fond ou la croix demandent avant de tout abandonner.
  let asking = $state(false), touched = $state(false), sending = $state(false);
  let askEl = $state<HTMLButtonElement>();
  let lastField: HTMLElement | null = null;
  const isBusy = $derived(busy || sending);
  const isDirty = () => (dirty ? dirty() : touched);
  $effect(() => { if (asking) askEl?.focus(); });
  function track(e: Event) { if (!ignoredInput(e.target)) touched = true; }
  function focusIn(e: FocusEvent) { const t = e.target as HTMLElement; if (t.matches?.('input,select,textarea')) lastField = t; }
  function requestClose() { if (isDirty()) asking = true; else onclose(); }
  // Second Échap (ou « Continuer la saisie ») : retour au champ en cours.
  async function resume() { asking = false; await tick(); if (lastField?.isConnected) lastField.focus(); }
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    // Un bouton du corps sans type (segment, action secondaire) n'envoie pas le formulaire.
    if (e.submitter && !e.submitter.hasAttribute('data-dlg-submit')) return;
    if (isBusy || submitDisabled || !onsubmit) return;
    sending = true;
    try { await onsubmit(e); touched = false; } catch (x) { fail(x); } finally { sending = false; }
  }
</script>

{#snippet inner()}
  <div class="wz-head">
    <div class="row">
      <span class="eyebrow">{eyebrow}</span>
      {#if !locked}<button type="button" class="icon-btn sm" disabled={isBusy} onclick={requestClose} aria-label="Fermer (Échap)" title="Fermer (Échap)"><Icon name="x" /></button>{/if}
    </div>
  </div>
  <div class="wz-body dlg-body">
    <h2>{title}</h2>
    {@render children()}
  </div>
  {#if asking}
    <div class="wz-foot dlg-ask" role="alertdialog" aria-label="Fermer sans enregistrer ?">
      <span>Fermer sans enregistrer ? Ce qui n’a pas été enregistré sera perdu.</span>
      <button type="button" class="btn ghost" bind:this={askEl} onclick={resume}>Continuer la saisie</button>
      <button type="button" class="btn primary danger" onclick={onclose}>Fermer sans enregistrer</button>
    </div>
  {:else if onsubmit && submitLabel}
    <div class="wz-foot dlg-foot">
      {#if footer}{@render footer()}{/if}
      {#if !locked}<button type="button" class="btn ghost" disabled={isBusy} onclick={onclose}>{cancelLabel}</button>{/if}
      <button type="submit" data-dlg-submit class="btn primary" class:danger disabled={isBusy || submitDisabled}>{#if isBusy}<span class="spin" aria-hidden="true"></span>{/if}{submitLabel}</button>
    </div>
  {:else if footer}<div class="wz-foot">{@render footer()}</div>{/if}
{/snippet}

<div class="modal" role="presentation" use:modal={{ close: onclose, locked: () => locked || isBusy, dirty: isDirty, ask: () => (asking ? resume() : (asking = true)) }}>
  <div class="dialog" class:wide class:xl role="dialog" aria-modal="true" aria-label={title} tabindex="-1" oninput={track} onchange={track} onfocusin={focusIn}>
    {#if onsubmit}
      <form class="dlg-form" onsubmit={submit}>
        <!-- Bouton par défaut du formulaire (Entrée dans un champ), placé en tête pour qu'aucun bouton du corps ne le devienne. -->
        <button type="submit" class="dlg-def" data-dlg-submit tabindex="-1" aria-hidden="true" disabled={isBusy || submitDisabled}></button>
        {@render inner()}
      </form>
    {:else}{@render inner()}{/if}
  </div>
</div>

<style>
  .dialog.wide { width: min(900px, 100%); }
  .dialog.xl { width: min(1240px, 100%); }
  .dlg-body { min-height: 0; }
  .dlg-body h2 { margin: 0; font: 700 var(--fs-xl, 20px)/1.2 var(--display); letter-spacing: -.015em; }
  .dlg-foot { justify-content: flex-end; align-items: center; }
  .dlg-def { position: absolute; width: 1px; height: 1px; padding: 0; border: 0; overflow: hidden; clip: rect(0 0 0 0); opacity: 0; pointer-events: none; }
  .dlg-ask { gap: 8px; align-items: center; }
  .dlg-ask span { flex: 1; font-size: var(--fs-md, 13.5px); color: var(--fg-2); }
</style>
