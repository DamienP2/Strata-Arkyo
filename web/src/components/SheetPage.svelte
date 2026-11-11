<script lang="ts">
  // Une feuille de la planche d'élévations, en SVG millimétrique : sert à l'écran (édition)
  // et à l'export (PDF vectoriel, PNG, JPG). Polices standard pour un PDF fidèle.
  import { fnColor } from '../lib/catalog.svelte';
  import { BLOCK, FACE_LABEL, MARGIN, RACK_HEAD, RACK_NUM, itemSize, pageSize, rackGeom, rackSize, type RackData, type Sheet, type SheetItem, type SheetPage } from '../lib/sheet';

  interface Info { project: string; client: string; venue: string; site: string; date: string; locations: Record<string, { name: string; code: string }> }
  let { sheet, page, racks, info, index, total, editable = false, selected = null, onpick }: {
    sheet: Sheet; page: SheetPage; racks: Record<string, RackData>; info: Info; index: number; total: number;
    editable?: boolean; selected?: string | null; onpick?: (it: SheetItem, e: PointerEvent) => void;
  } = $props();

  const F = 'Helvetica, Arial, sans-serif';
  const size = $derived(pageSize(sheet));
  const k = $derived(sheet.scale);
  const units = (cid: string) => racks[cid]?.units ?? 42;
  const fit = (s: string, width: number, fs: number) => { const n = Math.max(1, Math.floor(width / (fs * 0.56))); return s.length > n ? s.slice(0, Math.max(1, n - 1)) + '…' : s; };
  const COLS: Record<string, number> = { full: 1, half: 2, third: 3, quarter: 4 };
  const color = (e: any) => (e.function ? fnColor(e.function) : '#8A919B');
  const block = $derived({ x: size[0] - MARGIN - BLOCK[0], y: size[1] - MARGIN - BLOCK[1] });
  const cells = $derived.by(() => {
    const b = sheet.block;
    return [
      [{ l: 'Site', v: info.site, w: 70 }, { l: 'Titre', v: b.title, w: 110, big: true }],
      [{ l: 'Projet', v: info.project, w: 70 }, { l: 'Client', v: info.client, w: 60 }, { l: 'Lieu', v: info.venue, w: 50 }],
      [{ l: 'Référence', v: b.reference, w: 45 }, { l: 'Date', v: info.date, w: 30 }, { l: 'Planche', v: `${index + 1} / ${total}`, w: 25 }, { l: 'Version', v: b.version, w: 20 }, { l: 'Échelle', v: `1:${k}`, w: 20 }, { l: 'Format', v: sheet.paper, w: 20 }],
      [{ l: 'Auteur', v: b.author, w: 45 }, { l: 'Vérificateur', v: b.checker, w: 45 }, { l: 'Normes / notes', v: [b.standards, b.notes].filter(Boolean).join(' · '), w: 90 }],
    ];
  });
  const ROWH = [12, 10, 11, 11];
</script>

