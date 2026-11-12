<script lang="ts">
  // Nœud du synoptique, dessiné comme dans le synoptique : en-tête à la couleur de la
  // fonction, entrées à gauche, sorties et E/S à droite, un point par port à la couleur
  // du signal, le connecteur et son genre (M/F). Aperçu de la fiche équipement.
  import { refById } from '../lib/catalog.svelte';

  interface Port { connector_id: string; signal_id: string | null; direction: string; gender: string; quantity: number; label: string | null; variant: string | null }
  let { name, subtitle = '', fn = null, ports, maxPerSide = 48, hl = null }: { name: string; subtitle?: string; fn?: string | null; ports: Port[]; maxPerSide?: number; hl?: number | null } = $props();

  const NW = 270, HH = 52, PS = 28;
  const uid = $props.id();
  const color = (p: Port) => refById('signals', p.signal_id)?.data?.color ?? refById('connectors', p.connector_id)?.data?.color ?? 'var(--fg-3)';
  const short = (c: string) => {
    const n = refById('connectors', c)?.name ?? c;
    const s = n.replace(/\s*\/.*$/, '').replace(/ monophasé| triphasé/, '');
    return s.length > 9 ? s.split(/\s+/)[0]!.slice(0, 9) : s;
  };
  /** Un port par exemplaire : « SDI IN 1 », « SDI IN 2 »… */
  function expand(list: Port[]) {
    const out: { label: string; p: Port; g: number }[] = [];
    for (const p of list) {
      const g = ports.indexOf(p);
      const q = Math.max(1, Math.min(Number(p.quantity) || 1, 1024));
      const base = p.label?.trim() || refById('signals', p.signal_id)?.name || short(p.connector_id);
      for (let i = 1; i <= q; i++) out.push({ label: q > 1 ? `${base} ${i}` : base, p, g });
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

{#snippet port(e: { label: string; p: Port; g: number }, i: number, out: boolean)}
  {@const x = out ? NW : 0}
  {@const s = short(e.p.connector_id) + (e.p.gender === 'M' ? ' M' : e.p.gender === 'F' ? ' F' : '')}
  {@const room = (out ? shownIn : shownOut).length > i ? 9 : 20}
  {@const cw = chipW(s)}
  {@const cx0 = out ? NW - 16 - cw : 16}
  <g class="port" class:hl={hl === e.g} class:dim={hl != null && hl !== e.g} style="--c:{color(e.p)}">
    <title>{e.label} · {refById('connectors', e.p.connector_id)?.name ?? e.p.connector_id}{e.p.variant ? ` ${e.p.variant}` : ''} {e.p.gender === 'M' ? 'mâle' : 'femelle'}{e.p.signal_id ? ` · ${refById('signals', e.p.signal_id)?.name ?? ''}` : ''}{e.p.direction === 'bidir' ? ' · bidirectionnel' : ''}</title>
    {#if hl === e.g}<rect class="hlbar" x={out ? NW / 2 + 2 : 2} y={y(i) - 13} width={NW / 2 - 4} height="26" rx="6" />{/if}
    <circle class="pdot" cx={x} cy={y(i)} r="4.5" />
    <g class="cchip"><rect x={cx0} y={y(i) - 8.5} width={cw} height="17" rx="4" /><text x={cx0 + cw / 2} y={y(i) + 3.6} text-anchor="middle">{s}</text></g>
    <text class="pn" x={out ? cx0 - 7 : cx0 + cw + 7} y={y(i) + 4.5} text-anchor={out ? 'end' : 'start'}>{e.label.length > room ? e.label.slice(0, room - 1) + '…' : e.label}{e.p.direction === 'bidir' ? ' ↔' : ''}</text>
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
  .pdot { fill: var(--node-bg, var(--surface)); stroke: var(--c); stroke-width: 2; }
  .port:hover .pdot, .port.hl .pdot { fill: var(--c); }
  .port { transition: opacity .15s; }
  .port.dim { opacity: .35; }
  .hlbar { fill: color-mix(in srgb, var(--c) 12%, transparent); }
  .cchip rect { fill: color-mix(in srgb, var(--fg) 7%, transparent); stroke: color-mix(in srgb, var(--fg) 14%, transparent); stroke-width: .8; }
  .cchip text { font: 600 9.5px var(--mono); letter-spacing: .03em; fill: var(--fg-3); }
  .pn { font: 500 13px var(--font); fill: var(--fg-2); }
</style>
