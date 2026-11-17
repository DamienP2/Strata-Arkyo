<script lang="ts">
  // Une feuille des planches, en SVG millimétrique : sert à l'écran (édition) et à l'export
  // (PDF vectoriel, PNG, JPG). Polices Inter et JetBrains Mono incorporées au PDF.
  // Impression : cadre à 10 mm du bord, aucun texte sous 6 pt (2,15 mm), petits textes en #333.
  // Baies : vues avant, arrière, côté et 3D, en rendu réel (façades simplifiées), schématique
  // (aplats à la couleur de la famille de signal, catégorie en texte) ou 3D ; mobilier : face,
  // côté, dessus, 3D et casquettes ; synoptiques (le dessin de l'écran Synoptique, lib/render,
  // cadré sur tout son contenu et mis à l'échelle). Chaque dessin a sa propre échelle (`zoom`),
  // réglée par les poignées de coin. Couleurs et traits de pdfkit.
  import { familyColor } from '../lib/catalog.svelte';
  import { rackLines } from '../lib/racktext.svelte';
  import { slotColor as panelSlotColor } from '../lib/panel';
  import { HEX } from '../lib/pdfkit';
  import { inkOn } from '../lib/color';
  import { accFace, screensOf, visualOf } from '../lib/faceplate';
  import { sideGeom } from '../lib/rackgeom';
  import {
    BLOCK, CASQ_W, CASQ_EXTRA, FRAME, RACK_NUM, RACK_VIEWS, DESK_VIEWS, SYN_MM, U_MM,
    casqProfile, deskExtent, groupLayout, isDrawing, itemSize, pageSize, rackGeom, rackBodyH, scaleOf, shotKey, usableW, viewsOf, zoomOf,
    type Corner, type Ctx, type DeskData, type RackData, type RackRender, type Sheet, type SheetItem, type SheetPage, type ViewBox,
  } from '../lib/sheet';

  /** `revision` : révision du projet, affichée quand la planche n'a pas de version propre. */
  interface Info { project: string; client: string; venue: string; site: string; date: string; revision?: string; locations: Record<string, { name: string; code: string }> }
  let { sheet, page, ctx, info, index, total, shots = {}, editable = false, selected = null, onpick, onresize, onselect, nameOf }: {
    sheet: Sheet; page: SheetPage; ctx: Ctx; info: Info; index: number; total: number; shots?: Record<string, string>;
    editable?: boolean; selected?: string | null; onpick?: (it: SheetItem, e: PointerEvent) => void;
    /** Poignée de coin saisie : le coin opposé reste en place pendant le redimensionnement. */
    onresize?: (it: SheetItem, e: PointerEvent, corner: Corner) => void;
    /** Sélection au clavier (focus, Entrée ou Espace) ; `nameOf` : nom lu par les lecteurs d'écran. */
    onselect?: (it: SheetItem) => void; nameOf?: (it: SheetItem) => string;
  } = $props();

  const CORNERS: [Corner, string][] = [['nw', 'haut gauche'], ['ne', 'haut droit'], ['sw', 'bas gauche'], ['se', 'bas droit']];
  const F = 'Inter, Helvetica, Arial, sans-serif', MONO = 'JetBrainsMono, Courier, monospace';
  /** Encre et gris foncé des petits textes, ceux des PDF (pdfkit) ; LINE : contour des appareils. */
  const INK = HEX.ink, DARK = HEX.dark, LINE = '#3A4048';
  /** Plus petit corps imprimé : 6 pt ≈ 2,15 mm. Un libellé qui ne tient pas à ce corps n'est pas dessiné. */
  const MIN = 2.15;
  const fsU = (eh: number, max: number) => Math.max(MIN, Math.min(max, eh * 0.42));
  const size = $derived(pageSize(sheet));
  const k = $derived(sheet.scale);
  const maxW = $derived(usableW(sheet));
  // Textes : jamais de points de suspension ni de texte écrasé. Un texte trop long est
  // raccourci au dernier mot entier qui tient dans la largeur disponible.
  const est = (s: string, fs: number) => s.length * fs * 0.56;
  const fit = (s: string, width: number, fs: number) => {
    const max = Math.max(1, Math.floor(width / (fs * 0.56)));
    if (s.length <= max) return s;
    const cut = s.slice(0, max), sp = cut.lastIndexOf(' ');
    return (sp > max * 0.6 ? cut.slice(0, sp) : cut).trimEnd();
  };
  const COLS: Record<string, number> = { full: 1, half: 2, third: 3, quarter: 4 };
  /** Couleur d'un appareil : sa famille de signal (papier), FAMILY_NONE sans famille. */
  const color = (e: any) => familyColor(e.function, { screen: false });
  /** Emplacement de panel : famille de l'emplacement, sinon celle de son groupe (lib/panel). */
  const slotColor = (panel: any, s: any) => panelSlotColor(panel, s);
  const block = $derived({ x: size[0] - FRAME - BLOCK[0], y: size[1] - FRAME - BLOCK[1] });
  const cells = $derived.by(() => {
    const b = sheet.block;
    // Grille de 30 mm : les séparations verticales tombent les unes sous les autres.
    return [
      [{ l: 'Site', v: info.site, w: 60 }, { l: 'Titre', v: b.title, w: 120, big: true }],
      [{ l: 'Projet', v: info.project, w: 60 }, { l: 'Client', v: info.client, w: 60 }, { l: 'Lieu', v: info.venue, w: 60 }],
      [{ l: 'Référence', v: b.reference, w: 30 }, { l: 'Date', v: info.date, w: 30 }, { l: 'Planche', v: `${index + 1} / ${total}`, w: 30 }, { l: 'Version', v: b.version || info.revision || '', w: 30 }, { l: 'Échelle', v: `1:${k}`, w: 30 }, { l: 'Format', v: sheet.paper, w: 30 }],
      [{ l: 'Auteur', v: b.author, w: 60 }, { l: 'Vérificateur', v: b.checker, w: 30 }, { l: 'Normes / notes', v: [b.standards, b.notes].filter(Boolean).join(' · '), w: 90 }],
    ];
  });
  const ROWH = [12, 10, 11, 11];
  /** Échelle effective d'un dessin, quand elle diffère de celle de la feuille. */
  const own = (it: SheetItem) => (zoomOf(it) !== 1 && it.kind !== 'synoptic' ? `1:${+scaleOf(it, k).toFixed(1)}` : '');
  const title = (it: SheetItem) => {
    const name = (n: string | undefined) => it.label?.trim() || n || 'Élément retiré';
    if (it.kind === 'synoptic') { const id = it.container_id ?? ''; return [it.label?.trim() || `Synoptique · ${ctx.racks[id]?.name ?? ctx.desks[id]?.name ?? 'élément retiré'}`, '']; }
    if (it.kind === 'rack') { const r = ctx.racks[it.container_id ?? '']; return [name(r?.name), r ? [`${r.units} U · P ${r.depth_mm} mm`, own(it)].filter(Boolean).join(' · ') : '']; }
    const d = ctx.desks[it.container_id ?? '']; return [name(d?.name), d ? [`${d.length_mm} × ${d.depth_mm} mm${d.casq.length ? ` · ${d.casq.length} casquette${d.casq.length > 1 ? 's' : ''}` : ''}`, own(it)].filter(Boolean).join(' · ') : ''];
  };
  const viewLabel = (it: SheetItem, b: ViewBox) => b.v === 'syn' ? '' : b.v === 'casq' ? (ctx.racks[b.cid ?? '']?.name ?? 'Casquette') : (it.kind === 'rack' ? RACK_VIEWS : DESK_VIEWS)[b.v as never] ?? '';
  /** Hachures à 45° dans un rectangle (segments calculés : rendu identique à l'écran et au PDF). */
  const hatch = (x: number, y: number, w: number, h: number, step: number): [number, number, number, number][] => {
    const out: [number, number, number, number][] = [];
    for (let c = x + y + step / 2; c < x + w + y + h; c += step) {
      const x1 = Math.max(x, c - (y + h)), x2 = Math.min(x + w, c - y);
      if (x2 > x1) out.push([x1, c - x1, x2, c - x2]);
    }
    return out;
  };
  /** Synoptique : contenu du SVG rendu et origine de son cadre. */
  const synParts = (svg: string) => {
    const m = /^<svg[^>]*viewBox="([^"]+)"[^>]*>([\s\S]*)<\/svg>$/.exec(svg);
    if (!m) return null;
    const [x0, y0] = m[1]!.split(/\s+/).map(Number);
    return { x0: x0 ?? 0, y0: y0 ?? 0, inner: m[2]! };
  };
  const pts = (p: [number, number][]) => p.map(([x, y]) => `${x.toFixed(2)},${y.toFixed(2)}`).join(' ');