{#snippet rack(it: SheetItem)}
  {@const r = racks[it.container_id ?? '']}
  {@const n = units(it.container_id ?? '')}
  {@const [w, h] = rackSize(n, k)}
  {@const g = rackGeom(k)}
  {@const x0 = it.x + RACK_NUM}
  {@const y0 = it.y + RACK_HEAD}
  {@const x1 = x0 + g.rail}
  {@const y1 = y0 + g.pad}
  {@const step = g.u >= 2.4 ? 1 : g.u >= 1.2 ? 2 : 5}
  <text x={x0} y={it.y + 4.6} font-family={F} font-size="3.6" font-weight="bold" fill="#15171A">{fit(r?.name ?? 'Élément retiré', w - RACK_NUM - 12, 3.6)}</text>
  <text x={it.x + w} y={it.y + 4.6} font-family={F} font-size="2.6" fill="#5B636D" text-anchor="end">{n} U</text>
  <text x={x0} y={it.y + 8} font-family={F} font-size="2.4" fill="#5B636D" letter-spacing=".2">{FACE_LABEL[it.face ?? 'front'].toUpperCase()}</text>
  <rect x={x0} y={y0} width={w - RACK_NUM} height={h - RACK_HEAD} rx="0.8" fill="#FFFFFF" stroke="#15171A" stroke-width="0.5" />
  <rect x={x0} y={y0} width={g.rail} height={h - RACK_HEAD} fill="#D9DDE2" stroke="#15171A" stroke-width="0.25" />
  <rect x={x1 + g.inner} y={y0} width={g.rail} height={h - RACK_HEAD} fill="#D9DDE2" stroke="#15171A" stroke-width="0.25" />
  {#each Array(n + 1) as _, i}<line x1={x1} x2={x1 + g.inner} y1={y1 + i * g.u} y2={y1 + i * g.u} stroke="#E3E6EA" stroke-width="0.12" />{/each}
  {#each Array(n) as _, i}{#if (n - i) % step === 0 || n - i === 1}<text x={x0 - 1.2} y={y1 + i * g.u + g.u / 2 + 0.8} font-family={F} font-size={Math.min(2.2, g.u * 0.8)} fill="#5B636D" text-anchor="end">{n - i}</text>{/if}{/each}
  {#each (r?.elements ?? []).filter((e) => e.placement.face === (it.face ?? 'front') && !e.placement.shelf_id) as e (e.placement.id)}
    {@const cols = COLS[e.placement.width] ?? 1}
    {@const ew = g.inner / cols}
    {@const ex = x1 + ew * (e.placement.col ?? 0)}
    {@const eh = e.height_u * g.u}
    {@const ey = y1 + (n - (e.placement.u_start - 1) - e.height_u) * g.u}
    {@const fs = Math.max(1.1, Math.min(2.8, eh * 0.42))}
    {@const panel = e.accessory?.kind === 'panel' ? e.accessory.panel : null}
    {@const acc = e.nature === 'accessory' && !panel}
    <rect x={ex + 0.15} y={ey + 0.1} width={ew - 0.3} height={eh - 0.2} fill={acc ? '#EEF0F2' : '#F6F7F9'} stroke="#3A4048" stroke-width="0.22" />
    {#if !acc}<rect x={ex + 0.15} y={ey + 0.1} width={Math.min(1.4, ew * 0.08)} height={eh - 0.2} fill={color(e)} />{/if}
    {#if panel}
      {@const per = panel.per_u}
      {@const pw = (ew * 0.62) / per}
      {#each panel.slots as s}
        {@const row = Math.floor((s.index - 1) / per)}
        {@const col = (s.index - 1) % per}
        <circle cx={ex + ew * 0.36 + (col + s.width / 2) * pw} cy={ey + (row + 0.5) * (eh / panel.height_u)} r={Math.min(pw * 0.36, (eh / panel.height_u) * 0.3)} fill="#FFFFFF" stroke={s.function ? fnColor(s.function) : '#2459E0'} stroke-width="0.25" />
      {/each}
      <text x={ex + 2} y={ey + eh / 2 + fs * 0.35} font-family={F} font-size={fs} fill="#15171A">{fit(e.label, ew * 0.33, fs)}</text>
    {:else}
      {@const carried = (r?.elements ?? []).filter((x) => x.placement.shelf_id === e.placement.id).map((x) => x.label)}
      <text x={ex + (acc ? 1.2 : 2.2)} y={ey + eh / 2 + fs * 0.35} font-family={F} font-size={fs} fill={acc ? '#5B636D' : '#15171A'} font-weight={acc ? 'normal' : 'bold'}>{fit(carried.length ? `${e.label} · ${carried.join(', ')}` : e.label, ew - (e.tag ? 14 : 4), fs)}</text>
      {#if e.tag && ew > 24}<text x={ex + ew - 1.2} y={ey + eh / 2 + fs * 0.35} font-family="Courier, monospace" font-size={fs * 0.9} fill="#5B636D" text-anchor="end">{e.tag}</text>{/if}
    {/if}
  {/each}
{/snippet}

<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {size[0]} {size[1]}" width="{size[0]}mm" height="{size[1]}mm" class="sheet-page">
  <rect x="0" y="0" width={size[0]} height={size[1]} fill="#FFFFFF" />
  <rect x={MARGIN / 2} y={MARGIN / 2} width={size[0] - MARGIN} height={size[1] - MARGIN} fill="none" stroke="#15171A" stroke-width="0.35" />
  {#each page.items as it (it.id)}
    {@const [w, h] = itemSize(it, k, units)}
    <g class="it" class:edit={editable} onpointerdown={(e) => onpick?.(it, e)} role={editable ? 'button' : undefined} aria-label={editable ? 'Déplacer' : undefined}>
      {#if editable}<rect x={it.x - 1} y={it.y - 1} width={w + 2} height={h + 2} fill="transparent" stroke={selected === it.id ? '#4F6BFF' : 'transparent'} stroke-width="0.5" stroke-dasharray="2 1" />{/if}
      {#if it.kind === 'rack'}{@render rack(it)}
      {:else if it.kind === 'location'}
        {@const l = info.locations[it.location_id ?? '']}
        <text x={it.x} y={it.y + 6} font-family={F} font-size="5.2" font-weight="bold" fill="#15171A">{fit(l?.name ?? it.label ?? 'Lieu', w - 20, 5.2)}</text>
        {#if l?.code}<text x={it.x + w} y={it.y + 6} font-family="Courier, monospace" font-size="3" fill="#5B636D" text-anchor="end">{l.code}</text>{/if}
        <line x1={it.x} x2={it.x + w} y1={it.y + 8.6} y2={it.y + 8.6} stroke="#15171A" stroke-width="0.5" />
      {:else}
        <rect x={it.x} y={it.y} width={w} height={h} fill="#FAFBFC" stroke="#8A919B" stroke-width="0.35" stroke-dasharray="2.5 1.5" />
        <text x={it.x + w / 2} y={it.y + h / 2} font-family={F} font-size="3.4" fill="#5B636D" text-anchor="middle">{fit(it.label || 'Emplacement réservé', w - 4, 3.4)}</text>
        <text x={it.x + w / 2} y={it.y + h / 2 + 4.4} font-family={F} font-size="2.4" fill="#8A919B" text-anchor="middle">RÉSERVÉ</text>
      {/if}
    </g>
  {/each}
  {#if sheet.titleBlock}
    <g>
      <rect x={block.x} y={block.y} width={BLOCK[0]} height={BLOCK[1]} fill="#FFFFFF" stroke="#15171A" stroke-width="0.5" />
      {#each cells as row, ri}
        {@const ry = block.y + ROWH.slice(0, ri).reduce((a, b) => a + b, 0)}
        {#if ri > 0}<line x1={block.x} x2={block.x + BLOCK[0]} y1={ry} y2={ry} stroke="#15171A" stroke-width="0.25" />{/if}
        {#each row as c, ci}
          {@const cx = block.x + row.slice(0, ci).reduce((a, b) => a + b.w, 0)}
          {#if ci > 0}<line x1={cx} x2={cx} y1={ry} y2={ry + ROWH[ri]!} stroke="#15171A" stroke-width="0.25" />{/if}
          <text x={cx + 1.4} y={ry + 2.8} font-family={F} font-size="1.9" fill="#5B636D" letter-spacing=".15">{c.l.toUpperCase()}</text>
          <text x={cx + 1.4} y={ry + ROWH[ri]! - 2.2} font-family={F} font-size={'big' in c ? 4.4 : 3} font-weight={'big' in c || ri === 1 && ci === 0 ? 'bold' : 'normal'} fill="#15171A">{fit(c.v || '—', c.w - 3, 'big' in c ? 4.4 : 3)}</text>
        {/each}
      {/each}
    </g>
  {/if}
</svg>

<style>
  .sheet-page { display: block; width: 100%; height: auto; background: #fff; }
  .it.edit { cursor: grab; }
  .it.edit:active { cursor: grabbing; }
</style>
