<script lang="ts">
  // Une feuille de la planche d'élévations, en SVG millimétrique : sert à l'écran (édition)
  // et à l'export (PDF vectoriel, PNG, JPG). Polices Inter et JetBrains Mono incorporées au PDF.
  // Impression : cadre à 10 mm du bord, aucun texte sous 6 pt (2,15 mm), petits textes en #333.
  // Baies : vues avant, arrière, côté et 3D ; mobilier : face, côté, dessus, 3D et casquettes.
  import { fnColor } from '../lib/catalog.svelte';
  import {
    BLOCK, CASQ_W, CASQ_EXTRA, FRAME, RACK_NUM, RACK_VIEWS, DESK_VIEWS, U_MM,
    casqProfile, deskExtent, groupLayout, itemSize, pageSize, rackGeom, rackBodyH, usableW,
    type Ctx, type DeskData, type RackData, type Sheet, type SheetItem, type SheetPage, type ViewBox,
  } from '../lib/sheet';

  interface Info { project: string; client: string; venue: string; site: string; date: string; locations: Record<string, { name: string; code: string }> }
  let { sheet, page, ctx, info, index, total, shots = {}, editable = false, selected = null, onpick }: {
    sheet: Sheet; page: SheetPage; ctx: Ctx; info: Info; index: number; total: number; shots?: Record<string, string>;
    editable?: boolean; selected?: string | null; onpick?: (it: SheetItem, e: PointerEvent) => void;
  } = $props();

  const F = 'Inter, Helvetica, Arial, sans-serif', MONO = 'JetBrainsMono, Courier, monospace';
  const INK = '#15171A', MUTE = '#333333', LINE = '#3A4048';
  /** Plus petit corps imprimé : 6 pt ≈ 2,15 mm. Un libellé qui ne tient pas à ce corps n'est pas dessiné. */
  const MIN = 2.15;
  const fsU = (eh: number, max: number) => Math.max(MIN, Math.min(max, eh * 0.42));
  const size = $derived(pageSize(sheet));
  const k = $derived(sheet.scale);
  const maxW = $derived(usableW(sheet));
  // Textes : jamais de points de suspension tant que c'est possible. Un texte un peu trop long
  // est resserré à la largeur disponible (textLength) ; seul un texte vraiment trop long est
  // raccourci, au dernier mot entier.
  const est = (s: string, fs: number) => s.length * fs * 0.56;
  const fit = (s: string, width: number, fs: number) => {
    const max = Math.max(1, Math.floor((width * 1.6) / (fs * 0.56)));
    if (s.length <= max) return s;
    const cut = s.slice(0, max), sp = cut.lastIndexOf(' ');
    return (sp > max * 0.6 ? cut.slice(0, sp) : cut).trimEnd();
  };
  // Pas de resserrement d'un seul caractère : svg2pdf divise par (n − 1) et abandonne le rendu.
  const sq = (s: string, width: number, fs: number) => { const t = fit(s, width, fs); return t.length > 1 && est(t, fs) > width ? Math.max(1, width) : undefined; };
  const COLS: Record<string, number> = { full: 1, half: 2, third: 3, quarter: 4 };
  const color = (e: any) => (e.function ? fnColor(e.function) : '#8A919B');
  const block = $derived({ x: size[0] - FRAME - BLOCK[0], y: size[1] - FRAME - BLOCK[1] });
  const cells = $derived.by(() => {
    const b = sheet.block;
    // Grille de 30 mm : les séparations verticales tombent les unes sous les autres.
    return [
      [{ l: 'Site', v: info.site, w: 60 }, { l: 'Titre', v: b.title, w: 120, big: true }],
      [{ l: 'Projet', v: info.project, w: 60 }, { l: 'Client', v: info.client, w: 60 }, { l: 'Lieu', v: info.venue, w: 60 }],
      [{ l: 'Référence', v: b.reference, w: 30 }, { l: 'Date', v: info.date, w: 30 }, { l: 'Planche', v: `${index + 1} / ${total}`, w: 30 }, { l: 'Version', v: b.version, w: 30 }, { l: 'Échelle', v: `1:${k}`, w: 30 }, { l: 'Format', v: sheet.paper, w: 30 }],
      [{ l: 'Auteur', v: b.author, w: 60 }, { l: 'Vérificateur', v: b.checker, w: 30 }, { l: 'Normes / notes', v: [b.standards, b.notes].filter(Boolean).join(' · '), w: 90 }],
    ];
  });
  const ROWH = [12, 10, 11, 11];
  const title = (it: SheetItem) => {
    if (it.kind === 'rack') { const r = ctx.racks[it.container_id ?? '']; return [r?.name ?? 'Élément retiré', r ? `${r.units} U · P ${r.depth_mm} mm` : '']; }
    const d = ctx.desks[it.container_id ?? '']; return [d?.name ?? 'Élément retiré', d ? `${d.length_mm} × ${d.depth_mm} mm${d.casq.length ? ` · ${d.casq.length} casquette${d.casq.length > 1 ? 's' : ''}` : ''}` : ''];
  };
  const viewLabel = (it: SheetItem, b: ViewBox) => b.v === 'casq' ? (ctx.racks[b.cid ?? '']?.name ?? 'Casquette') : (it.kind === 'rack' ? RACK_VIEWS : DESK_VIEWS)[b.v as never] ?? '';
  const pts = (p: [number, number][]) => p.map(([x, y]) => `${x.toFixed(2)},${y.toFixed(2)}`).join(' ');