</script>

<!-- Élévation 19″ (face avant ou arrière) d'une baie ou d'une casquette. -->
{#snippet elevation(x: number, y: number, r: RackData | undefined, face: 'front' | 'rear', kk: number, mode: RackRender)}
  {@const n = r?.units ?? 42}
  {@const g = rackGeom(kk)}
  {@const x0 = x + RACK_NUM}
  {@const w = g.inner + 2 * g.rail}
  {@const h = rackBodyH(n, kk)}
  {@const x1 = x0 + g.rail}
  {@const y1 = y + g.pad}
  {@const step = g.u >= 2.6 ? 1 : g.u >= 1.3 ? 2 : g.u >= 0.52 ? 5 : 10}
  {@const els = (r?.elements ?? []).filter((e) => !e.placement.shelf_id && (face === 'front' ? e.placement.face === 'front' : e.placement.face !== 'front'))}
  <rect x={x0} y={y} width={w} height={h} rx="0.8" fill="#FFFFFF" stroke={INK} stroke-width="0.5" />
  <rect x={x0} y={y} width={g.rail} height={h} fill="#D9DDE2" stroke={INK} stroke-width="0.25" />
  <rect x={x1 + g.inner} y={y} width={g.rail} height={h} fill="#D9DDE2" stroke={INK} stroke-width="0.25" />
  {#if g.u >= 0.8}{#each Array(n + 1) as _, i}<line x1={x1} x2={x1 + g.inner} y1={y1 + i * g.u} y2={y1 + i * g.u} stroke="#E3E6EA" stroke-width="0.12" />{/each}{/if}
  {#each Array(n) as _, i}{#if (n - i) % step === 0 || n - i === 1}<text x={x0 - 1.2} y={y1 + i * g.u + g.u / 2 + 0.7} font-family={F} font-size={MIN} fill={DARK} text-anchor="end">{n - i}</text>{/if}{/each}
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
    {@const schem = mode === 'schematic'}
    {@const cc = color(e)}
    {@const carried = (r?.elements ?? []).filter((x) => x.placement.shelf_id === e.placement.id).map((x) => x.custom?.trim() || x.tag || x.label)}
    {@const rl = rackLines(e, 'both')}
    <!-- Schématique : aplat de la famille de signal ; la catégorie passe en texte, en petit. -->
    {@const lines = acc ? [carried.length ? `${schem ? `${e.label} · ` : ''}${carried.join(', ')}` : schem ? e.label : '', ''] : schem ? [rl[0], [e.category, rl[1]].filter(Boolean).join(' · ')] : rl}
    {@const idFont = !acc && !e.custom?.trim() && !!e.tag && lines[0] === e.tag ? MONO : F}
    {@const t = acc ? accFace(e) : visualOf(e)}
    {@const fs1 = Math.max(MIN, Math.min(2.8, eh * 0.3))}
    {@const fs2 = Math.max(MIN, fs1 * 0.82)}
    {@const two = !!lines[1] && eh >= fs1 + fs2 + 1.2}
    <!-- Façade réelle : la silhouette prend la place que le texte laisse, à droite. -->
    {@const need = !lines[0] ? 0 : two ? Math.max(est(lines[0], fs1) * 1.1, est(lines[1], fs2)) : est(lines[0], fs) * 1.1 + (lines[1] ? est(lines[1], MIN) + 3 : 0)}
    {@const sw = acc ? ew - 4 : Math.min(ew * 0.45, ew - 4.6 - need, Math.max(12, eh * 5))}
    {@const sil = mode === 'real' && !panel && eh >= 1.3 && (acc ? !lines[0] : sw >= 3.5)}
    {@const tw = ew - (acc ? 2.4 : 3.4) - (sil && !acc ? sw + 2 : 0)}
    <rect x={ex + 0.15} y={ey + 0.1} width={ew - 0.3} height={eh - 0.2} fill={schem ? (acc ? '#EEF0F2' : cc) : acc ? '#EEF0F2' : '#F6F7F9'} stroke={schem && !acc ? cc : LINE} stroke-width="0.22" stroke-dasharray={inner ? '1 0.6' : undefined} />
    {#if !acc && !schem}<rect x={ex + 0.15} y={ey + 0.1} width={Math.min(1.4, ew * 0.08)} height={eh - 0.2} fill={color(e)} />{/if}
    {#if sil}{@render facade(t, acc ? ex + 2 : ex + ew - sw - 1.2, ey + eh * 0.2, sw, eh * 0.6)}{/if}
    {#if panel && face === 'front'}
      {@const per = panel.per_u}
      {@const pw = (ew * 0.62) / per}
      {#each panel.slots as s}
        {@const row = Math.floor((s.index - 1) / per)}
        {@const col = (s.index - 1) % per}
        <circle cx={ex + ew * 0.36 + (col + s.width / 2) * pw} cy={ey + (row + 0.5) * (eh / panel.height_u)} r={Math.min(pw * 0.36, (eh / panel.height_u) * 0.3)} fill="#FFFFFF" stroke={slotColor(panel, s)} stroke-width="0.25" />
      {/each}
      {@const pl = rackLines(e, 'label')[0] || e.label}
      {#if eh >= 1.6}<text x={ex + 2} y={ey + eh / 2 + fs * 0.35} font-family={F} font-size={fs} fill={INK}>{fit(pl, ew * 0.33, fs)}</text>{/if}
    {:else if eh >= 1.6 && lines[0]}
      {@const ink = schem && !acc ? inkOn(cc) : acc ? DARK : INK}
      {@const sub = schem && !acc ? inkOn(cc) : DARK}
      {@const tx = ex + (acc ? 1.2 : 2.2)}
      {#if two}
        {@const top = ey + (eh - fs1 - fs2 - 0.5) / 2}
        <text x={tx} y={top + fs1 * 0.82} font-family={idFont} font-size={fs1} fill={ink} font-weight="bold">{fit(lines[0], tw, fs1)}</text>
        <text x={tx} y={top + fs1 + 0.5 + fs2 * 0.8} font-family={F} font-size={fs2} fill={sub}>{fit(lines[1], tw, fs2)}</text>
      {:else}
        {@const both = !!lines[1] && (est(lines[0], fs) * 1.1 + est(lines[1], MIN) + 3 < tw)}
        <text x={tx} y={ey + eh / 2 + fs * 0.35} font-family={idFont} font-size={fs} fill={ink} font-weight={acc ? 'normal' : 'bold'}>{fit(lines[0], tw, fs)}</text>
        {#if both}<text x={tx + tw} y={ey + eh / 2 + MIN * 0.35} font-family={F} font-size={MIN} fill={sub} text-anchor="end">{lines[1]}</text>{/if}
      {/if}
    {/if}
  {/each}
  {#if face === 'rear'}
    <!-- PDU verticaux, sur les montants (vue arrière : gauche et droite comme vus de l'arrière). -->
    {#each r?.vpdus ?? [] as v, i (v.article_id)}
      {@const L = Math.min(h - 2 * g.pad, v.accessory.length_mm / kk)}
      {@const px = v.accessory.side === 'left' ? x0 + g.rail * 0.1 : x1 + g.inner + g.rail * 0.1}
      {@const py = y + (h - L) / 2}
      {@const pw = g.rail * 0.8}
      <rect x={px} y={py} width={pw} height={L} rx="0.4" fill="#2B2F35" stroke={INK} stroke-width="0.25" />
      {#each Array(Math.min(60, v.accessory.outlets)) as _, j}<rect x={px + pw * 0.25} y={py + 3 + ((L - 5) / v.accessory.outlets) * j} width={pw * 0.5} height={Math.max(0.3, ((L - 5) / v.accessory.outlets) * 0.55)} fill="#FFFFFF" />{/each}
    {/each}
  {/if}
{/snippet}

<!-- Vue de côté d'une baie : avant à gauche, profondeur des appareils depuis leurs montants,
     réserve de câblage hachurée et cotée (mm). Noms seulement sans vue avant ni arrière. -->
{#snippet side(x: number, y: number, b: ViewBox, r: RackData | undefined, kk: number, named: boolean)}
  {@const n = r?.units ?? 42}
  {@const g = rackGeom(kk)}
  {@const D = (r?.depth_mm ?? 800) / kk}
  {@const x0 = x + 30 / kk}
  {@const fr = x0 + 70 / kk}
  {@const rr = x0 + D - (r?.c?.rear_rails_mm ?? 70) / kk}
  {@const y1 = y + g.pad}
  <rect x={x0} y={y} width={D} height={b.h} rx="0.8" fill="#FFFFFF" stroke={INK} stroke-width="0.5" />
  <line x1={fr} x2={fr} y1={y} y2={y + b.h} stroke={INK} stroke-width="0.35" />
  <line x1={rr} x2={rr} y1={y} y2={y + b.h} stroke={INK} stroke-width="0.35" />
  {#each r?.vpdus ?? [] as v (v.article_id)}
    {@const L = Math.min(b.h - 2 * g.pad, v.accessory.length_mm / kk)}
    <rect x={rr + 4 / kk} y={y + (b.h - L) / 2} width={45 / kk} height={L} rx="0.3" fill="#2B2F35" stroke={INK} stroke-width="0.25" />
  {/each}
  <!-- Même géométrie que la vue de côté de l'écran (lib/rackgeom) : corps depuis les rails, réserve derrière. -->
  {#each sideGeom((r?.elements ?? []).filter((e) => !e.placement.shelf_id), r?.depth_mm ?? 800, r?.c?.rear_rails_mm) as sb (sb.e.placement.id)}
    {@const e = sb.e}
    {@const eh = e.height_u * g.u}
    {@const ey = y1 + (n - (e.placement.u_start - 1) - e.height_u) * g.u}
    {@const dmm = sb.d}
    {@const ed = sb.w / kk}
    {@const front = sb.front}
    {@const ex = x0 + sb.x / kk}
    {@const fs = fsU(eh, 2.4)}
    {@const resmm = sb.res}
    {@const rw = sb.rw / kk}
    {@const rx = x0 + sb.rx / kk}
    {#if rw > 0.3}
      <rect x={rx} y={ey + 0.1} width={rw} height={eh - 0.2} fill="#FFFFFF" stroke="#8A919B" stroke-width="0.15" stroke-dasharray="0.8 0.5" />
      {#each hatch(rx, ey + 0.1, rw, eh - 0.2, 1.2) as [a, bb, c, d]}<line x1={a} y1={bb} x2={c} y2={d} stroke="#8A919B" stroke-width="0.12" />{/each}
    {/if}
    <rect x={ex} y={ey + 0.1} width={Math.max(0.4, ed)} height={eh - 0.2} fill={e.nature === 'accessory' ? '#EEF0F2' : '#F6F7F9'} stroke={LINE} stroke-width="0.22" />
    <rect x={front ? ex - 0.6 : ex + ed} y={ey + 0.1} width="0.6" height={eh - 0.2} fill={color(e)} />
    {#if eh >= 1.6 && e.nature !== 'accessory'}
      {@const cote = `${Math.round(dmm)}`}
      {@const cw = est(cote, MIN) + 1.6}
      {@const label = named ? rackLines(e, 'both')[0] : ''}
      {@const lw = ed - cw - 2.4}
      {#if label && lw > 6}<text x={ex + (front ? 1.2 : cw + 1.2)} y={ey + eh / 2 + fs * 0.35} font-family={F} font-size={fs} fill={INK}>{fit(label, lw, fs)}</text>{/if}
      {#if ed > cw}<text x={front ? ex + ed - 0.8 : ex + 0.8} y={ey + eh / 2 + MIN * 0.35} font-family={MONO} font-size={MIN} fill={DARK} text-anchor={front ? 'end' : 'start'}>{cote}</text>{/if}
      {#if rw > est(String(Math.round(resmm)), MIN) + 1}<text x={rx + rw / 2} y={ey + eh / 2 + MIN * 0.35} font-family={MONO} font-size={MIN} fill={DARK} text-anchor="middle">{Math.round(resmm)}</text>{/if}
    {/if}
  {/each}
  <text x={x0 + 1} y={y + b.h + 3} font-family={F} font-size={MIN} fill={DARK}>AVANT</text>
  {#if D > 34}<text x={x0 + D / 2} y={y + b.h + 3} font-family={MONO} font-size={MIN} fill={DARK} text-anchor="middle">P {r?.depth_mm ?? 800}</text>{/if}
  <text x={x0 + D - 1} y={y + b.h + 3} font-family={F} font-size={MIN} fill={DARK} text-anchor="end">ARRIÈRE</text>
{/snippet}

<!-- Façade simplifiée (rendu réel) : silhouette typique de la famille d'appareils, sans image. -->
{#snippet facade(t: string, x: number, y: number, w: number, h: number)}
  {@const scr = screensOf(t)}
  {@const SIL = '#8A919B'}
  {#if scr}
    {@const gap = Math.min(1, w * 0.04)}
    {@const sw = (w - gap * (scr - 1)) / scr}
    {#each Array(scr) as _, i}<rect x={x + i * (sw + gap)} y={y} width={sw} height={h} rx="0.2" fill="#4A5059" stroke={INK} stroke-width="0.12" />{/each}
  {:else if t === 'switch' || t === 'pdu' || t === 'rjp'}
    {@const rows = h > 3.2 ? 2 : 1}
    {@const cell = Math.min(h / rows, 2.2)}
    {@const cols = Math.max(1, Math.min(24, Math.floor(w / (cell * 1.25))))}
    {#each Array(rows * cols) as _, i}<rect x={x + w - (cols - (i % cols)) * cell * 1.25} y={y + (h - rows * cell) / 2 + Math.floor(i / cols) * cell + cell * 0.12} width={cell} height={cell * 0.76} fill="#FFFFFF" stroke={SIL} stroke-width="0.12" />{/each}
  {:else if t === 'server'}
    {@const n = Math.max(2, Math.min(8, Math.floor(w / Math.max(2.4, h * 1.2))))}
    {@const bw = w / n}
    {#each Array(n) as _, i}<rect x={x + i * bw + 0.2} y={y} width={bw - 0.4} height={h} fill="#FFFFFF" stroke={SIL} stroke-width="0.12" />{/each}
  {:else if t === 'cards'}
    {@const n = Math.max(3, Math.min(20, Math.floor(w / 1.2)))}
    {#each Array(n) as _, i}<line x1={x + ((i + 0.5) * w) / n} x2={x + ((i + 0.5) * w) / n} y1={y} y2={y + h} stroke={SIL} stroke-width="0.25" />{/each}
  {:else if t === 'bnc' || t === 'xlr' || t === 'stage' || t === 'vent'}
    {@const d = Math.min(h, 2)}
    {@const n = Math.max(2, Math.min(24, Math.floor(w / (d * 1.5))))}
    {#each Array(n) as _, i}<circle cx={x + (i + 0.5) * (w / n)} cy={y + h / 2} r={d * 0.36} fill="#FFFFFF" stroke={SIL} stroke-width="0.12" />{/each}
  {:else if t === 'brush'}
    <rect x={x} y={y + h * 0.25} width={w} height={h * 0.5} rx={h * 0.25} fill="#FFFFFF" stroke={SIL} stroke-width="0.15" />
    <line x1={x + 1} x2={x + w - 1} y1={y + h / 2} y2={y + h / 2} stroke={SIL} stroke-width="0.12" />
  {:else if t === 'drawer'}
    <rect x={x + w * 0.4} y={y + h * 0.35} width={w * 0.2} height={h * 0.3} rx="0.2" fill="#FFFFFF" stroke={SIL} stroke-width="0.15" />
  {:else if t === 'shelf'}
    <line x1={x} x2={x + w} y1={y + h} y2={y + h} stroke={SIL} stroke-width="0.25" />
  {:else if t === 'lacing'}
    <line x1={x} x2={x + w} y1={y + h / 2} y2={y + h / 2} stroke={SIL} stroke-width="0.3" />
  {:else if t === 'blank'}
    <!-- Plaque pleine : rien à dessiner. -->
  {:else if t === 'gen' || t === 'dist'}
    {@const n = Math.max(2, Math.min(5, Math.floor(h / 0.8)))}
    {#each Array(n) as _, i}<line x1={x + w * 0.35} x2={x + w * 0.82} y1={y + ((i + 0.5) * h) / n} y2={y + ((i + 0.5) * h) / n} stroke={SIL} stroke-width="0.15" />{/each}
    <circle cx={x + w - Math.min(h, 2) * 0.4} cy={y + h / 2} r={Math.min(h, 2) * 0.3} fill="#FFFFFF" stroke={SIL} stroke-width="0.15" />
  {:else}
    <!-- Afficheur et touches : grilles, mélangeurs, enregistreurs, synchro, onduleurs… -->
    {@const lw = Math.min(w * 0.38, h * 3.2)}
    {@const rows = h > 3.2 ? 2 : 1}
    {@const cell = Math.min(h / rows, 1.8)}
    {@const cols = Math.max(1, Math.min(12, Math.floor((w - lw - 1) / (cell * 1.3))))}
    <rect x={x} y={y + h * 0.1} width={lw} height={h * 0.8} rx="0.2" fill="#4A5059" />
    {#each Array(rows * cols) as _, i}<rect x={x + w - (cols - (i % cols)) * cell * 1.3} y={y + (h - rows * cell) / 2 + Math.floor(i / cols) * cell + cell * 0.15} width={cell} height={cell * 0.7} rx="0.15" fill="#FFFFFF" stroke={SIL} stroke-width="0.12" />{/each}
  {/if}
{/snippet}

<!-- Mobilier vu de face (côté opérateur), de côté (opérateur à gauche) ou de dessus. -->
{#snippet desk(x: number, y: number, b: ViewBox, d: DeskData | undefined, kk: number)}
  {#if d}
    {@const e = deskExtent(d)}
    {@const T = 32 / kk}
    {@const J = 45 / kk}
    {@const s = 1 / kk}
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
          {@const lab = rackLines(e, 'label')[0]}
          {#if eh >= 1.6 && lab}<text x={ex + 1.3} y={ey + eh / 2 + fs * 0.35} font-family={F} font-size={fs} fill={INK}>{fit(lab, ew - 2, fs)}</text>{/if}
        {/each}
        {#if cw > 14 && !(ctx.racks[c.id]?.elements ?? []).some((x) => x.placement.face === 'front')}<text x={cx + cw / 2} y={top - Hc / 2 + 1} font-family={F} font-size={Math.max(MIN, Math.min(2.4, cw / 12))} fill={DARK} text-anchor="middle">{fit(c.name, cw - 3, Math.max(MIN, Math.min(2.4, cw / 12)))}</text>{/if}
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
      <text x={x + 1} y={fy + 3} font-family={F} font-size={MIN} fill={DARK}>OPÉRATEUR</text>
    {:else if b.v === 'top'}
      <rect x={x} y={y} width={e.L * s} height={d.depth_mm * s} fill="#FFFFFF" stroke={INK} stroke-width="0.5" />
      <rect x={x} y={y + (d.depth_mm - 110) * s} width={e.L * s} height={110 * s} fill="#E7E9EC" stroke={LINE} stroke-width="0.2" />
      {#each d.casq as c (c.id)}
        {@const p = casqProfile(d, c)}
        {@const z0 = Math.min(...p.map((q) => q[0]))}
        {@const z1 = Math.max(...p.map((q) => q[0]))}
        {@const cx = x + Math.max(0, Math.min(d.length_mm - CASQ_W, c.offset_mm)) * s}
        <rect x={cx} y={y + (d.depth_mm - z1) * s} width={CASQ_W * s} height={(z1 - z0) * s} fill="#F6F7F9" stroke={INK} stroke-width="0.35" />
        {#if CASQ_W * s > 14}<text x={cx + (CASQ_W * s) / 2} y={y + (d.depth_mm - (z0 + z1) / 2) * s + 1} font-family={F} font-size="2.2" fill={INK} text-anchor="middle">{fit(c.name, CASQ_W * s - 3, 2.2)}</text>{/if}
      {/each}
      <text x={x + 1} y={y + d.depth_mm * s + 3} font-family={F} font-size={MIN} fill={DARK}>CÔTÉ OPÉRATEUR</text>
    {/if}
  {/if}
{/snippet}

{#snippet group(it: SheetItem)}
  {@const L = groupLayout(it, k, ctx, maxW)}
  {@const kk = scaleOf(it, k)}
  {@const [name, sub] = title(it)}
  {@const vs = viewsOf(it)}
  {@const named = it.kind === 'rack' && !vs.includes('front') && !vs.includes('rear')}
  {#if !it.hideLabel}
    <text x={it.x} y={it.y + 4} font-family={F} font-size="3.6" font-weight="bold" fill={INK}>{fit(name, Math.max(30, L.w - (sub ? 40 : 0)), 3.6)}</text>
    {#if sub}<text x={it.x + L.w} y={it.y + 4} font-family={F} font-size="2.4" fill={DARK} text-anchor="end">{L.w > 60 ? sub : ''}</text>{/if}
  {/if}
  {#each L.views as b, i (i)}
    {@const bx = it.x + b.x}
    {@const by = it.y + b.y}
    {@const lbl = viewLabel(it, b)}
    {#if lbl}<text x={bx + (b.v === 'front' || b.v === 'rear' || b.v === 'casq' ? RACK_NUM : 0)} y={by - 1.4} font-family={F} font-size="2.3" fill={DARK} letter-spacing=".2">{fit(lbl.toUpperCase(), b.w, 2.3)}</text>{/if}
    {#if b.v === 'syn'}
      {@const r = ctx.syn?.[it.container_id ?? '']}
      {@const sp = r ? synParts(r.svg) : null}
      {#if r && sp}
        <g clip-path="url(#clip-{page.id})"><g transform="translate({bx} {by}) scale({SYN_MM * zoomOf(it)}) translate({-sp.x0} {-sp.y0})">{@html sp.inner}</g></g>
        <rect x={bx} y={by} width={b.w} height={b.h} fill="none" stroke="#C9CDD3" stroke-width="0.25" />
      {:else}
        <rect x={bx} y={by} width={b.w} height={b.h} fill="#F6F7F9" stroke="#C9CDD3" stroke-width="0.3" stroke-dasharray="2 1" />
        <text x={bx + b.w / 2} y={by + b.h / 2 + 1} font-family={F} font-size="2.6" fill={DARK} text-anchor="middle">{r === null ? 'Synoptique vide' : 'Synoptique…'}</text>
      {/if}
    {:else if b.v === '3d' || (it.kind === 'rack' && it.render === '3d' && b.v !== 'none')}
      {@const src = shots[shotKey(it.id, b.v)]}
      {@const ox = b.v === 'front' || b.v === 'rear' ? RACK_NUM : 0}
      {#if src}<image href={src} x={bx + ox} y={by} width={b.w - ox} height={b.h} preserveAspectRatio="xMidYMid meet" />
      {:else}<rect x={bx + ox} y={by} width={b.w - ox} height={b.h} fill="#F6F7F9" stroke="#C9CDD3" stroke-width="0.3" stroke-dasharray="2 1" /><text x={bx + ox + (b.w - ox) / 2} y={by + b.h / 2} font-family={F} font-size="2.6" fill={DARK} text-anchor="middle">Vue 3D…</text>{/if}
    {:else if it.kind === 'rack'}
      {#if b.v === 'side'}{@render side(bx, by, b, ctx.racks[it.container_id ?? ''], kk, named)}
      {:else}{@render elevation(bx, by, ctx.racks[it.container_id ?? ''], b.v === 'rear' ? 'rear' : 'front', kk, it.render ?? 'real')}{/if}
    {:else if b.v === 'casq'}{@render elevation(bx, by, ctx.racks[b.cid ?? ''], 'front', kk, 'real')}
    {:else}{@render desk(bx, by, b, ctx.desks[it.container_id ?? ''], kk)}{/if}
  {/each}
{/snippet}

<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 {size[0]} {size[1]}" width="{size[0]}mm" height="{size[1]}mm" class="sheet-page">
  <rect x="0" y="0" width={size[0]} height={size[1]} fill="#FFFFFF" />
  <!-- Un synoptique plus grand que la feuille est coupé au cadre. -->
  <defs><clipPath id="clip-{page.id}"><rect x={FRAME} y={FRAME} width={size[0] - 2 * FRAME} height={size[1] - 2 * FRAME} /></clipPath></defs>
  {#each page.items as it (it.id)}
    {@const [w, h] = itemSize(it, k, ctx, maxW)}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <g class="it" class:edit={editable} onpointerdown={(e) => onpick?.(it, e)} role={editable ? 'button' : undefined} tabindex={editable ? 0 : undefined}
      aria-label={editable ? (nameOf?.(it) ?? title(it)[0] ?? 'Dessin') : undefined} aria-pressed={editable ? selected === it.id : undefined}
      onfocus={() => { if (editable) onselect?.(it); }}
      onkeydown={(e) => { if (editable && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); onselect?.(it); } }}>
      {#if editable}<rect x={it.x - 1.5} y={it.y - 1.5} width={w + 3} height={h + 3} rx="1" fill={selected === it.id ? 'rgba(79,107,255,.05)' : 'transparent'} stroke={selected === it.id ? '#4F6BFF' : 'transparent'} stroke-width="0.4" />{/if}
      {#if isDrawing(it)}{@render group(it)}
      {:else if it.kind === 'location'}
        {@const l = info.locations[it.location_id ?? '']}
        <text x={it.x} y={it.y + 6} font-family={F} font-size="5.2" font-weight="bold" fill={INK}>{fit(l?.name ?? it.label ?? 'Emplacement', w - 20, 5.2)}</text>
        {#if l?.code}<text x={it.x + w} y={it.y + 6} font-family={MONO} font-size="3" fill={DARK} text-anchor="end">{l.code}</text>{/if}
        <line x1={it.x} x2={it.x + w} y1={it.y + 8.6} y2={it.y + 8.6} stroke={INK} stroke-width="0.5" />
      {:else}
        <rect x={it.x} y={it.y} width={w} height={h} fill="#FAFBFC" stroke="#8A919B" stroke-width="0.35" stroke-dasharray="2.5 1.5" />
        <text x={it.x + w / 2} y={it.y + h / 2} font-family={F} font-size="3.4" fill={DARK} text-anchor="middle">{fit(it.label || 'Emplacement réservé', w - 4, 3.4)}</text>
        <text x={it.x + w / 2} y={it.y + h / 2 + 4.4} font-family={F} font-size="2.4" fill={DARK} text-anchor="middle">RÉSERVÉ</text>
      {/if}
    </g>
  {/each}
  <rect x={FRAME} y={FRAME} width={size[0] - 2 * FRAME} height={size[1] - 2 * FRAME} fill="none" stroke={INK} stroke-width="0.35" />
  {#if editable && onresize}
    {@const it = page.items.find((i) => i.id === selected && isDrawing(i))}
    {#if it}
      {@const [w, h] = itemSize(it, k, ctx, maxW)}
      {@const hs = Math.max(2.6, size[0] / 110)}
      <!-- Une poignée à chaque coin : proportions gardées, le coin opposé ne bouge pas. -->
      {#each CORNERS as [c, l] (c)}
        {@const cx = c[1] === 'w' ? it.x - 1.5 : it.x + w + 1.5}
        {@const cy = c[0] === 'n' ? it.y - 1.5 : it.y + h + 1.5}
        <rect class="rz rz-{c}" x={cx - hs / 2} y={cy - hs / 2} width={hs} height={hs} rx={hs * 0.2} fill="#FFFFFF" stroke="#4F6BFF" stroke-width={hs * 0.14} role="button" aria-label="Redimensionner (coin {l})" tabindex="-1" onpointerdown={(e) => { e.stopPropagation(); onresize(it, e, c); }}><title>Glisser pour redimensionner (proportions gardées)</title></rect>
      {/each}
    {/if}
  {/if}
  {#if sheet.titleBlock}
    <g>
      <rect x={block.x} y={block.y} width={BLOCK[0]} height={BLOCK[1]} fill="#FFFFFF" stroke={INK} stroke-width="0.5" />
      {#each cells as row, ri}
        {@const ry = block.y + ROWH.slice(0, ri).reduce((a, b) => a + b, 0)}
        {#if ri > 0}<line x1={block.x} x2={block.x + BLOCK[0]} y1={ry} y2={ry} stroke={INK} stroke-width="0.25" />{/if}
        {#each row as c, ci}
          {@const cx = block.x + row.slice(0, ci).reduce((a, b) => a + b.w, 0)}
          {#if ci > 0}<line x1={cx} x2={cx} y1={ry} y2={ry + ROWH[ri]!} stroke={INK} stroke-width="0.25" />{/if}
          <text x={cx + 1.4} y={ry + 3.1} font-family={F} font-size="2.4" fill={DARK} letter-spacing=".1">{c.l.toUpperCase()}</text>
          <text x={cx + 1.4} y={ry + ROWH[ri]! - 2.2} font-family={F} font-size={'big' in c ? 4.4 : 3} font-weight={'big' in c || ri === 1 && ci === 0 ? 'bold' : 'normal'} fill={INK}>{fit(c.v || '—', c.w - 3, 'big' in c ? 4.4 : 3)}</text>
        {/each}
      {/each}
    </g>
  {/if}
</svg>

<style>
  .sheet-page { display: block; width: 100%; height: auto; background: #fff; }
  .it.edit { cursor: grab; outline: none; }
  .it.edit:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .it.edit:active { cursor: grabbing; }
  .rz-nw, .rz-se { cursor: nwse-resize; }
  .rz-ne, .rz-sw { cursor: nesw-resize; }
</style>
