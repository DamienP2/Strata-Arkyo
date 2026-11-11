<script lang="ts">
  // Face avant d'un panel de brassage. En détail (éditeur) : oreilles, groupes de ports
  // colorés, connecteurs et libellés par port. En compact (élévation de la baie) : les
  // connecteurs seuls, à l'échelle de l'élément.
  import ConnGlyph from './ConnGlyph.svelte';
  import { family, slotColor, groupAt, colOf, rowOf, type PanelData, type PanelGroup, type PanelSlot } from '../lib/panel';
  import { refById } from '../lib/catalog.svelte';

  let {
    panel, detail = false, selected = [], target = null, onslot, ongroup,
  }: {
    panel: PanelData; detail?: boolean; selected?: number[]; target?: number | null;
    onslot?: (index: number, e: MouseEvent) => void; ongroup?: (g: PanelGroup, e: MouseEvent) => void;
  } = $props();

  let cw = $state(400), ch = $state(20);
  const EAR = 30;
  const W = $derived(detail ? 964 : Math.max(40, cw));
  const ROW = $derived(detail ? 89 : Math.max(8, ch / panel.height_u));
  const H = $derived(ROW * panel.height_u);
  const x0 = $derived(detail ? EAR : 0);
  const pitch = $derived((W - 2 * x0) / panel.per_u);
  const cy = $derived(detail ? 46 : ROW / 2);
  const scale = $derived(Math.max(0.2, Math.min(pitch * 0.78, detail ? 36 : ROW * 0.82) / 19));
  const sx = (index: number) => x0 + (colOf(panel, index) - 1) * pitch;
  const sy = (index: number) => (rowOf(panel, index) - 1) * ROW;
  const sel = $derived(new Set(selected));
  const name = (s: PanelSlot) => `${refById('connectors', s.connector)?.name ?? s.connector}${s.variant ? ` ${s.variant}` : ''}${s.d_format ? ' · format D' : ''}`;
  const empties = $derived.by(() => {
    const out: number[] = [];
    const used = new Set<number>();
    for (const s of panel.slots) for (let i = 0; i < Math.max(1, s.width); i++) used.add(s.index + i);
    for (let i = 1; i <= panel.height_u * panel.per_u; i++) if (!used.has(i)) out.push(i);
    return out;
  });
  const fs = $derived(Math.min(10.5, pitch * 0.28 + 3));
</script>

