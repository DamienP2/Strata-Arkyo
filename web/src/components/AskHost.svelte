<script lang="ts">
  // Hôte unique des confirmations ask() (posé une fois dans App.svelte).
  import Confirm from './Confirm.svelte';
  import { asking, answer, confirmAsk } from '../lib/ask.svelte';
</script>

{#if asking.current}
  {@const a = asking.current}
  <Confirm title={a.title} eyebrow={a.eyebrow ?? ''} confirmLabel={a.confirmLabel} cancelLabel={a.cancelLabel ?? 'Annuler'} danger={a.danger ?? false} onconfirm={confirmAsk} onclose={() => answer(false)}>
    {#if a.text}<p class="lead">{a.text}</p>{/if}
  </Confirm>
{/if}
