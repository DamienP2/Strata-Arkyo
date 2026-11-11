<script lang="ts">
  // Illustration de régie (car, flight-case, installation fixe), avec faisceau de câbles en option.
  let { kind, cables = false, pulse = false, w = 320, label = '' }: { kind: 'truck' | 'case' | 'fixe'; cables?: boolean; pulse?: boolean; w?: number; label?: string } = $props();
  const ANCHOR = { truck: [289, 99], case: [236, 114], fixe: [294, 112] } as const;
  const paths = $derived(['v', 'a', 's', 'n'].map((s, i) => {
    const [ax, ay] = ANCHOR[kind];
    const y = ay + i * 3.2, gy = 133 + i * 3.7;
    return { s, i, d: `M${ax} ${y}C${ax + 20 + i * 3} ${y} ${ax + 12 + i * 3} ${gy} ${ax + 36 + i * 3} ${gy}H${w + 60}` };
  }));
</script>

<svg class="ill" viewBox="0 0 {w} 160" role={label ? 'img' : undefined} aria-label={label || undefined} aria-hidden={label ? undefined : 'true'}>
  <use href="#ill-{kind}" width="320" height="160" />
  {#if cables}
    <g class="harness">
      {#each paths as p}
        <path d={p.d} style="stroke:var(--sig-{p.s})" />
        {#if pulse}<path class="pulse" d={p.d} style="animation-delay:-{(p.i * 0.9 + 0.3).toFixed(1)}s;animation-duration:{(3 + p.i * 0.45).toFixed(2)}s" />{/if}
      {/each}
    </g>
  {/if}
</svg>
