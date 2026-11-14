<script lang="ts">
  // État vide unique : une icône, une phrase, au plus une action.
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  let { icon = 'box', message, hint = '', action, card = true }: {
    icon?: string; message: string; hint?: string; action?: Snippet;
    /** Faux : sans cadre (dans une carte ou une cellule existante). */
    card?: boolean;
  } = $props();
</script>

<div class="empty-state" class:card role="status">
  <span class="es-ic"><Icon name={icon} /></span>
  <p class="es-msg">{message}</p>
  {#if hint}<p class="es-hint">{hint}</p>{/if}
  {#if action}<div class="es-act">{@render action()}</div>{/if}
</div>
