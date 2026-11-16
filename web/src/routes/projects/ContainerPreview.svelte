<script lang="ts">
  // Aperçu vivant de la fenêtre « Nouvel élément » : silhouette à l'échelle (baie ou valise vue de
  // face avec sa profondeur, U tracés, roulettes), grille d'un mur d'écrans ou pictogramme du type,
  // puis le résumé texte. Les façades restent sombres dans les deux thèmes, comme la vue Racks.
  import KindIll from '../../components/KindIll.svelte';
  import { type ContainerData } from '../../lib/projects';
  let { c, name, place, summary, chips = [] }: { c: ContainerData; name: string; place: string; summary: string; chips?: string[] } = $props();
  const uid = $props.id();
  const U = 44.45, PANEL = 482.6;
  // Scène : 240 × 270, une marge pour la cote de hauteur à gauche et un sol en bas.
  const VW = 240, VH = 270, PAD = 10, COTE = 26, FLOOR = VH - 18, PERSON = 1750;
  // Silhouette humaine (mm, depuis le sol, x centré) : épaules, bras, jambes.
  const BODY: [number, number][] = [[-55, 1530], [-190, 1475], [-235, 1430], [-255, 910], [-190, 905], [-172, 1240], [-165, 860], [-150, 0], [-35, 0], [-12, 800], [12, 800], [35, 0], [150, 0], [165, 860], [172, 1240], [190, 905], [255, 910], [235, 1430], [190, 1475], [55, 1530]];
  const isCase = $derived(/case/.test(c.kind));
  const fmtM = (mm: number) => `${(mm / 1000).toLocaleString('fr-FR', { minimumFractionDigits: 2, maximumFractionDigits: 2 })} m`;

  const g = $derived.by(() => {
    if (!c.rackable) return null;
    const units = Math.max(1, Math.min(60, Math.round(c.height_u ?? (c.kind === 'rack' ? 42 : 12))));
    const W = c.width_mm ?? (isCase ? 560 : 600);
    const D = Math.max(150, Math.min(1400, c.depth_mm ?? (isCase ? 600 : 800)));
    const top = c.kind === 'rack' ? 110 : isCase ? 60 : 30, base = c.kind === 'rack' ? 50 : isCase ? 50 : 20;
    const wheel = c.wheels ? (isCase ? 130 : 100) : 0;
    const H = units * U + top + base;
    // Perspective cavalière : la profondeur part vers le haut à droite.
    const kx = 0.3, ky = 0.17;
    const room = VW - COTE - 2 * PAD - 54; // place gardée pour le repère humain
    const s = Math.min((FLOOR - PAD - 6) / PERSON, room / (W + D * kx), (FLOOR - PAD) / (H + wheel + D * ky));
    const w = W * s, h = H * s, dx = D * kx * s, dy = D * ky * s;
    const person = PERSON * s <= FLOOR - PAD;
    const groupW = w + dx + (person ? 14 + 520 * s : 0);
    const x = COTE + PAD + Math.max(0, (VW - COTE - 2 * PAD - groupW) / 2);
    const y = FLOOR - wheel * s - h;
    const bx = x + ((W - PANEL) / 2) * s, bw = PANEL * s;
    const by = y + top * s, uh = U * s;
    const rail = Math.max(2, 16 * s);
    const lines = Array.from({ length: units - 1 }, (_, i) => by + (i + 1) * uh);
    const r = Math.max(2.2, 42 * s);
    return { units, W, D, H: H + wheel, s, x, y, w, h, dx, dy, bx, bw, by, bh: units * uh, uh, rail, lines, r, wheel: wheel * s, person, px: x + w + dx + 14 };
  });
  const wall = $derived(c.kind === 'monitor_wall' && c.details.wall ? { rows: Math.max(1, Math.min(12, c.details.wall.rows || 1)), cols: Math.max(1, Math.min(16, c.details.wall.cols || 1)) } : null);
</script>

