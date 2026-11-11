<svelte:options namespace="svg" />
<script lang="ts">
  // Dessin d'un connecteur (BNC, XLR, RJ45…), centré en (x, cy). Les couleurs viennent des
  // classes cg-shell, cg-ring, cg-pin et cg-key, stylées par le parent.
  let { f, x = 0, cy = 0 }: { f: string; x?: number; cy?: number } = $props();
</script>
{#if f === 'xlr3' || f === 'xlr4' || f === 'xlr5'}
    {@const n = Number(f.slice(3))}
    <circle class="cg-shell" cx={x} cy={cy} r="7.5" /><rect class="cg-key" x={x - 1.2} y={cy - 8.6} width="2.4" height="2.6" rx=".6" />
    {#each Array(n) as _, k}{@const a = -Math.PI / 2 + Math.PI / 4 + (k * 2 * Math.PI) / n}<circle class="cg-pin" cx={x + Math.cos(a) * 3.4} cy={cy + Math.sin(a) * 3.4 + 0.6} r="1.15" />{/each}
  {:else if f === 'hdmi'}
    <path class="cg-shell" d="M{x - 8} {cy - 4.5}h16v4.2l-2.6 4.3h-10.8l-2.6-4.3z" /><rect class="cg-pin" x={x - 5} y={cy - 1.5} width="10" height="1.6" rx=".5" />
  {:else if f === 'rj45'}
    <rect class="cg-shell" x={x - 6.5} y={cy - 6} width="13" height="11" rx="1.5" /><rect class="cg-key" x={x - 2.5} y={cy + 3} width="5" height="3.4" />
  {:else if f === 'usb'}
    <rect class="cg-shell" x={x - 7} y={cy - 4} width="14" height="8" rx="4" /><rect class="cg-pin" x={x - 4} y={cy - 1} width="8" height="2" rx="1" />
  {:else if f === 'dsub'}
    <path class="cg-shell" d="M{x - 8.5} {cy - 5}h17l-2.6 10h-11.8z" />{#each [-5, -2.5, 0, 2.5, 5] as dx}<circle class="cg-pin" cx={x + dx} cy={cy - 1.5} r=".9" />{/each}
  {:else if f === 'fiber'}
    <rect class="cg-shell" x={x - 8} y={cy - 5} width="7" height="10" rx="1.5" /><rect class="cg-shell" x={x + 1} y={cy - 5} width="7" height="10" rx="1.5" /><circle class="cg-pin" cx={x - 4.5} cy={cy} r="1.3" /><circle class="cg-pin" cx={x + 4.5} cy={cy} r="1.3" />
  {:else if f === 'power'}
    <circle class="cg-shell" cx={x} cy={cy} r="7.5" /><rect class="cg-pin" x={x - 4} y={cy - 3} width="1.6" height="4" /><rect class="cg-pin" x={x + 2.4} y={cy - 3} width="1.6" height="4" /><rect class="cg-pin" x={x - 0.8} y={cy + 2} width="1.6" height="3" />
  {:else if f === 'hdbnc'}
    <circle class="cg-shell" cx={x} cy={cy} r="5" /><circle class="cg-pin" cx={x} cy={cy} r="1.4" /><rect class="cg-key" x={x - 1} y={cy - 7} width="2" height="2.6" rx=".5" />
  {:else if f === 'din'}
    <rect class="cg-shell" x={x - 5.5} y={cy - 5.5} width="11" height="11" rx="2.5" /><circle class="cg-pin" cx={x} cy={cy} r="1.4" />
  {:else if f === 'bnc'}
    <circle class="cg-shell" cx={x} cy={cy} r="7" /><circle class="cg-ring" cx={x} cy={cy} r="3.6" /><circle class="cg-pin" cx={x} cy={cy} r="1.4" />
    <rect class="cg-key" x={x - 1.1} y={cy - 9.4} width="2.2" height="3" rx=".6" /><rect class="cg-key" x={x - 1.1} y={cy + 6.4} width="2.2" height="3" rx=".6" />
  {:else}
    <circle class="cg-shell" cx={x} cy={cy} r="6" /><circle class="cg-pin" cx={x} cy={cy} r="1.6" />
  {/if}
