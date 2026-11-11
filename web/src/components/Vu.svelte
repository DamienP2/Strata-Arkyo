<script lang="ts">
  // Vumètre à segments : progression (prog) ou charge avec zones vert, ambre, rouge (load).
  let { value, mode = 'prog', n = 20, cls = '' }: { value: number; mode?: 'prog' | 'load'; n?: number; cls?: string } = $props();
  const on = $derived(Math.round((Math.min(Math.max(value, 0), 100) / 100) * n));
  const zone = (i: number) => (mode === 'load' ? (i >= n * 0.88 ? 'r' : i >= n * 0.7 ? 'y' : 'g') : '');
</script>

<span class="vu {mode} {cls}" role="img" aria-label="{Math.round(value)} %">{#each Array(n) as _, i}<i class="{i < on ? 'on' : ''} {zone(i)}"></i>{/each}</span>
