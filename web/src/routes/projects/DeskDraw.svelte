<script lang="ts">
  // Mobilier vu de côté (opérateur à gauche) ou de dessus (opérateur en bas), aux cotes réelles,
  // ajusté à la zone de dessin. Mêmes géométries que les planches (casqProfile) ; casquettes
  // et postes cliquables, et cibles du glisser-déposer depuis « À placer ».
  import { nf } from '../../lib/format';
  import { casqProfile, deskExtent, CASQ_W, type DeskData } from '../../lib/sheet';
  import type { Container, DeskPosition } from '../../lib/projects';

  let { desk, casq, posts, mode, sel, selP, w, h, armed, nameOf, gearOf, onhit, onequip, ondropon, onover }: {
    desk: Container; casq: Container[]; posts: DeskPosition[]; mode: 'side' | 'top';
    sel: string | null; selP: string | null; w: number; h: number; armed: boolean;
    nameOf: (c: Container) => string; gearOf: (p: DeskPosition) => number;
    onhit: (kind: 'c' | 'p', id: string) => void; onequip: (id: string) => void;
    ondropon: (kind: 'c' | 'p', id: string) => void;
    /** Cible survolée (glisser, ou élément choisi), `null` à la sortie ; renvoie si le dépôt y est possible. */
    onover?: (kind: 'c' | 'p', id: string | null) => boolean;
  } = $props();

  const dd = $derived<DeskData>({
    name: desk.name, length_mm: desk.length_mm ?? 2400, depth_mm: desk.width_mm ?? 900, height_mm: desk.outer_height_mm ?? 750,
    casq: casq.map((c) => ({ id: c.id, name: c.name, offset_mm: c.offset_mm ?? 0, units: c.height_u ?? 6, depth_mm: c.depth_mm ?? 350, tilt_deg: c.tilt_deg ?? 0 })), c: desk,
  });
  const ext = $derived(deskExtent(dd));
  // Cadre du dessin (mm réels) : marges pour les cotes et la mention de l'opérateur.
  // Marges en pixels écran (cotes, mention de l'opérateur), converties en mm du dessin.
  const base = $derived(mode === 'side' ? { w: ext.D, h: ext.H } : { w: ext.L, h: ext.D });
  const k = $derived(Math.max(0.02, Math.min((w - 32 - 110) / base.w, (h - 32 - 110) / base.h)));
  /** Texte de 12 px à l'écran, en mm du dessin ; cote à 2 lignes sous le dessin. */
  const fs = $derived(12 / k);
  const dy = $derived(2 * fs);
  const vb = $derived(mode === 'side'
    ? { x: -60 / k, y: -(ext.H + 30 / k), w: ext.D + 90 / k, h: ext.H + 110 / k }
    : { x: -50 / k, y: -30 / k, w: ext.L + 80 / k, h: ext.D + 110 / k });
  const pts = (p: [number, number][]) => p.map(([z, y]) => `${z},${-y}`).join(' ');
  // Casquette sélectionnée dessinée en dernier (au-dessus des autres en vue de côté).
  const order = $derived([...dd.casq].sort((a, b) => (a.id === sel ? 1 : 0) - (b.id === sel ? 1 : 0)));
  const PH = 230; // profondeur de la zone d'un poste, depuis le bord opérateur (mm)
  /** Libellé tronqué à la largeur disponible (px écran, ~7 px par caractère à 12 px). */
  const cut = (t: string, px: number) => { const n = Math.floor((px - 12) / 7); return t.length > n ? `${t.slice(0, Math.max(1, n - 1))}…` : t; };
  function key(e: KeyboardEvent, kind: 'c' | 'p', id: string) {
    if (e.key === 'Enter' && kind === 'c' && !armed) { e.preventDefault(); onequip(id); }
    else if (e.key === ' ' || e.key === 'Enter') { e.preventDefault(); onhit(kind, id); }
  }
  let over = $state<string | null>(null);
  let bad = $state(false);
  const dz = (kind: 'c' | 'p', id: string) => ({
    ondragover: (e: DragEvent) => {
      e.preventDefault(); over = `${kind}:${id}`;
      bad = !(onover?.(kind, id) ?? true);
      if (e.dataTransfer) e.dataTransfer.dropEffect = bad ? 'none' : 'move';
    },
    ondragleave: (e: DragEvent) => { if (over === `${kind}:${id}` && !(e.currentTarget as Element).contains(e.relatedTarget as Node)) { over = null; bad = false; onover?.(kind, null); } },
    ondrop: (e: DragEvent) => { e.preventDefault(); over = null; bad = false; ondropon(kind, id); },
    onpointerenter: () => { if (armed) { over = `${kind}:${id}`; bad = !(onover?.(kind, id) ?? true); } },
    onpointerleave: () => { if (armed) { over = null; bad = false; onover?.(kind, null); } },
  });
