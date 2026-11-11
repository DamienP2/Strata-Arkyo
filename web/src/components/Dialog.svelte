<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  let { title, eyebrow = '', onclose, children, footer, wide = false }:
    { title: string; eyebrow?: string; onclose: () => void; children: Snippet; footer?: Snippet; wide?: boolean } = $props();

  function key(e: KeyboardEvent) { if (e.key === 'Escape') onclose(); }
  let dlg: HTMLDivElement;
  $effect(() => { dlg?.querySelector<HTMLElement>('input,select,textarea,button.primary')?.focus(); });
</script>

<svelte:window onkeydown={key} />
<div class="modal" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}>
  <div class="dialog" class:wide bind:this={dlg} role="dialog" aria-modal="true" aria-label={title}>
    <div class="wz-head">
      <div class="row">
        <span class="eyebrow">{eyebrow}</span>
        <button class="icon-btn sm" onclick={onclose} aria-label="Fermer"><Icon name="x" /></button>
      </div>
    </div>
    <div class="wz-body dlg-body">
      <h2>{title}</h2>
      {@render children()}
    </div>
    {#if footer}<div class="wz-foot">{@render footer()}</div>{/if}
  </div>
</div>

<style>
  .dialog.wide { width: min(900px, 100%); }
  .dlg-body { min-height: 0; }
  .dlg-body h2 { font-size: 30px; }
</style>
