<script lang="ts">
  import Confirm from './Confirm.svelte';
  import { asking, answer } from '../lib/ask.svelte';
</script>

{#if asking.current}
  {@const a = asking.current}
  <Confirm title={a.title} eyebrow={a.eyebrow ?? ''} confirmLabel={a.confirmLabel} danger={a.danger ?? false} onconfirm={() => answer(true)} onclose={() => answer(false)}>
    {#if a.text}<p class="lead">{a.text}</p>{/if}
  </Confirm>
{/if}
