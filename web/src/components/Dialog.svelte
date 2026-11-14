<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import { modal } from '../lib/modal';
  let { title, eyebrow = '', onclose, children, footer, wide = false, xl = false, locked = false }:
    { title: string; eyebrow?: string; onclose: () => void; children: Snippet; footer?: Snippet; wide?: boolean; xl?: boolean;
      /** Échap et le clic à côté ne ferment pas (seuls les boutons de la boîte ferment). */
      locked?: boolean } = $props();
  // Saisie commencée : Échap, le fond ou la croix demandent avant de tout abandonner.
  let asking = $state(false), touched = false;
  let askEl = $state<HTMLButtonElement>();
  $effect(() => { if (asking) askEl?.focus(); });
</script>

<div class="modal" role="presentation" use:modal={{ close: onclose, locked: () => locked, ask: () => (asking = true) }}>
  <div class="dialog" class:wide class:xl role="dialog" aria-modal="true" aria-label={title} oninput={() => (touched = true)} onchange={() => (touched = true)}>
    <div class="wz-head">
      <div class="row">
        <span class="eyebrow">{eyebrow}</span>
        {#if !locked}<button class="icon-btn sm" onclick={() => (touched ? (asking = true) : onclose())} aria-label="Fermer"><Icon name="x" /></button>{/if}
      </div>
    </div>
    <div class="wz-body dlg-body">
      <h2>{title}</h2>
      {@render children()}
    </div>
    {#if asking}
      <div class="wz-foot dlg-ask" role="alertdialog" aria-label="Abandonner la saisie ?">
        <span>Fermer cette fenêtre ? Ce qui n’a pas été enregistré sera perdu.</span>
        <button class="btn ghost" bind:this={askEl} onclick={() => (asking = false)}>Continuer la saisie</button>
        <button class="btn danger-btn" onclick={onclose}>Fermer sans enregistrer</button>
      </div>
    {:else if footer}<div class="wz-foot">{@render footer()}</div>{/if}
  </div>
</div>

<style>
  .dialog.wide { width: min(900px, 100%); }
  .dialog.xl { width: min(1240px, 100%); }
  .dlg-body { min-height: 0; }
  .dlg-body h2 { font-size: 30px; }
  .dlg-ask { gap: 10px; align-items: center; }
  .dlg-ask span { flex: 1; font-size: 14px; color: var(--fg-2); }
  .dlg-ask .danger-btn { background: var(--err); color: #fff; }
</style>
