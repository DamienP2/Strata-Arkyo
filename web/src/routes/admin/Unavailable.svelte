<script lang="ts">
  let { message }: { message: string } = $props();
</script>
<div class="alert warn" role="alert"><span>{message}</span></div>