</script>

<!-- Élévation 19″ (face avant ou arrière) d'une baie ou d'une casquette. -->
{#snippet elevation(x: number, y: number, r: RackData | undefined, face: 'front' | 'rear')}
  {@const n = r?.units ?? 42}
  {@const g = rackGeom(k)}
  {@const x0 = x + RACK_NUM}
  {@const w = g.inner + 2 * g.rail}
  {@const h = rackBodyH(n, k)}
  {@const x1 = x0 + g.rail}
  {@const y1 = y + g.pad}
  {@const step = g.u >= 2.6 ? 1 : g.u >= 1.3 ? 2 : g.u >= 0.52 ? 5 : 10}
  {@const els = (r?.elements ?? []).filter((e) => !e.placement.shelf_id && (face === 'front' ? e.placement.face === 'front' : e.placement.face !== 'front'))}
  <rect x={x0} y={y} width={w} height={h} rx="0.8" fill="#FFFFFF" stroke={INK} stroke-width="0.5" />
  <rect x={x0} y={y} width={g.rail} height={h} fill="#D9DDE2" stroke={INK} stroke-width="0.25" />
  <rect x={x1 + g.inner} y={y} width={g.rail} height={h} fill="#D9DDE2" stroke={INK} stroke-width="0.25" />
  {#if g.u >= 0.8}{#each Array(n + 1) as _, i}<line x1={x1} x2={x1 + g.inner} y1={y1 + i * g.u} y2={y1 + i * g.u} stroke="#E3E6EA" stroke-width="0.12" />{/each}{/if}
  {#each Array(n) as _, i}{#if (n - i) % step === 0 || n - i === 1}<text x={x0 - 1.2} y={y1 + i * g.u + g.u / 2 + 0.7} font-family={F} font-size={MIN} fill={MUTE} text-anchor="end">{n - i}</text>{/if}{/each}
  {#each els as e (e.placement.id)}
    {@const cols = COLS[e.placement.width] ?? 1}
    {@const ew = g.inner / cols}
    {@const ex = x1 + ew * (e.placement.col ?? 0)}
    {@const eh = e.height_u * g.u}
    {@const ey = y1 + (n - (e.placement.u_start - 1) - e.height_u) * g.u}
    {@const fs = fsU(eh, 2.8)}
    {@const panel = e.accessory?.kind === 'panel' ? e.accessory.panel : null}
    {@const acc = e.nature === 'accessory' && !panel}
    {@const inner = e.placement.face === 'rear_inner'}
    <rect x={ex + 0.15} y={ey + 0.1} width={ew - 0.3} height={eh - 0.2} fill={acc ? '#EEF0F2' : '#F6F7F9'} stroke={LINE} stroke-width="0.22" stroke-dasharray={inner ? '1 0.6' : undefined} />
    {#if !acc}<rect x={ex + 0.15} y={ey + 0.1} width={Math.min(1.4, ew * 0.08)} height={eh - 0.2} fill={color(e)} />{/if}
    {#if panel && face === 'front'}
      {@const per = panel.per_u}
      {@const pw = (ew * 0.62) / per}
      {#each panel.slots as s}
        {@const row = Math.floor((s.index - 1) / per)}
        {@const col = (s.index - 1) % per}
        <circle cx={ex + ew * 0.36 + (col + s.width / 2) * pw} cy={ey + (row + 0.5) * (eh / panel.height_u)} r={Math.min(pw * 0.36, (eh / panel.height_u) * 0.3)} fill="#FFFFFF" stroke={s.function ? fnColor(s.function) : '#2459E0'} stroke-width="0.25" />
      {/each}
      {#if eh >= 1.6}<text x={ex + 2} y={ey + eh / 2 + fs * 0.35} font-family={F} font-size={fs} fill={INK} textLength={sq(e.label, ew * 0.33, fs)} lengthAdjust="spacingAndGlyphs">{fit(e.label, ew * 0.33, fs)}</text>{/if}
    {:else if eh >= 1.6}
      {@const carried = (r?.elements ?? []).filter((x) => x.placement.shelf_id === e.placement.id).map((x) => x.label)}
      <text x={ex + (acc ? 1.2 : 2.2)} y={ey + eh / 2 + fs * 0.35} font-family={F} font-size={fs} fill={acc ? MUTE : INK} font-weight={acc ? 'normal' : 'bold'} textLength={sq(carried.length ? `${e.label} · ${carried.join(', ')}` : e.label, ew - (e.tag ? 14 : 4), fs)} lengthAdjust="spacingAndGlyphs">{fit(carried.length ? `${e.label} · ${carried.join(', ')}` : e.label, ew - (e.tag ? 14 : 4), fs)}</text>
      {#if e.tag && ew > 24}<text x={ex + ew - 1.2} y={ey + eh / 2 + fs * 0.35} font-family={MONO} font-size={Math.max(MIN, fs * 0.9)} fill={MUTE} text-anchor="end">{e.tag}</text>{/if}
    {/if}
  {/each}
  {#if face === 'rear'}
    <!-- PDU verticaux, sur les montants (vue arrière : gauche et droite comme vus de l'arrière). -->
    {#each r?.vpdus ?? [] as v, i (v.article_id)}
      {@const L = Math.min(h - 2 * g.pad, v.accessory.length_mm / k)}
      {@const px = v.accessory.side === 'left' ? x0 + g.rail * 0.1 : x1 + g.inner + g.rail * 0.1}
      {@const py = y + (h - L) / 2}
      {@const pw = g.rail * 0.8}
      <rect x={px} y={py} width={pw} height={L} rx="0.4" fill="#2B2F35" stroke={INK} stroke-width="0.25" />
      {#each Array(Math.min(60, v.accessory.outlets)) as _, j}<rect x={px + pw * 0.25} y={py + 3 + ((L - 5) / v.accessory.outlets) * j} width={pw * 0.5} height={Math.max(0.3, ((L - 5) / v.accessory.outlets) * 0.55)} fill="#FFFFFF" />{/each}
    {/each}
  {/if}
{/snippet}

<!-- Vue de côté d'une baie : avant à gauche, profondeur des appareils depuis leurs montants. -->
{#snippet side(x: number, y: number, b: ViewBox, r: RackData | undefined)}
  {@const n = r?.units ?? 42}
  {@const g = rackGeom(k)}
  {@const D = (r?.depth_mm ?? 800) / k}
  {@const x0 = x + 30 / k}
  {@const fr = x0 + 70 / k}
  {@const rr = x0 + D - (r?.c?.rear_rails_mm ?? 70) / k}
  {@const y1 = y + g.pad}
  <rect x={x0} y={y} width={D} height={b.h} rx="0.8" fill="#FFFFFF" stroke={INK} stroke-width="0.5" />
  <line x1={fr} x2={fr} y1={y} y2={y + b.h} stroke={INK} stroke-width="0.35" />
  <line x1={rr} x2={rr} y1={y} y2={y + b.h} stroke={INK} stroke-width="0.35" />
  {#each (r?.elements ?? []).filter((e) => !e.placement.shelf_id) as e (e.placement.id)}
    {@const eh = e.height_u * g.u}
    {@const ey = y1 + (n - (e.placement.u_start - 1) - e.height_u) * g.u}
    {@const ed = Math.min(D - 2 / k, ((e.depth_mm ?? (e.nature === 'accessory' ? 30 : 300)) / k))}
    {@const front = e.placement.face === 'front'}
    {@const ex = front ? fr : rr - ed}
    {@const fs = fsU(eh, 2.4)}
    <rect x={ex} y={ey + 0.1} width={Math.max(0.4, ed)} height={eh - 0.2} fill={e.nature === 'accessory' ? '#EEF0F2' : '#F6F7F9'} stroke={LINE} stroke-width="0.22" />
    <rect x={front ? ex - 0.6 : ex + ed} y={ey + 0.1} width="0.6" height={eh - 0.2} fill={color(e)} />
    {#if eh >= 1.6 && ed > 8}<text x={ex + (front ? 1.2 : ed - 1.2)} y={ey + eh / 2 + fs * 0.35} font-family={F} font-size={fs} fill={INK} text-anchor={front ? 'start' : 'end'} textLength={sq(e.label, ed - 2.4, fs)} lengthAdjust="spacingAndGlyphs">{fit(e.label, ed - 2.4, fs)}</text>{/if}
  {/each}
  {#each r?.vpdus ?? [] as v (v.article_id)}
    {@const L = Math.min(b.h - 2 * g.pad, v.accessory.length_mm / k)}
    <rect x={rr + 4 / k} y={y + (b.h - L) / 2} width={45 / k} height={L} rx="0.3" fill="#2B2F35" stroke={INK} stroke-width="0.25" />
  {/each}
  <text x={x0 + 1} y={y + b.h + 3} font-family={F} font-size={MIN} fill={MUTE}>AVANT</text>
  <text x={x0 + D - 1} y={y + b.h + 3} font-family={F} font-size={MIN} fill={MUTE} text-anchor="end">ARRIÈRE</text>
{/snippet}

<!-- Mobilier vu de face (côté opérateur), de côté (opérateur à gauche) ou de dessus. -->
{#snippet desk(x: number, y: number, b: ViewBox, d: DeskData | undefined)}
  {#if d}
    {@const e = deskExtent(d)}
    {@const T = 32 / k}
    {@const J = 45 / k}
    {@const s = 1 / k}
    {#if b.v === 'front'}
      {@const fy = y + b.h}
      {@const top = fy - d.height_mm * s}
      {#each d.casq as c (c.id)}
        {@const Hc = (Math.max(1, c.units) * U_MM + CASQ_EXTRA) * Math.cos((c.tilt_deg * Math.PI) / 180) * s}
        {@const cx = x + Math.max(0, Math.min(d.length_mm - CASQ_W, c.offset_mm)) * s}
        {@const cw = CASQ_W * s}
        <rect x={cx} y={top - Hc} width={cw} height={Hc} fill="#F6F7F9" stroke={INK} stroke-width="0.35" />
        {@const cos = Math.cos((c.tilt_deg * Math.PI) / 180)}
        {@const uh = U_MM * s * cos}
        {@const iy = top - Hc + 25 * s * cos}
        {@const ix = cx + 25 * s}
        {@const iw = cw - 50 * s}
        <rect x={ix} y={iy} width={iw} height={Math.max(0.5, c.units * uh)} fill="#FFFFFF" stroke={LINE} stroke-width="0.18" />
        {#each (ctx.racks[c.id]?.elements ?? []).filter((e) => e.placement.face === 'front' && !e.placement.shelf_id) as e (e.placement.id)}
          {@const cols = COLS[e.placement.width] ?? 1}
          {@const ew = iw / cols}
          {@const ex = ix + ew * (e.placement.col ?? 0)}
          {@const eh = e.height_u * uh}
          {@const ey = iy + (c.units - (e.placement.u_start - 1) - e.height_u) * uh}
          {@const fs = fsU(eh, 2.2)}
          <rect x={ex + 0.1} y={ey + 0.08} width={ew - 0.2} height={Math.max(0.2, eh - 0.16)} fill="#F6F7F9" stroke={LINE} stroke-width="0.16" />
          <rect x={ex + 0.1} y={ey + 0.08} width={Math.min(0.8, ew * 0.08)} height={Math.max(0.2, eh - 0.16)} fill={color(e)} />
          {#if eh >= 1.6}<text x={ex + 1.3} y={ey + eh / 2 + fs * 0.35} font-family={F} font-size={fs} fill={INK} textLength={sq(e.label, ew - 2, fs)} lengthAdjust="spacingAndGlyphs">{fit(e.label, ew - 2, fs)}</text>{/if}
        {/each}
        {#if cw > 14 && !(ctx.racks[c.id]?.elements ?? []).some((x) => x.placement.face === 'front')}<text x={cx + cw / 2} y={top - Hc / 2 + 1} font-family={F} font-size={Math.max(MIN, Math.min(2.4, cw / 12))} fill={MUTE} text-anchor="middle" textLength={sq(c.name, cw - 3, Math.max(MIN, Math.min(2.4, cw / 12)))} lengthAdjust="spacingAndGlyphs">{fit(c.name, cw - 3, Math.max(MIN, Math.min(2.4, cw / 12)))}</text>{/if}
      {/each}
      <rect x={x} y={top} width={e.L * s} height={T} fill="#2B2D31" stroke={INK} stroke-width="0.3" />
      <rect x={x} y={top + T} width={J} height={fy - top - T} fill="#C9CDD3" stroke={INK} stroke-width="0.3" />
      <rect x={x + e.L * s - J} y={top + T} width={J} height={fy - top - T} fill="#C9CDD3" stroke={INK} stroke-width="0.3" />
      <rect x={x + 50 * s} y={top + T} width={e.L * s - 100 * s} height={(d.height_mm * 0.42) * s} fill="#E7E9EC" stroke={LINE} stroke-width="0.2" />
    {:else if b.v === 'side'}
      {@const fy = y + b.h}
      {#each d.casq as c (c.id)}
        <polygon points={pts(casqProfile(d, c).map(([z, yy]) => [x + z * s, fy - yy * s]))} fill="#F6F7F9" stroke={INK} stroke-width="0.35" />
      {/each}
      <rect x={x} y={fy - d.height_mm * s} width={d.depth_mm * s} height={T} fill="#2B2D31" stroke={INK} stroke-width="0.3" />
      <rect x={x + 20 * s} y={fy - d.height_mm * s + T} width={(d.depth_mm - 40) * s} height={d.height_mm * s - T} fill="#C9CDD3" fill-opacity="0.55" stroke={INK} stroke-width="0.3" />
      <text x={x + 1} y={fy + 3} font-family={F} font-size={MIN} fill={MUTE}>OPÉRATEUR</text>
    {:else if b.v === 'top'}
      <rect x={x} y={y} width={e.L * s} height={d.depth_mm * s} fill="#FFFFFF" stroke={INK} stroke-width="0.5" />
      <rect x={x} y={y + (d.depth_mm - 110) * s} width={e.L * s} height={110 * s} fill="#E7E9EC" stroke={LINE} stroke-width="0.2" />
      {#each d.casq as c (c.id)}
        {@const p = casqProfile(d, c)}
        {@const z0 = Math.min(...p.map((q) => q[0]))}
        {@const z1 = Math.max(...p.map((q) => q[0]))}
        {@const cx = x + Math.max(0, Math.min(d.length_mm - CASQ_W, c.offset_mm)) * s}
        <rect x={cx} y={y + (d.depth_mm - z1) * s} width={CASQ_W * s} height={(z1 - z0) * s} fill="#F6F7F9" stroke={INK} stroke-width="0.35" />
        {#if CASQ_W * s > 14}<text x={cx + (CASQ_W * s) / 2} y={y + (d.depth_mm - (z0 + z1) / 2) * s + 1} font-family={F} font-size="2.2" fill={INK} text-anchor="middle" textLength={sq(c.name, CASQ_W * s - 3, 2.2)} lengthAdjust="spacingAndGlyphs">{fit(c.name, CASQ_W * s - 3, 2.2)}</text>{/if}
      {/each}
      <text x={x + 1} y={y + d.depth_mm * s + 3} font-family={F} font-size={MIN} fill={MUTE}>CÔTÉ OPÉRATEUR</text>
    {/if}
  {/if}
{/snippet}

{#snippet group(it: SheetItem)}
  {@const L = groupLayout(it, k, ctx, maxW)}
  {@const [name, sub] = title(it)}
  <text x={it.x} y={it.y + 4} font-family={F} font-size="3.6" font-weight="bold" fill={INK} textLength={sq(name, Math.max(30, L.w - 40), 3.6)} lengthAdjust="spacingAndGlyphs">{fit(name, Math.max(30, L.w - 40), 3.6)}</text>
  {#if sub}<text x={it.x + L.w} y={it.y + 4} font-family={F} font-size="2.4" fill={MUTE} text-anchor="end">{L.w > 60 ? sub : ''}</text>{/if}
  {#each L.views as b, i (i)}
    {@const bx = it.x + b.x}
    {@const by = it.y + b.y}
    <text x={bx + (b.v === 'front' || b.v === 'rear' || b.v === 'casq' ? RACK_NUM : 0)} y={by - 1.4} font-family={F} font-size="2.3" fill={MUTE} letter-spacing=".2" textLength={sq(viewLabel(it, b).toUpperCase(), b.w, 2.3)} lengthAdjust="spacingAndGlyphs">{fit(viewLabel(it, b).toUpperCase(), b.w, 2.3)}</text>
    {#if b.v === '3d'}
      {#if shots[it.id]}<image href={shots[it.id]} x={bx} y={by} width={b.w} height={b.h} preserveAspectRatio="xMidYMid meet" />
      {:else}<rect x={bx} y={by} width={b.w} height={b.h} fill="#F6F7F9" stroke="#C9CDD3" stroke-width="0.3" stroke-dasharray="2 1" /><text x={bx + b.w / 2} y={by + b.h / 2} font-family={F} font-size="2.6" fill={MUTE} text-anchor="middle">Vue 3D…</text>{/if}
    {:else if it.kind === 'rack'}
      {#if b.v === 'side'}{@render side(bx, by, b, ctx.racks[it.container_id ?? ''])}
      {:else}{@render elevation(bx, by, ctx.racks[it.container_id ?? ''], b.v === 'rear' ? 'rear' : 'front')}{/if}
    {:else if b.v === 'casq'}{@render elevation(bx, by, ctx.racks[b.cid ?? ''], 'front')}
    {:else}{@render desk(bx, by, b, ctx.desks[it.container_id ?? ''])}{/if}
  {/each}
{/snippet}

<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 {size[0]} {size[1]}" width="{size[0]}mm" height="{size[1]}mm" class="sheet-page">
  <rect x="0" y="0" width={size[0]} height={size[1]} fill="#FFFFFF" />
  <rect x={FRAME} y={FRAME} width={size[0] - 2 * FRAME} height={size[1] - 2 * FRAME} fill="none" stroke={INK} stroke-width="0.35" />
  {#each page.items as it (it.id)}
    {@const [w, h] = itemSize(it, k, ctx, maxW)}
    <g class="it" class:edit={editable} onpointerdown={(e) => onpick?.(it, e)} role={editable ? 'button' : undefined} aria-label={editable ? 'Sélectionner' : undefined}>
      {#if editable}<rect x={it.x - 1.5} y={it.y - 1.5} width={w + 3} height={h + 3} rx="1" fill={selected === it.id ? 'rgba(79,107,255,.05)' : 'transparent'} stroke={selected === it.id ? '#4F6BFF' : 'transparent'} stroke-width="0.4" />{/if}
      {#if it.kind === 'rack' || it.kind === 'desk'}{@render group(it)}
      {:else if it.kind === 'location'}
        {@const l = info.locations[it.location_id ?? '']}
        <text x={it.x} y={it.y + 6} font-family={F} font-size="5.2" font-weight="bold" fill={INK} textLength={sq(l?.name ?? it.label ?? 'Lieu', w - 20, 5.2)} lengthAdjust="spacingAndGlyphs">{fit(l?.name ?? it.label ?? 'Lieu', w - 20, 5.2)}</text>
        {#if l?.code}<text x={it.x + w} y={it.y + 6} font-family={MONO} font-size="3" fill={MUTE} text-anchor="end">{l.code}</text>{/if}
        <line x1={it.x} x2={it.x + w} y1={it.y + 8.6} y2={it.y + 8.6} stroke={INK} stroke-width="0.5" />
      {:else}
        <rect x={it.x} y={it.y} width={w} height={h} fill="#FAFBFC" stroke="#8A919B" stroke-width="0.35" stroke-dasharray="2.5 1.5" />
        <text x={it.x + w / 2} y={it.y + h / 2} font-family={F} font-size="3.4" fill={MUTE} text-anchor="middle" textLength={sq(it.label || 'Emplacement réservé', w - 4, 3.4)} lengthAdjust="spacingAndGlyphs">{fit(it.label || 'Emplacement réservé', w - 4, 3.4)}</text>
        <text x={it.x + w / 2} y={it.y + h / 2 + 4.4} font-family={F} font-size="2.4" fill={MUTE} text-anchor="middle">RÉSERVÉ</text>
      {/if}
    </g>
  {/each}
  {#if sheet.titleBlock}
    <g>
      <rect x={block.x} y={block.y} width={BLOCK[0]} height={BLOCK[1]} fill="#FFFFFF" stroke={INK} stroke-width="0.5" />
      {#each cells as row, ri}
        {@const ry = block.y + ROWH.slice(0, ri).reduce((a, b) => a + b, 0)}
        {#if ri > 0}<line x1={block.x} x2={block.x + BLOCK[0]} y1={ry} y2={ry} stroke={INK} stroke-width="0.25" />{/if}
        {#each row as c, ci}
          {@const cx = block.x + row.slice(0, ci).reduce((a, b) => a + b.w, 0)}
          {#if ci > 0}<line x1={cx} x2={cx} y1={ry} y2={ry + ROWH[ri]!} stroke={INK} stroke-width="0.25" />{/if}
          <text x={cx + 1.4} y={ry + 3.1} font-family={F} font-size="2.4" fill={MUTE} letter-spacing=".1">{c.l.toUpperCase()}</text>
          <text x={cx + 1.4} y={ry + ROWH[ri]! - 2.2} font-family={F} font-size={'big' in c ? 4.4 : 3} font-weight={'big' in c || ri === 1 && ci === 0 ? 'bold' : 'normal'} fill={INK} textLength={sq(c.v || '—', c.w - 3, 'big' in c ? 4.4 : 3)} lengthAdjust="spacingAndGlyphs">{fit(c.v || '—', c.w - 3, 'big' in c ? 4.4 : 3)}</text>
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
