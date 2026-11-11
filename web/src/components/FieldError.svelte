<script lang="ts">
  let { message }: { message?: string | null } = $props();
</script>

{#if message}<span class="help err" role="alert">{message}</span>{/if}
