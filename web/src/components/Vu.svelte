<script lang="ts">
  // Vumètre à segments : progression (prog) ou charge avec zones vert, ambre, rouge (load).
  // Au-delà de 100 % (dépassement), la jauge entière passe en couleur d'erreur : un dépassement
  // ne se confond jamais avec une jauge pleine.
  let { value, mode = 'prog', n = 20, cls = '' }: { value: number; mode?: 'prog' | 'load'; n?: number; cls?: string } = $props();
  const over = $derived(value > 100);
  const on = $derived(over ? n : Math.round((Math.min(Math.max(value, 0), 100) / 100) * n));
  const zone = (i: number) => (mode === 'load' && !over ? (i >= n * 0.88 ? 'r' : i >= n * 0.7 ? 'y' : 'g') : '');
</script>

<span class="vu {mode} {cls}" class:over role="img" aria-label="{Math.round(value)} %{over ? ' : dépassement' : ''}">{#each Array(n) as _, i}<i class="{i < on ? 'on' : ''} {zone(i)}"></i>{/each}</span>

<style>
  .vu.over i.on { --vu-c: var(--err); background: var(--err); }
</style>