{#snippet cell(index: number, width: number, s: PanelSlot | null)}
  {@const x = sx(index)}
  {@const y = sy(index)}
  {@const w = pitch * width}
  {@const g = detail ? groupAt(panel, rowOf(panel, index), colOf(panel, index)) : undefined}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <g class="cell" class:on={sel.has(index)} class:tgt={target === index} class:full={!!s} style="--c:{s ? slotColor(s) : 'var(--fp-mute,#8F98A3)'}"
    role={onslot ? 'button' : undefined} tabindex={onslot ? 0 : undefined}
    onclick={(e) => onslot?.(index, e)} onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onslot?.(index, e as unknown as MouseEvent); } }}>
    <title>{s ? `${index}${width > 1 ? `–${index + width - 1}` : ''} · ${s.label || 'sans libellé'} · ${name(s)}${g ? ` · ${g.label}` : ''}` : `Emplacement ${index} libre`}</title>
    <rect class="hit" x={x + 1} y={y + (detail ? 20 : 1)} width={w - 2} height={detail ? ROW - 22 : ROW - 2} rx={detail ? 5 : 2} />
    {#if s}
      {#if s.d_format && detail}
        {@const dw = Math.min(w - 6, 50)}
        <g class="dplate"><rect x={x + w / 2 - dw / 2} y={y + cy - 25} width={dw} height="48" rx="5" />
          <circle cx={x + w / 2 - dw / 2 + 6} cy={y + cy - 19} r="2.2" /><circle cx={x + w / 2 + dw / 2 - 6} cy={y + cy + 17} r="2.2" /></g>
      {/if}
      <g class="port" transform="translate({x + w / 2} {y + cy}) scale({scale})"><ConnGlyph f={family(s.connector)} /></g>
      {#if detail}
        <text class="pl" class:muted={!s.label} x={x + w / 2} y={y + 80} text-anchor="middle" font-size={fs}>{(s.label || String(index)).slice(0, Math.max(3, Math.floor((w / fs) * 1.7)))}</text>
      {/if}
    {:else if !detail}
      <circle class="hole" cx={x + w / 2} cy={y + cy} r={Math.max(1, Math.min(pitch, ROW) * 0.18)} />
    {:else}
      <circle class="hole" cx={x + w / 2} cy={y + cy} r={Math.min(9, pitch * 0.25)} />
      <text class="pl muted" x={x + w / 2} y={y + 80} text-anchor="middle" font-size={fs}>{index}</text>
    {/if}
  </g>
{/snippet}

<div class="pface" class:detail bind:clientWidth={cw} bind:clientHeight={ch}>
  <svg viewBox="0 0 {W} {H}" preserveAspectRatio={detail ? 'xMidYMid meet' : 'none'} role="img" aria-label="Face du panel {panel.name}">
    {#if detail}
      <rect class="plate" x="0" y="0" width={W} height={H} rx="6" />
      {#each Array(panel.height_u) as _, r}
        {#each [EAR / 2, W - EAR / 2] as ex}<rect class="slotted" x={ex - 4} y={r * ROW + ROW / 2 - 9} width="8" height="18" rx="4" />{/each}
        {#if r > 0}<line class="rowsep" x1={EAR} x2={W - EAR} y1={r * ROW} y2={r * ROW} />{/if}
      {/each}
      {#each panel.groups as g}
        {@const gx = x0 + (g.start - 1) * pitch}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <g class="grp" style="--g:{g.color}" role={ongroup ? 'button' : undefined} tabindex={ongroup ? 0 : undefined}
          onclick={(e) => ongroup?.(g, e)} onkeydown={(e) => { if (e.key === 'Enter') ongroup?.(g, e as unknown as MouseEvent); }}>
          <title>Groupe {g.label || 'sans nom'} · ports {g.start} à {g.start + g.len - 1}</title>
          <rect class="gbox" x={gx + 2} y={(g.row - 1) * ROW + 3} width={g.len * pitch - 4} height={ROW - 6} rx="6" />
          <rect class="gband" x={gx + 2} y={(g.row - 1) * ROW + 3} width={g.len * pitch - 4} height="15" rx="4" />
          <text class="gl" x={gx + (g.len * pitch) / 2} y={(g.row - 1) * ROW + 14} text-anchor="middle">{g.label.slice(0, Math.max(2, Math.floor((g.len * pitch) / 6.4)))}</text>
        </g>
      {/each}
    {:else}
      {#each panel.groups as g}
        <rect class="gline" style="--g:{g.color}" x={(g.start - 1) * pitch + 1} y={(g.row - 1) * ROW} width={g.len * pitch - 2} height={Math.max(1.5, ROW * 0.08)} />
      {/each}
    {/if}
    {#each empties as i}{@render cell(i, 1, null)}{/each}
    {#each panel.slots as s (s.index)}{@render cell(s.index, Math.max(1, s.width), s)}{/each}
  </svg>
</div>

<style>
  .pface { width: 100%; height: 100%; min-width: 0; }
  .pface.detail { height: auto; }
  svg { display: block; width: 100%; height: 100%; overflow: visible; }
  .detail svg { height: auto; }
  .plate { fill: #23272D; stroke: #3A4048; filter: drop-shadow(0 6px 16px rgba(0, 0, 0, .22)); }
  .slotted { fill: #0F1114; stroke: #4A515A; }
  .rowsep { stroke: #3A4048; stroke-dasharray: 2 4; }
  .hit { fill: transparent; }
  .detail .cell { cursor: pointer; }
  .detail .cell:hover .hit { fill: rgba(255, 255, 255, .06); }
  .cell.on .hit { fill: color-mix(in srgb, var(--accent, #4F6BFF) 30%, transparent); stroke: var(--accent, #4F6BFF); stroke-width: 2; }
  .cell.tgt .hit { stroke: #F2C14E; stroke-width: 2; stroke-dasharray: 4 3; }
  .cell:focus-visible { outline: none; }
  .cell:focus-visible .hit { stroke: #fff; stroke-width: 1.5; }
  .hole { fill: #121418; stroke: #3F464E; stroke-width: 1; }
  .dplate rect { fill: #15171A; stroke: #4C535B; stroke-width: 1; }
  .dplate circle { fill: #2B3036; stroke: #5A6068; stroke-width: .8; }
  .port :global(.cg-shell) { fill: #101215; stroke: var(--c); stroke-width: 2.2; }
  .port :global(.cg-ring) { fill: none; stroke: var(--c); stroke-width: 1.2; opacity: .75; }
  .port :global(.cg-pin), .port :global(.cg-key) { fill: var(--c); }
  .pl { font-family: var(--mono); font-weight: 600; fill: #E8EBEE; letter-spacing: .02em; pointer-events: none; }
  .pl.muted { fill: #6F7882; font-weight: 500; }
  .gbox { fill: color-mix(in srgb, var(--g) 12%, transparent); stroke: color-mix(in srgb, var(--g) 70%, transparent); stroke-width: 1.2; }
  .gband { fill: var(--g); }
  .gl { font: 700 9.5px var(--mono); fill: #fff; letter-spacing: .06em; text-transform: uppercase; pointer-events: none; }
  .grp { cursor: pointer; }
  .gline { fill: var(--g); }
</style>
