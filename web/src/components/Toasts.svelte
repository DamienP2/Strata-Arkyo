<script lang="ts">
  import { toasts, dismiss } from '../lib/toast.svelte';
  import Icon from './Icon.svelte';
  const icon = { ok: 'ok', info: 'info', warn: 'alert', err: 'alert' } as const;
</script>

<div class="toasts" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="toast {t.kind}" role={t.kind === 'err' ? 'alert' : 'status'}>
      <Icon name={icon[t.kind]} /><span>{t.text}</span>
      {#if t.undo}<button class="t-undo" onclick={() => { t.undo!.run(); dismiss(t.id); }}>{t.undo.label}</button>{/if}
      <button class="t-x" aria-label="Fermer la notification" title="Fermer" onclick={() => dismiss(t.id)}><Icon name="x" size="sm" /></button>
    </div>
  {/each}
</div>

<style>
  .toast { pointer-events: auto; }
  .t-undo { flex: none; border: 0; background: none; color: inherit; font: 700 13px var(--font); text-decoration: underline; cursor: pointer; padding: 0 2px; }
  .t-x { flex: none; display: grid; place-items: center; width: 24px; height: 24px; margin-left: 2px; border: 0; border-radius: 6px; background: none; color: inherit; opacity: .7; cursor: pointer; }
  .t-x:hover { opacity: 1; background: color-mix(in srgb, currentColor 12%, transparent); }
</style>