<div class="pv">
  <div class="pv-stage" aria-hidden="true">
    {#if g}
      {@const caseRx = c.kind === 'hard_case' ? Math.max(3, 40 * g.s) : Math.max(1.5, 14 * g.s)}
      <svg viewBox="0 0 {VW} {VH}" width={VW} height={VH} class="pv-svg">
        <defs>
          <linearGradient id="{uid}-front" x1="0" x2="1"><stop offset="0" stop-color="#2A2E34" /><stop offset=".08" stop-color="#3A3F46" /><stop offset=".5" stop-color="#30353B" /><stop offset=".92" stop-color="#3A3F46" /><stop offset="1" stop-color="#2A2E34" /></linearGradient>
          <linearGradient id="{uid}-rail" x1="0" x2="1"><stop offset="0" stop-color="#5E656E" /><stop offset=".42" stop-color="#C3C8CE" /><stop offset=".7" stop-color="#959CA4" /><stop offset="1" stop-color="#4E555D" /></linearGradient>
          <radialGradient id="{uid}-shadow"><stop offset="0" stop-color="#000" stop-opacity=".35" /><stop offset="1" stop-color="#000" stop-opacity="0" /></radialGradient>
        </defs>
        <line class="floor" x1={PAD} x2={VW - PAD} y1={FLOOR + .5} y2={FLOOR + .5} />
        <ellipse cx={g.x + (g.w + g.dx) / 2} cy={FLOOR} rx={(g.w + g.dx) * .62} ry="5" fill="url(#{uid}-shadow)" />
        <!-- Profondeur : dessus et flanc. -->
        <path d="M{g.x} {g.y}l{g.dx} {-g.dy}h{g.w}l{-g.dx} {g.dy}z" fill={isCase ? 'var(--ill-alu)' : '#4A4F57'} />
        <path d="M{g.x + g.w} {g.y}l{g.dx} {-g.dy}v{g.h}l{-g.dx} {g.dy}z" fill={isCase ? '#262A30' : '#1A1D22'} />
        {#if isCase}
          <path d="M{g.x + g.w} {g.y}l{g.dx} {-g.dy}v{g.h}l{-g.dx} {g.dy}z" fill="none" stroke="var(--ill-alu)" stroke-width={Math.max(1.2, 18 * g.s)} />
          <rect x={g.x} y={g.y} width={g.w} height={g.h} rx={caseRx} fill="#2C3036" stroke="var(--ill-alu)" stroke-width={Math.max(2, 26 * g.s)} />
          {#each [[g.x, g.y], [g.x + g.w, g.y], [g.x, g.y + g.h], [g.x + g.w, g.y + g.h]] as [cx, cy]}<rect x={cx - 3} y={cy - 3} width="6" height="6" rx="1.2" fill="var(--ill-corner)" />{/each}
          {#if c.kind === 'hard_case'}<path class="handle" d="M{g.x + g.w / 2 - 9} {g.y - .5}v-4h18v4" />{/if}
        {:else}
          <rect x={g.x} y={g.y} width={g.w} height={g.h} rx={Math.max(1.5, 12 * g.s)} fill="url(#{uid}-front)" />
          <rect x={g.x + g.w * .18} y={g.y + Math.max(1.5, 30 * g.s)} width={g.w * .64} height={Math.max(1.5, 40 * g.s)} rx=".8" fill="#0B0C0E" opacity=".85" />
        {/if}
        <!-- Baie 19″ : fond, U, rails percés. -->
        <rect x={g.bx} y={g.by} width={g.bw} height={g.bh} fill="#08090B" />
        {#each g.lines as ly, i}<line x1={g.bx} x2={g.bx + g.bw} y1={ly} y2={ly} stroke={(i + 1) % 6 === 0 ? '#4A5058' : '#262B32'} stroke-width=".7" />{/each}
        <rect x={g.bx - g.rail} y={g.by} width={g.rail} height={g.bh} fill="url(#{uid}-rail)" />
        <rect x={g.bx + g.bw} y={g.by} width={g.rail} height={g.bh} fill="url(#{uid}-rail)" />
        {#if !isCase}<rect x={g.x + g.w * .2} y={g.y + g.h - Math.max(1.5, 34 * g.s)} width={g.w * .6} height={Math.max(1.5, 24 * g.s)} rx=".8" fill="#0B0C0E" opacity=".7" />{/if}
        {#if c.wheels}
          {#each [g.x + g.r + 2, g.x + g.w - g.r - 2] as wx}<circle cx={wx} cy={FLOOR - g.r} r={g.r} fill="#16181C" stroke="#4A4F57" stroke-width="1" /><line x1={wx} x2={wx} y1={g.y + g.h} y2={FLOOR - g.r * 2} stroke="#4A4F57" stroke-width="1.6" />{/each}
        {/if}
        <!-- Cote de hauteur hors tout. -->
        <g class="cote">
          <path d="M{g.x - 9} {g.y}v{FLOOR - g.y}M{g.x - 12} {g.y}h6M{g.x - 12} {FLOOR}h6" />
          <text x={g.x - 13} y={(g.y + FLOOR) / 2} transform="rotate(-90 {g.x - 13} {(g.y + FLOOR) / 2})" text-anchor="middle">{fmtM(g.H)}</text>
        </g>
        {#if g.person}
          <!-- Repère humain de 1,75 m à la même échelle. -->
          {@const hx = g.px + 250 * g.s}
          <g class="person"><title>Repère : 1,75 m</title>
            <circle cx={hx} cy={FLOOR - (PERSON - 112) * g.s} r={108 * g.s} />
            <path d={BODY.map(([bx, by], i) => `${i ? 'L' : 'M'}${(hx + bx * g.s).toFixed(1)} ${(FLOOR - by * g.s).toFixed(1)}`).join('') + 'Z'} />
          </g>
        {/if}
      </svg>
    {:else if wall}
      <div class="pv-wall" style="--r:{wall.rows};--c:{wall.cols}">{#each Array(wall.rows * wall.cols) as _}<i></i>{/each}</div>
    {:else}
      <div class="pv-kind"><KindIll kind={c.kind} size={168} /></div>
    {/if}
  </div>
  <div class="pv-txt" aria-live="polite">
    <b class="pv-name">{name.trim() || 'Sans nom'}</b>
    <span class="pv-sum">{summary}</span>
    {#if place}<span class="muted small">{place}</span>{/if}
    {#if chips.length}<span class="pv-chips">{#each chips as t}<span class="chip">{t}</span>{/each}</span>{/if}
  </div>
</div>

<style>
  .pv { display: grid; gap: 0; border-radius: var(--r-md); background: var(--card-bg); box-shadow: var(--card-sh); overflow: hidden; }
  .pv-stage { display: grid; place-items: center; min-height: 230px; padding: 8px; background: radial-gradient(70% 55% at 50% 8%, var(--stage-a) 0%, transparent 100%), linear-gradient(180deg, var(--stage-a), var(--stage-b)); }
  .pv-svg { display: block; max-width: 100%; height: auto; overflow: visible; }
  .floor { stroke: color-mix(in srgb, var(--fg-3) 45%, transparent); stroke-width: 1; }
  .cote path { fill: none; stroke: var(--fg-3); stroke-width: 1; }
  .cote text { font: 600 11.5px var(--mono); fill: var(--fg-2); font-variant-numeric: tabular-nums; }
  .person :global(*) { fill: color-mix(in srgb, var(--fg-3) 12%, transparent); stroke: color-mix(in srgb, var(--fg-3) 70%, transparent); stroke-width: 1; stroke-linejoin: round; }
  .handle { fill: none; stroke: var(--ill-alu); stroke-width: 2; stroke-linejoin: round; }
  .pv-kind { display: grid; place-items: center; padding: 18px 0; }
  .pv-kind :global(.ki) { color: var(--fg-2); }
  .pv-wall { display: grid; grid-template-columns: repeat(var(--c), 1fr); gap: 3px; width: min(100%, 220px); padding: 7px; border-radius: 8px; background: #15181C; box-shadow: 0 10px 20px -10px rgba(0, 0, 0, .5); }
  .pv-wall i { aspect-ratio: 16 / 9; border-radius: 2px; background: linear-gradient(180deg, #2C3138, #1E2227); box-shadow: inset 0 0 0 1px #3A4048; }
  .pv-txt { display: grid; gap: 3px; padding: 12px 14px 14px; border-top: 1px solid var(--line); min-width: 0; }
  .pv-name { font: 650 var(--fs-lg)/1.25 var(--display); overflow-wrap: anywhere; }
  .pv-sum { font: 600 var(--fs-sm)/1.4 var(--font); color: var(--fg-2); font-variant-numeric: tabular-nums; }
  .pv-chips { display: flex; flex-wrap: wrap; gap: 5px; margin-top: 5px; }
  /* Mobile : aperçu en bandeau compact (silhouette à gauche, résumé à droite), au-dessus des champs. */
  @media (max-width: 860px) {
    .pv { grid-template-columns: 128px minmax(0, 1fr); }
    .pv-stage { min-height: 0; padding: 4px; }
    .pv-svg { width: 100%; }
    .pv-kind { padding: 10px 6px; }
    .pv-kind :global(.ki) { width: 100%; height: auto; }
    .pv-wall { padding: 4px; gap: 2px; }
    .pv-txt { border-top: 0; border-left: 1px solid var(--line); align-content: center; }
  }
</style>
