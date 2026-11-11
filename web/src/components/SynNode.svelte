<script lang="ts">
  // Nœud du synoptique : en-tête à la couleur de la fonction, entrées à gauche, sorties
  // à droite, chaque port avec le dessin de son connecteur et la couleur de son signal.
  // Sert d'aperçu dans la fiche équipement ; le synoptique (lot 6) dessinera de même.
  import { refById } from '../lib/catalog.svelte';

  interface Port { connector_id: string; signal_id: string | null; direction: string; gender: string; quantity: number; label: string | null; variant: string | null }
  let { name, subtitle = '', fn = null, ports, maxPerSide = 12 }: { name: string; subtitle?: string; fn?: string | null; ports: Port[]; maxPerSide?: number } = $props();

  const NW = 270, HH = 52, PS = 28;
  const uid = $props.id();
  const color = (p: Port) => refById('signals', p.signal_id)?.data?.color ?? refById('connectors', p.connector_id)?.data?.color ?? 'var(--fg-3)';
  const short = (c: string) => {
    const n = refById('connectors', c)?.name ?? c;
    const s = n.replace(/\s*\/.*$/, '').replace(/ monophasé| triphasé/, '');
    return s.length > 9 ? s.split(/\s+/)[0]!.slice(0, 9) : s;
  };
  /** Famille de dessin d'un connecteur. */
  function family(c: string): string {
    if (c === 'bnc') return 'bnc';
    if (c === 'hd-bnc' || c === 'sma' || c === 'n-type') return 'hdbnc';
    if (c === 'din-1023') return 'din';
    if (c.startsWith('xlr')) return c;
    if (c === 'rj45' || c === 'ethercon') return 'rj45';
    if (c === 'hdmi' || c === 'displayport') return 'hdmi';
    if (c === 'usb') return 'usb';
    if (c === 'dsub9') return 'dsub';
    if (c.startsWith('opticalcon') || c === 'lc' || c === 'sc' || c === 'toslink') return 'fiber';
    if (c.startsWith('powercon') || c.startsWith('iec') || c.startsWith('cee') || c === 'schuko') return 'power';
    return 'round';
  }
  /** Un port par exemplaire : « SDI IN 1 », « SDI IN 2 »… */
  function expand(list: Port[]) {
    const out: { label: string; p: Port }[] = [];
    for (const p of list) {
      const q = Math.max(1, Math.min(Number(p.quantity) || 1, 1024));
      const base = p.label?.trim() || refById('signals', p.signal_id)?.name || short(p.connector_id);
      for (let i = 1; i <= q; i++) out.push({ label: q > 1 ? `${base} ${i}` : base, p });
    }
    return out;
  }
  const ins = $derived(expand(ports.filter((p) => p.direction === 'in')));
  const outs = $derived(expand(ports.filter((p) => p.direction !== 'in')));
  const shownIn = $derived(ins.slice(0, maxPerSide));
  const shownOut = $derived(outs.slice(0, maxPerSide));
  const rows = $derived(Math.max(shownIn.length + (ins.length > maxPerSide ? 1 : 0), shownOut.length + (outs.length > maxPerSide ? 1 : 0), 1));
  const H = $derived(HH + rows * PS + 12);
  const fnColor = $derived(refById('functions', fn)?.data?.color ?? 'var(--fg-3)');
  const y = (i: number) => HH + i * PS + PS / 2 + 4;
  const chipW = (s: string) => s.length * 6.3 + 12;
</script>

