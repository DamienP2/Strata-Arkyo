<script lang="ts">
  import { toasts } from '../lib/toast.svelte';
  import Icon from './Icon.svelte';
  const icon = { ok: 'ok', info: 'info', warn: 'alert', err: 'alert' } as const;
</script>

<div class="toasts" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="toast {t.kind}"><Icon name={icon[t.kind]} /><span>{t.text}</span></div>
  {/each}
</div>
