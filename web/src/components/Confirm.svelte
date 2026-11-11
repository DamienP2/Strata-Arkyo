<script lang="ts">
  import type { Snippet } from 'svelte';
  import Dialog from './Dialog.svelte';
  let { title, eyebrow = '', confirmLabel, danger = false, onconfirm, onclose, children }:
    { title: string; eyebrow?: string; confirmLabel: string; danger?: boolean; onconfirm: () => Promise<void> | void; onclose: () => void; children: Snippet } = $props();
  let busy = $state(false);
  async function go() { busy = true; try { await onconfirm(); } finally { busy = false; } }
</script>

<Dialog {title} {eyebrow} {onclose}>
  {@render children()}
  {#snippet footer()}
    <button class="btn ghost" onclick={onclose}>Annuler</button>
    <button class="btn primary" class:danger-btn={danger} disabled={busy} onclick={go}>{#if busy}<span class="spin"></span>{/if}{confirmLabel}</button>
  {/snippet}
</Dialog>

<style>
  .danger-btn { background: var(--err); color: #fff; }
</style>