{#snippet glyph(f: string, x: number, cy: number)}
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
{/snippet}

{#snippet port(e: { label: string; p: Port }, i: number, out: boolean)}
  {@const x = out ? NW : 0}
  {@const s = short(e.p.connector_id)}
  {@const cw = chipW(s)}
  {@const cx0 = out ? NW - 16 - cw : 16}
  <g class="port" style="--c:{color(e.p)}">
    <title>{e.label} · {refById('connectors', e.p.connector_id)?.name ?? e.p.connector_id}{e.p.variant ? ` ${e.p.variant}` : ''} {e.p.gender === 'M' ? 'mâle' : 'femelle'}{e.p.signal_id ? ` · ${refById('signals', e.p.signal_id)?.name ?? ''}` : ''}{e.p.direction === 'bidir' ? ' · bidirectionnel' : ''}</title>
    {@render glyph(family(e.p.connector_id), x, y(i))}
    <g class="cchip"><rect x={cx0} y={y(i) - 8.5} width={cw} height="17" rx="4" /><text x={cx0 + cw / 2} y={y(i) + 3.6} text-anchor="middle">{s}</text></g>
    <text class="pn" x={out ? cx0 - 7 : cx0 + cw + 7} y={y(i) + 4.5} text-anchor={out ? 'end' : 'start'}>{e.label}{e.p.direction === 'bidir' ? ' ↔' : ''}</text>
  </g>
{/snippet}

<svg class="syn-node" viewBox="-24 -16 {NW + 48} {H + 32}" width={NW + 48} role="img" aria-label="Aperçu de {name || 'l’équipement'} dans le synoptique">
  <g class="node" style="--c:{fnColor}">
    <rect class="box" width={NW} height={H} rx="10" />
    <path class="hdr" d="M0 10a10 10 0 0 1 10-10h{NW - 20}a10 10 0 0 1 10 10v{HH - 10}H0z" />
    <clipPath id={uid}><rect width={NW} height={H} rx="10" /></clipPath>
    <rect class="band" width={NW} height="4" clip-path="url(#{uid})" />
    <line class="sep" x1="0" x2={NW} y1={HH} y2={HH} />
    <circle class="nled" cx="16" cy={HH / 2 + 1} r="4.5" />
    <text class="nt" x="30" y="25">{(name || 'Nouvel équipement').slice(0, 30)}</text>
    <text class="ns" x="30" y="42">{subtitle}</text>
    {#each shownIn as e, i}{@render port(e, i, false)}{/each}
    {#if ins.length > maxPerSide}<text class="more" x="16" y={y(shownIn.length) + 4}>+ {ins.length - maxPerSide} entrées</text>{/if}
    {#each shownOut as e, i}{@render port(e, i, true)}{/each}
    {#if outs.length > maxPerSide}<text class="more" x={NW - 16} y={y(shownOut.length) + 4} text-anchor="end">+ {outs.length - maxPerSide} sorties</text>{/if}
    {#if !ports.length}<text class="more" x={NW / 2} y={HH + 26} text-anchor="middle">Aucun port</text>{/if}
  </g>
</svg>

<style>
  .syn-node { display: block; max-width: 100%; height: auto; overflow: visible; }
  .node { filter: var(--node-shadow, drop-shadow(0 2px 6px rgba(0, 0, 0, .12))); }
  .box { fill: var(--node-bg); stroke: var(--node-edge); stroke-width: 1; }
  .hdr { fill: color-mix(in srgb, var(--c) 9%, var(--node-bg)); }
  .band { fill: var(--c); }
  .sep { stroke: var(--line); }
  .nled { fill: var(--c); filter: drop-shadow(0 0 3px var(--c)); }
  .nt { font: 650 15px var(--font); fill: var(--fg); }
  .ns { font: 500 11px var(--mono); fill: var(--fg-3); }
  .more { font: 600 11.5px var(--mono); fill: var(--fg-3); }
  .port :global(.cg-shell) { fill: var(--node-bg); stroke: var(--c); stroke-width: 2.2; }
  .port :global(.cg-ring) { fill: none; stroke: var(--c); stroke-width: 1.2; opacity: .75; }
  .port :global(.cg-pin), .port :global(.cg-key) { fill: var(--c); }
  .port:hover :global(.cg-shell) { fill: color-mix(in srgb, var(--c) 22%, var(--node-bg)); }
  .cchip rect { fill: color-mix(in srgb, var(--fg) 7%, transparent); stroke: color-mix(in srgb, var(--fg) 14%, transparent); stroke-width: .8; }
  .cchip text { font: 600 9.5px var(--mono); letter-spacing: .03em; fill: var(--fg-3); }
  .pn { font: 500 13px var(--font); fill: var(--fg-2); }
</style>
