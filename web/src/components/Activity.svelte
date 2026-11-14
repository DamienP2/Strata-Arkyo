<script lang="ts">
  // Barre d'activité : visible quand un enregistrement dure plus de 300 ms.
  import { onDestroy } from 'svelte';
  import { activity } from '../lib/api';
  let on = $state(false);
  let t: ReturnType<typeof setTimeout> | null = null;
  const f = (n: number) => { if (n > 0 && !t && !on) t = setTimeout(() => { t = null; on = activity.count > 0; }, 300); if (n === 0) { if (t) { clearTimeout(t); t = null; } on = false; } };
  activity.listeners.add(f);
  onDestroy(() => activity.listeners.delete(f));
</script>

{#if on}<div class="act" role="progressbar" aria-label="Enregistrement en cours"></div>{/if}

<style>
  .act { position: fixed; left: 0; right: 0; top: 0; height: 3px; z-index: 9999; background: linear-gradient(90deg, transparent, var(--accent, #4F6BFF), transparent); background-size: 40% 100%; background-repeat: no-repeat; animation: act 1s linear infinite; pointer-events: none; }
  @keyframes act { from { background-position: -40% 0; } to { background-position: 140% 0; } }
</style>