</script>

<svg class="dd" class:armed class:bad viewBox="{vb.x} {vb.y} {vb.w} {vb.h}" width={vb.w * k} height={vb.h * k} role="group" aria-label="{desk.name} · {mode === 'side' ? 'côté' : 'dessus'}" style="--sw:{1.2 / k}">
  {#if mode === 'side'}
    {@const Dp = dd.depth_mm}
    {@const Hd = dd.height_mm}
    <line class="floor" x1={-dy} y1="0" x2={ext.D + 40 / k} y2="0" />
    <rect class="body" x="20" y={-Hd + 32} width={Dp - 40} height={Hd - 32} />
    <rect class="top" x="0" y={-Hd} width={Dp} height="32" rx="6" />
    {#each order as c (c.id)}
      {@const ct = casq.find((x) => x.id === c.id)!}
      <polygon class="cq" class:sel={sel === c.id} class:over={over === `c:${c.id}`} points={pts(casqProfile(dd, c))} role="button" tabindex="0" aria-pressed={sel === c.id}
        aria-label="{nameOf(ct)} : Espace pour sélectionner, Entrée pour équiper" onclick={() => onhit('c', c.id)} ondblclick={() => onequip(c.id)} onkeydown={(e) => key(e, 'c', c.id)} {...dz('c', c.id)}><title>{nameOf(ct)} · {c.units} U · {nf(c.tilt_deg)}°</title></polygon>
    {/each}
    <!-- Cotes : hauteur du plateau à gauche, profondeur en bas. -->
    <g class="dim">
      <line x1={-dy} y1="0" x2={-dy} y2={-Hd} /><line x1={-dy - 20} y1="0" x2={-dy + 20} y2="0" /><line x1={-dy - 20} y1={-Hd} x2={-dy + 20} y2={-Hd} />
      <text x={-dy - fs * 0.45} y={-Hd / 2} font-size={fs} text-anchor="middle" transform="rotate(-90 {-dy - fs * 0.45} {-Hd / 2})">{nf(Hd)} mm</text>
      <line x1="0" y1={dy} x2={Dp} y2={dy} /><line x1="0" y1={dy - 20} x2="0" y2={dy + 20} /><line x1={Dp} y1={dy - 20} x2={Dp} y2={dy + 20} />
      <text x={Dp / 2} y={dy - fs * 0.45} font-size={fs} text-anchor="middle">{nf(Dp)} mm</text>
    </g>
    <text class="op" x="0" y={dy + fs * 1.9} font-size={fs}>OPÉRATEUR</text>
  {:else}
    {@const Dp = dd.depth_mm}
    {@const L = dd.length_mm}
    {@const y0 = ext.D - Dp}
    <rect class="top" x="0" y={y0} width={L} height={Dp} rx="10" />
    <rect class="tray" x="0" y={y0} width={L} height="110" />
    {#each posts as p, i (p.id)}
      {@const pw = L / posts.length}
      <g class="post" class:sel={selP === p.id} class:over={over === `p:${p.id}`} role="button" tabindex="0" aria-pressed={selP === p.id} aria-label="Poste {i + 1} · {p.role || 'Poste'} : Espace pour sélectionner"
        onclick={() => onhit('p', p.id)} onkeydown={(e) => key(e, 'p', p.id)} {...dz('p', p.id)}>
        <rect x={i * pw + 14} y={ext.D - PH} width={pw - 28} height={PH - 14} rx="18" />
        {#if pw * k > 36}<text x={i * pw + pw / 2} y={ext.D - PH / 2 + fs * 0.35} font-size={fs} text-anchor="middle">{cut(`${i + 1} · ${p.role || 'Poste'} · ${gearOf(p)}`, (pw - 28) * k)}</text>{/if}
        <title>Poste {i + 1} · {p.role || 'Poste'}{p.name ? ` · ${p.name}` : ''} · {gearOf(p)} exemplaire{gearOf(p) > 1 ? 's' : ''}</title>
      </g>
    {/each}
    {#each order as c (c.id)}
      {@const ct = casq.find((x) => x.id === c.id)!}
      {@const pr = casqProfile(dd, c)}
      {@const z0 = Math.min(...pr.map((q) => q[0]))}
      {@const z1 = Math.max(...pr.map((q) => q[0]))}
      {@const cx = Math.max(0, Math.min(L - CASQ_W, c.offset_mm))}
      <g class="cq" class:sel={sel === c.id} class:over={over === `c:${c.id}`} role="button" tabindex="0" aria-pressed={sel === c.id} aria-label="{nameOf(ct)} : Espace pour sélectionner, Entrée pour équiper"
        onclick={() => onhit('c', c.id)} ondblclick={() => onequip(c.id)} onkeydown={(e) => key(e, 'c', c.id)} {...dz('c', c.id)}>
        <rect x={cx} y={ext.D - z1} width={CASQ_W} height={z1 - z0} rx="6" />
        {#if CASQ_W * k > 36}<text x={cx + CASQ_W / 2} y={ext.D - (z0 + z1) / 2 + fs * 0.35} font-size={fs} text-anchor="middle">{cut(nameOf(ct), CASQ_W * k)}</text>{/if}
        <title>{nameOf(ct)} · {c.units} U · {nf(c.offset_mm)} mm</title>
      </g>
    {/each}
    <g class="dim">
      <line x1="0" y1={ext.D + dy} x2={L} y2={ext.D + dy} /><line x1="0" y1={ext.D + dy - 20} x2="0" y2={ext.D + dy + 20} /><line x1={L} y1={ext.D + dy - 20} x2={L} y2={ext.D + dy + 20} />
      <text x={L / 2} y={ext.D + dy - fs * 0.45} font-size={fs} text-anchor="middle">{nf(L)} mm</text>
      <line x1={-dy} y1={y0} x2={-dy} y2={ext.D} /><line x1={-dy - 20} y1={y0} x2={-dy + 20} y2={y0} /><line x1={-dy - 20} y1={ext.D} x2={-dy + 20} y2={ext.D} />
      <text x={-dy - fs * 0.45} y={y0 + Dp / 2} font-size={fs} text-anchor="middle" transform="rotate(-90 {-dy - fs * 0.45} {y0 + Dp / 2})">{nf(Dp)} mm</text>
    </g>
    <text class="op" x={L / 2} y={ext.D + dy + fs * 1.9} font-size={fs} text-anchor="middle">CÔTÉ OPÉRATEUR</text>
  {/if}
</svg>

<style>
  .dd { display: block; flex: none; overflow: visible; }
  .dd :global(*) { vector-effect: non-scaling-stroke; }
  .floor { stroke: var(--line-2); stroke-width: 1.5; }
  .top { fill: color-mix(in srgb, var(--fg) 22%, var(--surface)); stroke: var(--fg-2); stroke-width: 1.2; }
  .body { fill: color-mix(in srgb, var(--fg) 8%, var(--surface)); stroke: var(--fg-3); stroke-width: 1.2; }
  .tray { fill: color-mix(in srgb, var(--fg) 14%, transparent); }
  .cq { cursor: pointer; outline: none; }
  polygon.cq, .cq rect { fill: var(--surface); stroke: var(--fg-2); stroke-width: 1.5; }
  .cq text, .post text { fill: var(--fg); font-family: var(--font); font-weight: 600; pointer-events: none; }
  .cq:hover rect, polygon.cq:hover { stroke: var(--accent); }
  .cq.sel rect, polygon.cq.sel, .cq:focus-visible rect, polygon.cq:focus-visible { fill: var(--accent-soft); stroke: var(--accent); stroke-width: 2.5; }
  .post { cursor: pointer; outline: none; }
  .post rect { fill: color-mix(in srgb, var(--surface) 82%, transparent); stroke: var(--fg-3); stroke-width: 1.2; stroke-dasharray: 5 4; }
  .post:hover rect { stroke: var(--accent); }
  .post.sel rect, .post:focus-visible rect { fill: var(--accent-soft); stroke: var(--accent); stroke-width: 2.5; stroke-dasharray: none; }
  .over rect, polygon.over { stroke: var(--accent); stroke-width: 3; fill: var(--accent-soft); }
  .bad .over rect, .bad polygon.over { stroke: var(--err); fill: var(--err-soft); }
  .bad .over { cursor: not-allowed; }
  .armed .cq rect, .armed polygon.cq, .armed .post rect { stroke: var(--accent); }
  .dim line { stroke: var(--fg-3); stroke-width: 1; }
  .dim text, .op { fill: var(--fg-3); font-family: var(--mono); }
  .op { font-family: var(--font); font-weight: 600; letter-spacing: .06em; }
</style>
