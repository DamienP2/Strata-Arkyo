<script lang="ts">
  import { toasts, dismiss, runUndo } from '../lib/toast.svelte';
  import Icon from './Icon.svelte';
  const icon = { ok: 'ok', info: 'info', warn: 'alert', err: 'alert' } as const;
</script>

<div class="toasts" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="toast {t.kind}" role={t.kind === 'err' ? 'alert' : 'status'}>
      <Icon name={icon[t.kind]} /><span>{t.text}</span>
      {#if t.undo}<button type="button" class="t-undo" onclick={() => runUndo(t)}>Annuler</button>{/if}
      <button type="button" class="icon-btn xs t-x" aria-label="Fermer la notification" title="Fermer la notification" onclick={() => dismiss(t.id)}><Icon name="x" size="sm" /></button>
    </div>
  {/each}
</div>

<style>
  .toast { pointer-events: auto; }
  .t-undo { flex: none; border: 0; background: none; color: inherit; font: 700 var(--fs-ctl-sm, 13px) var(--font); text-decoration: underline; text-underline-offset: 3px; cursor: pointer; padding: 0 2px; border-radius: 4px; }
  .t-undo:focus-visible { outline: 2px solid currentColor; outline-offset: 2px; }
  /* Le toast est une plaque d'encre : la fermeture (.icon-btn.xs) y reprend la couleur du texte (.toast .icon-btn, harmonie.css). */
  .t-x { margin-left: 2px; }
  .t-x:hover:not(:disabled) { color: inherit; background: color-mix(in srgb, currentColor 14%, transparent); box-shadow: none; }
  .t-x:focus-visible { outline: 2px solid currentColor; outline-offset: 2px; }
</style>
