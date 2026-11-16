<script lang="ts">
  // Planches (section 9.8) : composition des baies (vues avant, arrière, côté, 3D ; rendu réel,
  // schématique ou 3D), du mobilier (face, côté, dessus, 3D, casquettes), des synoptiques, de
  // titres de lieux et d'emplacements réservés, sur une ou plusieurs feuilles avec cartouche.
  // La feuille occupe la page (zoom, défilement) ; le panneau de droite montre l'élément
  // sélectionné, ou la bibliothèque et le cartouche. Chaque dessin se redimensionne par ses
  // poignées de coin (échelle propre, proportions gardées, coin opposé fixe) ou par son champ
  // Échelle (%) ; un synoptique dont le texte passe sous 6 pt est signalé. Réglages mémorisés
  // par projet ; export PDF
  // vectoriel, PNG ou JPG. Enregistrement automatique (lib/autosave), pile Annuler / Rétablir,
  // clavier commun des canevas (lib/canvaskeys) : Échap, Suppr, Ctrl+Z, flèches (1 mm, 10 mm avec Maj).
  import { tick, untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import Seg from '../../components/Seg.svelte';
  import Menu from '../../components/Menu.svelte';
  import SaveState from '../../components/SaveState.svelte';
  import Loading from '../../components/Loading.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import NumField from '../../components/NumField.svelte';
  import SheetPage from '../../components/SheetPage.svelte';
  import Sheet3DView from '../../components/Sheet3DView.svelte';
  import { api } from '../../lib/api';
  import { revision } from '../../lib/pdfkit';
  import { ask } from '../../lib/ask.svelte';
  import { autosave } from '../../lib/autosave';
  import { canvasKeys } from '../../lib/canvaskeys';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { loadRefs } from '../../lib/catalog.svelte';
  import type { ProjectView } from '../../lib/projects';
  import {
    PAPERS, SCALES, RACK_VIEWS, DESK_VIEWS, RACK_RENDERS, FRAME, MARGIN, BLOCK, MIN_PT, SYN_LEGIBLE, SYN_MM, SYN_MIN, ZOOM_MAX, ZOOM_MIN,
    autoLayout, blankSheet, fileName, groupLayout, isDrawing, itemSize, minTextPt, normalize, pageSize, scaleOf, shotKey, shotViews, uid, usableW, viewsOf, zoomOf,
    type Corner, type Ctx, type Paper, type RackRender, type Sheet, type SheetItem,
  } from '../../lib/sheet';
  import { isDesk, loadCtx, loadSyn, shot3D, shotsFor, synIds, synViews } from '../../lib/sheetctx';
  import { pl } from '../../lib/format';

  let { view }: { view: ProjectView } = $props();
  loadRefs();
  const pid = $derived(view.project.id);
  const url = $derived(`/projects/${pid}/settings/rack_sheet`);
  const writable = $derived(can('projets', 'ecriture'));
  const bays = $derived(view.containers.filter((c) => c.rackable && c.kind !== 'casquette'));
  const desks = $derived(view.containers.filter(isDesk));
  const synList = $derived(synViews(view));

  let sheet = $state<Sheet | null>(null);
  let rev = 0;
  let ctx = $state<Ctx>({ racks: {}, desks: {} });
  let shots = $state<Record<string, string>>({});
  let site = $state('Strata Arkyo');
  let cur = $state(0);
  let selected = $state<string | null>(null);
  let relayout = $state(false);
  let rackViews = $state<string[]>(['front', 'rear', 'side']);
  let deskViews = $state<string[]>(['front', 'side', 'top']);
  let exporting = $state(false);
  let svgHost = $state<HTMLDivElement>(null!);
  let exportHost = $state<HTMLDivElement>(null!);
  let exportShots = $state<Record<string, string>>({});
  let viewer = $state<{ frame: (v: 'front' | 'rear' | 'quarter' | 'side' | 'top') => void } | null>(null);

  const info = $derived({
    project: view.project.name, client: view.project.client ?? 'Projet interne', venue: view.project.venue ?? '', site,
    date: new Date().toLocaleDateString('fr-FR'), revision: revision(view.project),
    locations: Object.fromEntries(view.locations.map((l) => [l.id, { name: l.name, code: l.code }])),
  });

  async function load() {
    try {
      const [s, st] = await Promise.all([api.get<{ data: Sheet | null; rev: number }>(url), api.get<{ name: string }>('/site')]);
      site = st.name; rev = s.rev;
      ctx = await loadCtx(view, synIds(s.data?.pages ? s.data : null));
      if (s.data && s.data.pages) sheet = normalize({ ...blankSheet(), ...s.data, block: { ...blankSheet().block, ...s.data.block } });
      else { const b = blankSheet(); b.pages = [{ id: uid(), items: [] }]; sheet = b; }
      cur = Math.min(cur, sheet.pages.length - 1);
      resetHistory();
    } catch (e) { fail(e); }
  }
  // Le projet se recharge sur les événements des autres membres : les dessins suivent.
  $effect(() => { view; untrack(() => { if (!sheet) load(); else loadCtx(view, synIds(sheet)).then((c) => (ctx = c)); }); });
  /** Synoptique d'un élément, chargé à la demande. */
  async function ensureSyn(cid: string) {
    if (ctx.syn && cid in ctx.syn) return ctx.syn[cid];
    const r = await loadSyn(pid, cid);
    ctx.syn = { ...(ctx.syn ?? {}), [cid]: r };
    return r;
  }

  const layout = (s: Sheet) => autoLayout(s, ctx, bays, desks, view.locations, rackViews.length ? rackViews : ['front'], deskViews.length ? deskViews : ['front']);

  // ----- Enregistrement automatique (debounce, vidage en sortie, nouvel essai, conflit avec choix) -----
  const saver = autosave({
    save: async () => { if (!sheet) return; const r = await api.put<{ rev: number }>(url, { rev, data: $state.snapshot(sheet) }); rev = r.rev; },
    // « Garder ma version » : on relit la révision du serveur, puis on réécrit la nôtre.
    keepMine: async () => { const s = await api.get<{ rev: number }>(url); rev = s.rev; },
    reload: async () => { sheet = null; await load(); },
  });

  // ----- Pile Annuler / Rétablir : instantanés de la planche après chaque modification validée -----
  let past: string[] = [], future: string[] = [], base = '';
  let hist = $state({ undo: 0, redo: 0 });
  let lastKey = '', lastAt = 0;
  const snap = () => JSON.stringify($state.snapshot(sheet));
  function resetHistory() { past = []; future = []; base = sheet ? snap() : ''; lastKey = ''; hist = { undo: 0, redo: 0 }; }
  /** Après chaque modification validée. `key` regroupe les gestes répétés (flèches, curseurs) en un seul pas d'annulation. */
  function changed(key = '') {
    if (!writable || !sheet) return;
    const now = Date.now();
    if (!(key && key === lastKey && now - lastAt < 1200)) { if (base) { past.push(base); if (past.length > 60) past.shift(); } future = []; }
    lastKey = key; lastAt = now;
    base = snap();
    hist = { undo: past.length, redo: future.length };
    saver.changed();
  }
  function restore(s: string) {
    const next = JSON.parse(s) as Sheet;
    sheet = next; base = s; lastKey = '';
    cur = Math.min(cur, next.pages.length - 1);
    if (selected && !next.pages[cur]?.items.some((i) => i.id === selected)) selected = null;
    hist = { undo: past.length, redo: future.length };
    saver.changed();
  }
  function undo() { if (!writable || !sheet || !past.length) return; future.push(base); restore(past.pop()!); }
  function redo() { if (!writable || !sheet || !future.length) return; past.push(base); restore(future.pop()!); }

  const page = $derived(sheet?.pages[cur] ?? null);
  const sel = $derived(page?.items.find((i) => i.id === selected) ?? null);
  const nameOf = (it: SheetItem) => (it.kind === 'rack' ? ctx.racks[it.container_id ?? '']?.name : it.kind === 'desk' ? ctx.desks[it.container_id ?? '']?.name : it.kind === 'synoptic' ? view.containers.find((c) => c.id === it.container_id)?.name : it.kind === 'location' ? (info.locations[it.location_id ?? '']?.name ?? 'Titre d’emplacement') : 'Emplacement réservé') ?? 'Élément retiré';
  const selName = $derived(sel ? nameOf(sel) : '');
  const KIND: Record<SheetItem['kind'], string> = { rack: 'Rack', desk: 'Mobilier', synoptic: 'Synoptique', location: 'Titre d’emplacement', reserved: 'Emplacement réservé' };
  const box3d = $derived(sel && sheet && (sel.kind === 'rack' || sel.kind === 'desk') ? groupLayout(sel, sheet.scale, ctx).views.find((v) => v.v === '3d') ?? null : null);

  // Vues 3D de la feuille affichée (et élévations en rendu 3D) : rendues sans fond, au format
  // du cadre, à chaque changement ; pas pendant un redimensionnement (rendu au lâcher).
  $effect(() => {
    if (!sheet || !page || resizing) return;
    const k = sheet.scale;
    const list = page.items.filter((i) => shotViews(i).length);
    void JSON.stringify(list); void ctx;
    untrack(() => { for (const it of list) for (const v of shotViews(it)) shot3D($state.snapshot(it) as SheetItem, ctx, k, v).then((u) => { if (u) shots[shotKey(it.id, v)] = u; }).catch(() => {}); });
  });

  /** Pose un élément en (x, y) mm, gardé dans le cadre de la feuille. */
  function putAt(it: SheetItem, x: number, y: number) {
    if (!sheet) return;
    const [W, H] = pageSize(sheet);
    const [w, h] = itemSize(it, sheet.scale, ctx, usableW(sheet));
    it.x = Math.round(Math.min(Math.max(FRAME, x), Math.max(FRAME, W - FRAME - w)));
    it.y = Math.round(Math.min(Math.max(FRAME, y), Math.max(FRAME, H - FRAME - h)));
  }

  // Glisser un élément de la feuille, au millimètre (juste à tout zoom : repère de l'écran du SVG).
  // Un clic sans déplacement sélectionne seulement : rien n'est enregistré.
  let drag: { it: SheetItem; dx: number; dy: number; x0: number; y0: number } | null = null;
  function toMm(e: PointerEvent): [number, number] {
    const svg = svgHost.querySelector('svg')!;
    const p = new DOMPoint(e.clientX, e.clientY).matrixTransform(svg.getScreenCTM()!.inverse());
    return [p.x, p.y];
  }
  function pick(it: SheetItem, e: PointerEvent) {
    selected = it.id;
    if (!writable) return;
    e.preventDefault();
    const [x, y] = toMm(e);
    drag = { it, dx: x - it.x, dy: y - it.y, x0: it.x, y0: it.y };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up, { once: true });
  }
  function move(e: PointerEvent) {
    if (!drag || !sheet) return;
    const [x, y] = toMm(e);
    putAt(drag.it, x - drag.dx, y - drag.dy);
  }
  function up() {
    window.removeEventListener('pointermove', move);
    const d = drag; drag = null;
    if (d && (d.it.x !== d.x0 || d.it.y !== d.y0)) changed();
  }

  // Redimensionner un dessin par l'une de ses poignées de coin : échelle propre, proportions
  // gardées (un dessin à l'échelle ne se déforme pas), le coin opposé reste en place.
  let rz: { it: SheetItem; w0: number; h0: number; z0: number; x0: number; y0: number; ax: number; ay: number; sx: number; sy: number } | null = null;
  let resizing = $state(false);
  const zMin = (it: SheetItem) => (it.kind === 'synoptic' ? SYN_MIN : ZOOM_MIN);
  const clampZ = (it: SheetItem, z: number) => Math.round(Math.min(ZOOM_MAX, Math.max(zMin(it), z)) * 1000) / 1000;
  function startResize(it: SheetItem, e: PointerEvent, corner: Corner = 'se') {
    if (!writable || !sheet) return;
    e.preventDefault();
    selected = it.id;
    const [w0, h0] = itemSize(it, sheet.scale, ctx, usableW(sheet));
    const sx = corner[1] === 'e' ? 1 : -1, sy = corner[0] === 's' ? 1 : -1;
    rz = { it, w0, h0, z0: zoomOf(it), x0: it.x, y0: it.y, ax: sx > 0 ? it.x : it.x + w0, ay: sy > 0 ? it.y : it.y + h0, sx, sy }; resizing = true;
    window.addEventListener('pointermove', moveResize);
    window.addEventListener('pointerup', upResize, { once: true });
  }
  function moveResize(e: PointerEvent) {
    if (!rz || !sheet) return;
    const [x, y] = toMm(e);
    // Projection du pointeur sur la diagonale issue du coin opposé : agrandit ou réduit des deux côtés à la fois.
    const dx = (x - rz.ax) * rz.sx, dy = (y - rz.ay) * rz.sy;
    const ratio = (dx * rz.w0 + dy * rz.h0) / (rz.w0 * rz.w0 + rz.h0 * rz.h0);
    rz.it.zoom = clampZ(rz.it, rz.z0 * Math.max(0.02, ratio));
    const [w, h] = itemSize(rz.it, sheet.scale, ctx, usableW(sheet));
    rz.it.x = Math.round((rz.sx > 0 ? rz.ax : rz.ax - w) * 10) / 10;
    rz.it.y = Math.round((rz.sy > 0 ? rz.ay : rz.ay - h) * 10) / 10;
  }
  function upResize() {
    window.removeEventListener('pointermove', moveResize);
    const r = rz; rz = null; resizing = false;
    if (!r) return;
    keepIn(r.it);
    // Agrandi par un coin haut ou gauche : le dessin reste dans le cadre.
    r.it.x = Math.max(FRAME, r.it.x); r.it.y = Math.max(FRAME, r.it.y);
    if (zoomOf(r.it) !== r.z0 || r.it.x !== r.x0 || r.it.y !== r.y0) changed();
  }
  /** Le dessin dépasse-t-il la zone dans le cadre ? */
  const tooBig = (it: SheetItem) => { if (!sheet) return false; const [W, H] = pageSize(sheet), [w, h] = itemSize(it, sheet.scale, ctx, usableW(sheet)); return w > W - 2 * FRAME + 0.5 || h > H - 2 * FRAME + 0.5; };
  /** Taille propre : `commit` à faux pendant le glissement du curseur (aperçu), vrai au lâcher. */
  function setZoom(it: SheetItem, z: number, commit = true) { it.zoom = clampZ(it, z); keepIn(it); if (commit) changed('zoom:' + it.id); }
  /** Synoptique posé entier dans le cadre de la feuille (même calcul qu'à l'ajout). */
  function fitSel(it: SheetItem) {
    const r = ctx.syn?.[it.container_id ?? '']; if (!r || !sheet) return;
    setZoom(it, synFit(r));
    putAt(it, it.x, it.y);
  }
  /** Échelle propre en %, saisie au clavier (alternative aux poignées), validée en sortie de champ. */
  const pctOf = (it: SheetItem) => Math.round(zoomOf(it) * 1000) / 10;
  let zPct = $state<number | null>(null);
  $effect(() => { if (sel) { const v = pctOf(sel); untrack(() => (zPct = v)); } });
  function commitPct() { if (!sel) return; if (zPct != null && zPct > 0) setZoom(sel, zPct / 100); zPct = pctOf(sel); }
  /** Corps en points, à la française (« 4,2 »). */
  const ptTxt = (v: number) => v.toFixed(1).replace('.', ',');
  /** Plus petit texte de l’élément sélectionné, en points (synoptique). */
  const selPt = $derived(sel ? minTextPt(sel) : null);
  /** Dessins dont le texte passe sous 6 pt à leur échelle, toutes feuilles : signalés sur la
   *  feuille et rappelés avant l'export. */
  const tiny = $derived((sheet?.pages ?? []).flatMap((p, i) => p.items.filter((it) => (minTextPt(it) ?? MIN_PT) < MIN_PT - 0.05).map((it) => ({ name: nameOf(it), page: i + 1, pt: minTextPt(it)! }))));
  /** Éléments qui sortent du cadre, toutes feuilles : signalés dans la barre et rappelés avant l'export. */
  const outside = $derived.by(() => {
    if (!sheet) return [] as { name: string; page: number }[];
    const s = sheet, [W, H] = pageSize(s);
    return s.pages.flatMap((p, i) => p.items.filter((it) => {
      const [w, h] = itemSize(it, s.scale, ctx, usableW(s));
      return it.x < FRAME - 0.5 || it.y < FRAME - 0.5 || it.x + w > W - FRAME + 0.5 || it.y + h > H - FRAME + 0.5;
    }).map((it) => ({ name: nameOf(it), page: i + 1 })));
  });

  // ----- Zoom de la feuille à l'écran -----
  // 1 = feuille entière visible (« Ajuster ») ; molette + Ctrl (ou pincement du pavé tactile)
  // zoome autour du pointeur ; le défilement déplace la feuille quand elle dépasse.
  let zoom = $state(1);
  let deskEl = $state<HTMLDivElement>(null!);
  let deskW = $state(0), deskH = $state(0);
  // Téléphone : marge réduite autour de la feuille (même valeur que la règle CSS).
  let narrow = $state(false);
  $effect(() => {
    const mq = window.matchMedia('(max-width: 600px)');
    const on = () => (narrow = mq.matches); on();
    mq.addEventListener('change', on);
    return () => mq.removeEventListener('change', on);
  });
  const DESK_PAD = $derived(narrow ? 12 : 28);
  const fitW = $derived.by(() => {
    if (!sheet || !deskW) return 800;
    const [W, H] = pageSize(sheet);
    return Math.max(200, Math.min(deskW - 2 * DESK_PAD, ((deskH - 2 * DESK_PAD) * W) / H));
  });
  const paperW = $derived(Math.round(fitW * zoom));
  async function zoomTo(z: number, cx?: number, cy?: number) {
    if (!deskEl) { zoom = z; return; }
    z = Math.min(8, Math.max(0.25, z));
    const r = deskEl.getBoundingClientRect();
    const px = cx ?? r.left + r.width / 2, py = cy ?? r.top + r.height / 2;
    const paper = deskEl.querySelector('.pl-paper') as HTMLElement | null;
    const pr = paper?.getBoundingClientRect();
    // Point de la feuille sous le pointeur (fraction de sa taille) : il y reste après le zoom.
    const fx = pr ? (px - pr.left) / pr.width : 0.5, fy = pr ? (py - pr.top) / pr.height : 0.5;
    zoom = z;
    await tick();
    const nr = paper?.getBoundingClientRect();
    if (nr) { deskEl.scrollLeft += nr.left + fx * nr.width - px; deskEl.scrollTop += nr.top + fy * nr.height - py; }
  }
  $effect(() => {
    if (!deskEl) return;
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return;
      e.preventDefault();
      zoomTo(zoom * Math.exp(-e.deltaY * (e.deltaMode === 1 ? 0.05 : 0.0025)), e.clientX, e.clientY);
    };
    // Pincement à deux doigts (écran tactile).
    const touches = new Map<number, [number, number]>();
    let pinch: { d: number; z: number } | null = null;
    const dist = () => { const [a, b] = [...touches.values()]; return a && b ? Math.hypot(a[0] - b[0], a[1] - b[1]) : 0; };
    const down = (e: PointerEvent) => { if (e.pointerType !== 'touch') return; touches.set(e.pointerId, [e.clientX, e.clientY]); if (touches.size === 2) { pinch = { d: dist(), z: zoom }; drag = null; } };
    const mv = (e: PointerEvent) => {
      if (!touches.has(e.pointerId)) return;
      touches.set(e.pointerId, [e.clientX, e.clientY]);
      if (pinch && touches.size === 2 && pinch.d > 0) { const [a, b] = [...touches.values()]; zoomTo(pinch.z * (dist() / pinch.d), (a![0] + b![0]) / 2, (a![1] + b![1]) / 2); }
    };
    const end = (e: PointerEvent) => { touches.delete(e.pointerId); if (touches.size < 2) pinch = null; };
    deskEl.addEventListener('wheel', onWheel, { passive: false });
    deskEl.addEventListener('pointerdown', down, true);
    window.addEventListener('pointermove', mv);
    window.addEventListener('pointerup', end);
    window.addEventListener('pointercancel', end);
    return () => { deskEl.removeEventListener('wheel', onWheel); deskEl.removeEventListener('pointerdown', down, true); window.removeEventListener('pointermove', mv); window.removeEventListener('pointerup', end); window.removeEventListener('pointercancel', end); };
  });

  /** Première place libre de la feuille courante (de haut en bas, puis de gauche à droite) ; sinon, décalé du dernier élément. */
  function freeSpot(it: SheetItem): [number, number] {
    if (!sheet || !page) return [MARGIN + 4, MARGIN + 4];
    const s = sheet, [W, H] = pageSize(s), [w, h] = itemSize(it, s.scale, ctx, usableW(s));
    const GAP = 6, x0 = MARGIN + 4, y0 = MARGIN + 4;
    const boxes = page.items.map((o) => { const [ow, oh] = itemSize(o, s.scale, ctx, usableW(s)); return [o.x, o.y, ow, oh] as const; });
    if (s.titleBlock) boxes.push([W - FRAME - BLOCK[0], H - FRAME - BLOCK[1], BLOCK[0], BLOCK[1]]);
    const xs = [...new Set([x0, ...boxes.map((b) => b[0] + b[2] + GAP)])].sort((a, b) => a - b);
    const ys = [...new Set([y0, ...boxes.map((b) => b[1] + b[3] + GAP)])].sort((a, b) => a - b);
    const free = (x: number, y: number) => x + w <= W - FRAME && y + h <= H - FRAME && !boxes.some((b) => x < b[0] + b[2] + GAP / 2 && x + w + GAP / 2 > b[0] && y < b[1] + b[3] + GAP / 2 && y + h + GAP / 2 > b[1]);
    for (const y of ys) for (const x of xs) if (free(x, y)) return [Math.round(x), Math.round(y)];
    const last = page.items.at(-1);
    return last ? [last.x + 10, last.y + 10] : [x0, y0];
  }
  function add(it: Omit<SheetItem, 'id' | 'x' | 'y'>) {
    if (!page) return;
    const n: SheetItem = { id: uid(), x: MARGIN + 4, y: MARGIN + 4, ...it };
    [n.x, n.y] = freeSpot(n);
    page.items.push(n);
    const p = page.items.at(-1);
    if (p) keepIn(p);
    selected = n.id;
    changed();
  }
  /** Échelle qui fait tenir un synoptique entier dans le cadre (texte plafonné à 160 %). */
  function synFit(r: { w: number; h: number }): number {
    if (!sheet) return 1;
    const [W, H] = pageSize(sheet);
    const fit = Math.min((W - 2 * MARGIN - 8) / (r.w * SYN_MM), (H - 2 * MARGIN - 20 - (sheet.titleBlock ? BLOCK[1] : 0)) / (r.h * SYN_MM));
    return Math.floor(Math.min(1.6, Math.max(SYN_MIN, fit)) * 1000) / 1000;
  }
  /** Synoptique d'un élément : posé entier, à l'échelle qui le fait tenir dans le cadre ; sous
   *  6 pt, il est signalé et se règle par ses poignées ou son champ Échelle. */
  async function addSyn(cid: string) {
    if (!sheet) return;
    const r = await ensureSyn(cid);
    add({ kind: 'synoptic', container_id: cid, zoom: r ? synFit(r) : 1 });
  }
  /** Retire l'élément sélectionné : notification avec Annuler (et Ctrl+Z). */
  function removeSel() {
    if (!page || !sel || !writable) return;
    const it = $state.snapshot(sel) as SheetItem, pageId = page.id, idx = page.items.indexOf(sel), name = nameOf(sel);
    page.items = page.items.filter((i) => i.id !== it.id); selected = null; changed();
    toast(`« ${name} » retiré de la feuille.`, 'ok', { run: () => {
      const i = sheet?.pages.findIndex((p) => p.id === pageId) ?? -1;
      const p = sheet?.pages[i];
      if (!p || p.items.some((x) => x.id === it.id)) return false;
      p.items.splice(Math.min(idx, p.items.length), 0, it);
      cur = i; selected = it.id; changed();
    } });
  }
  function toPage(n: number) {
    if (!sheet || !page || !sel) return;
    if (n >= sheet.pages.length) sheet.pages.push({ id: uid(), items: [] });
    const it = sel;
    page.items = page.items.filter((i) => i !== it);
    sheet.pages[n]!.items.push(it);
    cur = n; changed();
  }
  function addPage() { if (!sheet) return; sheet.pages.push({ id: uid(), items: [] }); cur = sheet.pages.length - 1; selected = null; changed(); }
  /** Supprime la feuille affichée : confirmation si elle contient des éléments, puis notification avec Annuler. */
  async function dropPage() {
    if (!sheet || sheet.pages.length < 2 || !writable) return;
    const i = cur, n = sheet.pages[i]!.items.length;
    if (n && !(await ask({
      title: `Supprimer la feuille ${i + 1} ?`,
      text: n === 1 ? 'L’élément qu’elle contient est retiré de la planche. Vous pourrez annuler juste après.' : `Les ${n} éléments qu’elle contient sont retirés de la planche. Vous pourrez annuler juste après.`,
      confirmLabel: 'Supprimer la feuille', danger: true,
    }))) return;
    if (!sheet || !sheet.pages[i]) return;
    const copy = $state.snapshot(sheet.pages[i]!);
    sheet.pages.splice(i, 1); cur = Math.max(0, i - 1); selected = null; changed();
    toast(`Feuille ${i + 1} supprimée.`, 'ok', { run: () => {
      if (!sheet || sheet.pages.some((p) => p.id === copy.id)) return false;
      sheet.pages.splice(Math.min(i, sheet.pages.length), 0, copy);
      cur = i; changed();
    } });
  }
  function doLayout() { if (!sheet) return; sheet.pages = layout(sheet); cur = 0; selected = null; relayout = false; changed(); }
  /** Feuilles où figure un élément (numéros à partir de 1). */
  const where = (cid: string, syn = false) => (sheet?.pages ?? []).flatMap((p, i) => (p.items.some((x) => x.container_id === cid && (syn ? x.kind === 'synoptic' : x.kind === 'rack' || x.kind === 'desk')) ? [i + 1] : []));
  function toggleView(it: SheetItem, v: string) {
    const order = Object.keys(it.kind === 'rack' ? RACK_VIEWS : DESK_VIEWS);
    const cur = viewsOf(it);
    const next = cur.includes(v) ? cur.filter((x) => x !== v) : [...cur, v];
    if (!next.length) return;
    it.views = order.filter((x) => next.includes(x));
    keepIn(it);
    changed();
  }
  /** Un groupe qui s'élargit reste sur la feuille. */
  function keepIn(it: SheetItem) {
    if (!sheet) return;
    const [W, H] = pageSize(sheet), [w, h] = itemSize(it, sheet.scale, ctx, usableW(sheet));
    if (it.x + w > W - FRAME) it.x = Math.max(FRAME, Math.round(W - FRAME - w));
    if (it.y + h > H - FRAME) it.y = Math.max(FRAME, Math.round(H - FRAME - h));
  }
  const locName = (id: string) => view.locations.find((l) => l.id === id);

  // Position de l'élément sélectionné, en mm (alternative clavier au glisser).
  let pos = $state<{ x: number | null; y: number | null }>({ x: null, y: null });
  $effect(() => { if (sel) { const x = sel.x, y = sel.y; untrack(() => { pos.x = x; pos.y = y; }); } });
  function commitPos() { if (!sel) return; putAt(sel, pos.x ?? sel.x, pos.y ?? sel.y); pos.x = sel.x; pos.y = sel.y; changed('pos:' + sel.id); }

  // ----- Clavier commun des canevas -----
  /** Flèches : déplace l'élément sélectionné de 1 mm (10 mm avec Maj). */
  function nudge(e: KeyboardEvent) {
    if (!writable || !sel || e.ctrlKey || e.metaKey || e.altKey) return false;
    const d = ({ ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] } as Record<string, [number, number]>)[e.key];
    if (!d) return false;
    const k = e.shiftKey ? 10 : 1;
    putAt(sel, sel.x + d[0] * k, sel.y + d[1] * k);
    changed('nudge:' + sel.id);
    return true;
  }
  $effect(() => canvasKeys({
    enabled: () => !!sheet,
    onEscape: () => { if (selected) selected = null; },
    onDelete: () => { if (sel) removeSel(); },
    onUndo: undo,
    onRedo: redo,
    onKey: nudge,
  }));

  // ----- Export -----
  function download(blob: Blob, name: string) {
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob); a.download = name; document.body.appendChild(a); a.click(); a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 2000);
  }
  async function raster(svg: SVGSVGElement, type: 'image/png' | 'image/jpeg'): Promise<Blob> {
    const [W, H] = pageSize(sheet!);
    const dpi = Math.min(200, Math.sqrt(30e6 / ((W / 25.4) * (H / 25.4))));
    const pw = Math.round((W / 25.4) * dpi), ph = Math.round((H / 25.4) * dpi);
    const clone = svg.cloneNode(true) as SVGSVGElement;
    clone.setAttribute('width', String(pw)); clone.setAttribute('height', String(ph));
    const url = URL.createObjectURL(new Blob([new XMLSerializer().serializeToString(clone)], { type: 'image/svg+xml' }));
    try {
      const img = new Image();
      await new Promise((ok, ko) => { img.onload = ok; img.onerror = ko; img.src = url; });
      const c = document.createElement('canvas'); c.width = pw; c.height = ph;
      const g = c.getContext('2d')!; g.fillStyle = '#fff'; g.fillRect(0, 0, pw, ph); g.drawImage(img, 0, 0, pw, ph);
      return await new Promise<Blob>((ok, ko) => c.toBlob((b) => (b ? ok(b) : ko(new Error('Image impossible'))), type, 0.92));
    } finally { URL.revokeObjectURL(url); }
  }
  /** Une planche sans aucun élément n'est pas exportée (feuilles vides). */
  const hasItems = $derived(!!sheet?.pages.some((p) => p.items.length));
  async function exportSheet() {
    if (!sheet || !hasItems) return;
    // Un élément hors cadre est coupé sur le papier : on prévient avant de générer.
    if (outside.length && !(await ask({
      title: outside.length === 1 ? 'Un élément dépasse du cadre' : `${outside.length} éléments dépassent du cadre`,
      text: `${outside.map((o) => `${o.name} (feuille ${o.page})`).join(', ')}. Ils seront coupés au bord de la feuille : réduisez l’échelle, déplacez-les ou choisissez un format plus grand.`,
      confirmLabel: 'Exporter quand même', cancelLabel: 'Revenir à la planche',
    }))) return;
    // Texte sous 6 pt : illisible à l'impression, on prévient aussi.
    if (tiny.length && !(await ask({
      title: tiny.length === 1 ? 'Un synoptique a un texte sous 6 pt' : `${tiny.length} synoptiques ont un texte sous 6 pt`,
      text: `${tiny.map((o) => `${o.name} (feuille ${o.page}, ${ptTxt(o.pt)} pt)`).join(', ')}. Ce texte sera difficile à lire imprimé : agrandissez-les, répartissez-les sur plusieurs feuilles ou choisissez un format plus grand.`,
      confirmLabel: 'Exporter quand même', cancelLabel: 'Revenir à la planche',
    }))) return;
    exporting = true;
    try {
      exportShots = await shotsFor($state.snapshot(sheet) as Sheet, ctx);
      await new Promise((r) => setTimeout(r, 80));
      const svgs = [...exportHost.querySelectorAll('svg')] as SVGSVGElement[];
      const [W, H] = pageSize(sheet);
      if (sheet.format === 'pdf') {
        const { newDocMm } = await import('../../lib/pdfkit');
        const doc = await newDocMm(W, H);
        for (let i = 0; i < svgs.length; i++) {
          if (i > 0) doc.addPage([W, H], W > H ? 'landscape' : 'portrait');
          await doc.svg(svgs[i]!, { x: 0, y: 0, width: W, height: H });
        }
        download(doc.output('blob'), fileName(view.project.name, 'pdf'));
      } else {
        const type = sheet.format === 'png' ? 'image/png' : 'image/jpeg';
        for (let i = 0; i < svgs.length; i++) download(await raster(svgs[i]!, type), fileName(view.project.name, sheet.format, svgs.length > 1 ? i + 1 : undefined));
      }
      toast(`Planches exportées (${sheet.format.toUpperCase()}, ${svgs.length} feuille${svgs.length > 1 ? 's' : ''}).`);
    } catch (e) { fail(e); } finally { exporting = false; }
  }

  const ORIENT = [{ value: 'portrait', label: 'Portrait' }, { value: 'landscape', label: 'Paysage' }] as const;
  const FORMATS = [{ value: 'pdf', label: 'PDF', hint: 'Un document, une page par feuille' }, { value: 'png', label: 'PNG', hint: 'Une image par feuille' }, { value: 'jpg', label: 'JPG', hint: 'Une image par feuille, plus légère' }] as const;
  /** Le format choisi est retenu avec la planche (proposé en premier la fois suivante). */
  function exportAs(f: Sheet['format']) {
    if (sheet && sheet.format !== f) { sheet.format = f; if (writable) changed(); }
    exportSheet();
  }
  const FRAMES = [['quarter', '3/4'], ['front', 'Avant'], ['side', 'Côté'], ['rear', 'Arrière'], ['top', 'Dessus']] as const;
</script>

{#snippet busyTrig()}<span class="spin" aria-hidden="true"></span>Génération{/snippet}

{#if !sheet}<Loading label="Chargement des planches" />
{:else if !bays.length && !desks.length}
  <EmptyState icon="sheet" message="Aucun rack ni mobilier dans ce projet" hint="Ajoutez-en dans l’inventaire pour composer la planche.">
    {#snippet action()}<a class="btn primary" href="/projets/{pid}/inventaire"><Icon name="plus" size="sm" />Ajouter un élément</a>{/snippet}
  </EmptyState>
{:else}
  <div class="pl">
    <div class="pl-bar">
      <select class="input sm" aria-label="Format de la feuille" value={sheet.paper} disabled={!writable} onchange={(e) => { sheet!.paper = (e.target as HTMLSelectElement).value as Paper; changed(); }}>{#each Object.keys(PAPERS) as p}<option value={p}>{p}</option>{/each}</select>
      <Seg label="Orientation" bind:value={sheet.orientation} options={ORIENT.map((o) => ({ ...o, disabled: !writable }))} onchange={() => changed()} />
      <select class="input sm" aria-label="Échelle" value={sheet.scale} disabled={!writable} onchange={(e) => { sheet!.scale = Number((e.target as HTMLSelectElement).value); changed(); }}>{#each SCALES as s}<option value={s}>1:{s}</option>{/each}</select>
      {#if writable}<button class="btn sm ghost" aria-label="Disposer automatiquement" title="Placer tous les racks et le mobilier sur les feuilles, d’un coup" onclick={() => (relayout = true)}><Icon name="grid" size="sm" /><span class="pl-lbl">Disposer</span></button>{/if}
      <span class="pl-sep"></span>
      <Seg mode="tabs" label="Feuilles" bind:value={cur} options={sheet.pages.map((_, i) => ({ value: i, label: `Feuille ${i + 1}` }))} onchange={() => (selected = null)} />
      {#if writable}<button class="btn sm ghost" aria-label="Ajouter une feuille" title="Ajouter une feuille à la planche" onclick={addPage}><Icon name="plus" size="sm" /><span class="pl-lbl">Ajouter une feuille</span></button>{/if}
      {#if writable}
        <span class="pl-sep"></span>
        <button class="icon-btn sm" aria-label="Annuler (Ctrl+Z)" title="Annuler (Ctrl+Z)" disabled={!hist.undo} onclick={undo}><Icon name="undo" size="sm" /></button>
        <button class="icon-btn sm" aria-label="Rétablir (Ctrl+Maj+Z)" title="Rétablir (Ctrl+Maj+Z)" disabled={!hist.redo} onclick={redo}><span class="flip"><Icon name="undo" size="sm" /></span></button>
      {/if}
      <span class="pl-sep"></span>
      <div class="pl-zoom" role="group" aria-label="Zoom de la feuille">
        <button class="icon-btn sm" aria-label="Zoom arrière" title="Zoom arrière (Ctrl + molette)" disabled={zoom <= 0.25} onclick={() => zoomTo(zoom / 1.25)}><Icon name="zout" size="sm" /></button>
        <span class="mono small pl-zv" title="Taille à l’écran, par rapport à la feuille entière visible">{zoom === 1 ? 'Ajusté' : `${Math.round(zoom * 100)} %`}</span>
        <button class="icon-btn sm" aria-label="Zoom avant" title="Zoom avant (Ctrl + molette)" disabled={zoom >= 8} onclick={() => zoomTo(zoom * 1.25)}><Icon name="zin" size="sm" /></button>
        <button class="icon-btn sm" aria-label="Ajuster : voir la feuille entière" title="Ajuster : voir la feuille entière" disabled={zoom === 1} onclick={() => { zoom = 1; deskEl?.scrollTo(0, 0); }}><Icon name="fit" size="sm" /></button>
      </div>
      <!-- Bloc insécable aligné à droite : l'indicateur d'enregistrement a une largeur réservée, Exporter ne bouge pas. -->
      <div class="pl-out">
        {#if writable}<SaveState status={$saver} onretry={() => saver.flush()} />{/if}
        <span title={hasItems ? undefined : 'Planches vides : aucune feuille ne contient d’élément, composez les planches avant de les exporter.'}>
          <Menu label="Exporter" icon="download" btnClass="btn primary" menuLabel="Format d’export des planches" disabled={exporting || !hasItems} trigger={exporting ? busyTrig : undefined}>
            {#snippet children()}{#each FORMATS as f}<button role="menuitem" class="pl-exp" aria-current={sheet!.format === f.value ? 'true' : undefined} onclick={() => exportAs(f.value)}><b>{f.label}</b><small>{f.hint}</small></button>{/each}{/snippet}
          </Menu>
        </span>
      </div>
    </div>

    <div class="pl-body">
      <div class="pl-stage">
        <!-- Éléments hors cadre : signalés sur la feuille (la barre d'outils reste sur une ligne). -->
        {#if outside.length || tiny.length}<div class="pl-out-chip" role="status">
          {#if outside.length}<span class="chip warn" title="{outside.map((o) => `${o.name} (feuille ${o.page})`).join(', ')} : réduisez l’échelle, déplacez ou choisissez un format plus grand."><Icon name="alert" size="sm" />{outside.length} hors cadre</span>{/if}
          {#if tiny.length}<span class="chip warn" title="{tiny.map((o) => `${o.name} (feuille ${o.page}, ${ptTxt(o.pt)} pt)`).join(', ')} : texte difficile à lire imprimé, agrandissez ou choisissez un format plus grand."><Icon name="alert" size="sm" />{tiny.length === 1 ? 'Texte' : `${tiny.length} synoptiques : texte`} &lt; 6 pt</span>{/if}
        </div>{/if}
        <div class="pl-desk" role="presentation" bind:this={deskEl} bind:clientWidth={deskW} bind:clientHeight={deskH} onpointerdown={(e) => { if (!(e.target as Element).closest('.it, .rz, .pl-empty button')) selected = null; }}>
          <div class="pl-paper" bind:this={svgHost} style="width:{paperW}px;aspect-ratio:{pageSize(sheet)[0]} / {pageSize(sheet)[1]}">
            {#if page && !page.items.length}<div class="pl-empty"><b>Feuille vide</b><span>{writable ? 'Ajoutez des racks, du mobilier ou des synoptiques depuis le panneau de droite, ou placez tout d’un coup.' : 'Aucun élément sur cette feuille.'}</span>{#if writable}<button class="btn sm" onclick={() => (relayout = true)}><Icon name="grid" size="sm" />Disposer automatiquement</button>{/if}</div>{/if}
            {#if page}<SheetPage {sheet} {page} {ctx} {info} {shots} index={cur} total={sheet.pages.length} editable={writable} {selected} onpick={pick} onresize={startResize} onselect={(it) => (selected = it.id)} {nameOf} />{/if}
          </div>
        </div>
      </div>

      <aside class="pl-panel">
        {#if sel}
          <header class="pl-ph">
            <span class="pl-ic"><Icon name={sel.kind === 'rack' ? 'server' : sel.kind === 'desk' ? 'grid' : sel.kind === 'synoptic' ? 'syn' : sel.kind === 'location' ? 'pin' : 'plus'} size="sm" /></span>
            <span class="pl-pt"><b>{selName}</b><small>{KIND[sel.kind]}</small></span>
            <button class="icon-btn sm" aria-label="Désélectionner (Échap)" title="Désélectionner (Échap)" onclick={() => (selected = null)}><Icon name="x" size="sm" /></button>
          </header>

          {#if isDrawing(sel)}
            <section class="pl-sec">
              <div class="field"><label for="pl-gl">Libellé</label><input class="input" id="pl-gl" value={sel.label ?? ''} placeholder={sel.kind === 'synoptic' ? `Synoptique · ${selName}` : selName} maxlength="80" disabled={!writable} oninput={(e) => { sel.label = (e.target as HTMLInputElement).value || undefined; }} onchange={() => changed('label:' + sel.id)}></div>
              <label class="toggle-row"><input type="checkbox" checked={!sel.hideLabel} disabled={!writable} onchange={(e) => { sel.hideLabel = !(e.target as HTMLInputElement).checked || undefined; keepIn(sel); changed(); }}><span>Afficher le libellé</span></label>
              <div class="pl-row pl-scale">
                <fieldset class="pl-fs pl-zf" disabled={!writable} onchange={commitPct}>
                  <NumField id="pl-zs" label="Échelle" unit="%" min={zMin(sel) * 100} max={ZOOM_MAX * 100} bind:value={zPct} />
                </fieldset>
                {#if selPt != null}<span class="chip {selPt < MIN_PT - 0.05 ? 'warn' : 'ok'} pl-tp" role="status" title="Plus petit texte du synoptique (connecteur des ports), imprimé">{#if selPt < MIN_PT - 0.05}<Icon name="alert" size="sm" />{/if}Texte {ptTxt(selPt)} pt</span>{/if}
              </div>
              <input class="pl-range" type="range" min={Math.log(zMin(sel))} max={Math.log(ZOOM_MAX)} step="0.005" value={Math.log(zoomOf(sel))} disabled={!writable} aria-label="Échelle propre du dessin (curseur)" oninput={(e) => setZoom(sel, Math.exp(Number((e.target as HTMLInputElement).value)), false)} onchange={() => changed('zoom:' + sel.id)}>
              {#if selPt != null && selPt < MIN_PT - 0.05}<p class="pl-note small">Texte sous 6 pt : difficile à lire imprimé. Il atteint 6 pt à partir de {Math.ceil(SYN_LEGIBLE * 100)} % ; sinon, choisissez un format plus grand.</p>{/if}
              {#if writable}<div class="pl-acts">
                {#if zoomOf(sel) !== 1}<button class="btn sm ghost pl-reset" onclick={() => setZoom(sel, 1)}><Icon name="refresh" size="sm" />Revenir à 100 %</button>{/if}
                {#if sel.kind === 'synoptic' && ctx.syn?.[sel.container_id ?? '']}<button class="btn sm ghost" title="Échelle qui fait tenir le synoptique entier dans le cadre de la feuille" onclick={() => fitSel(sel)}><Icon name="fit" size="sm" />Ajuster à la feuille</button>{/if}
              </div>{/if}
              <span class="muted small">{sel.kind === 'synoptic' ? 'Poignées aux quatre coins de la feuille : proportions gardées, coin opposé fixe.' : `Échelle du dessin 1:${+scaleOf(sel, sheet.scale).toFixed(1)} (feuille 1:${sheet.scale}). Poignées aux quatre coins : proportions gardées.`}</span>
            </section>
          {/if}
          {#if isDrawing(sel) && tooBig(sel)}
            <p class="pl-warn small">Le dessin dépasse la feuille : réduisez-le ou choisissez un format plus grand.</p>
          {/if}
          {#if sel.kind === 'synoptic'}
            <section class="pl-sec">
              {#if ctx.syn?.[sel.container_id ?? ''] === null}<span class="muted small">Ce synoptique est vide : aucun port relié sur cet élément.</span>{/if}
              <a class="btn sm ghost" href="/projets/{pid}/synoptique/element/{sel.container_id}"><Icon name="syn" size="sm" />Ouvrir le synoptique</a>
            </section>
          {:else if sel.kind === 'rack' || sel.kind === 'desk'}
            {@const opts = sel.kind === 'rack' ? RACK_VIEWS : DESK_VIEWS}
            <section class="pl-sec">
              <span class="lbl-t">Vues affichées</span>
              <div class="pl-chips">{#each Object.entries(opts) as [v, l]}{@const on = viewsOf(sel).includes(v)}<button class="fchip" class:on disabled={!writable || (sel.kind === 'desk' && v === 'casq' && !ctx.desks[sel.container_id ?? '']?.casq.length)} aria-pressed={on} onclick={() => toggleView(sel, v)}>{#if on}<Icon name="check" size="sm" />{/if}{l}</button>{/each}</div>
              {#if sel.kind === 'rack'}
                <span class="lbl-t">Rendu</span>
                <Seg label="Rendu" full value={sel.render ?? 'real'} options={Object.entries(RACK_RENDERS).map(([k, l]) => ({ value: k as RackRender, label: l, disabled: !writable }))} onchange={(v) => { sel.render = v; changed(); }} />
              {/if}
            </section>
            {#if box3d}
              <section class="pl-sec">
                <span class="lbl-t">Vue 3D</span>
                <Sheet3DView bind:this={viewer} it={sel} {ctx} aspect={box3d.w / box3d.h} oncamera={(p) => { sel.cam = p; changed('cam:' + sel.id); }} />
                <span class="muted small">Glisser : tourner · molette : zoom · clic droit : décaler. L’angle est repris sur la feuille.</span>
                <span class="flabel">Cadrage</span>
                <div class="pl-acts" role="group" aria-label="Cadrage de la vue 3D">{#each FRAMES as [k, l]}<button class="btn sm ghost" disabled={!writable} onclick={() => viewer?.frame(k)}>{k === 'front' && sel.kind === 'desk' ? 'Opérateur' : l}</button>{/each}</div>
                {#if sel.kind === 'rack' && !/case/.test(ctx.racks[sel.container_id ?? '']?.c?.kind ?? '')}
                  <div class="pl-row"><span class="flabel pl-l">Portes</span><Seg label="Portes" value={sel.doors ?? 'removed'} options={[{ value: 'removed', label: 'Retirées', disabled: !writable }, { value: 'closed', label: 'Fermées', disabled: !writable }]} onchange={(v) => { sel.doors = v as 'removed' | 'closed'; changed(); }} /></div>
                {/if}
                <div class="pl-row"><span class="flabel pl-l">{sel.kind === 'desk' ? 'Joues' : 'Flancs'}</span><Seg label={sel.kind === 'desk' ? 'Joues' : 'Flancs'} value={sel.sides ?? true} options={[{ value: true, label: 'Avec', disabled: !writable }, { value: false, label: 'Sans', disabled: !writable }]} onchange={(v) => { sel.sides = v as boolean; changed(); }} /></div>
                <label class="pl-row"><span class="flabel pl-l">Cadre</span><input type="range" min="0.4" max="3" step="0.05" value={sel.r3d ?? (sel.kind === 'desk' ? 1.8 : 0.8)} disabled={!writable} oninput={(e) => { sel.r3d = Number((e.target as HTMLInputElement).value); keepIn(sel); }} onchange={() => changed('r3d:' + sel.id)}><span class="mono small">{(sel.r3d ?? (sel.kind === 'desk' ? 1.8 : 0.8)).toFixed(2).replace('.', ',')}</span></label>
              </section>
            {/if}
          {:else if sel.kind === 'reserved'}
            <section class="pl-sec">
              <div class="field"><label for="pl-rl">Libellé</label><input class="input" id="pl-rl" bind:value={sel.label} onchange={() => changed()} maxlength="60" disabled={!writable}></div>
              <fieldset class="pl-fs fg2" disabled={!writable} onchange={() => { keepIn(sel); changed(); }}>
                <NumField id="pl-rw" label="Largeur" unit="mm" min={10} max={1000} bind:value={() => sel.w ?? null, (v) => { if (v != null) sel.w = Math.min(1000, Math.max(10, v)); }} />
                <NumField id="pl-rh" label="Hauteur" unit="mm" min={10} max={1000} bind:value={() => sel.h ?? null, (v) => { if (v != null) sel.h = Math.min(1000, Math.max(10, v)); }} />
              </fieldset>
            </section>
          {:else}
            <section class="pl-sec">
              <fieldset class="pl-fs" disabled={!writable} onchange={() => { keepIn(sel); changed(); }}>
                <NumField id="pl-lw" label="Largeur du titre" unit="mm" min={30} max={1100} bind:value={() => sel.w ?? null, (v) => { if (v != null) sel.w = Math.min(1100, Math.max(30, v)); }} />
              </fieldset>
            </section>
          {/if}

          <section class="pl-sec">
            <span class="lbl-t">Position sur la feuille</span>
            <fieldset class="pl-fs fg2" disabled={!writable} onchange={commitPos}>
              <NumField id="pl-x" label="Depuis la gauche" unit="mm" bind:value={pos.x} />
              <NumField id="pl-y" label="Depuis le haut" unit="mm" bind:value={pos.y} />
            </fieldset>
            {#if writable}<span class="muted small">Flèches : 1 mm · Maj + flèches : 10 mm.</span>{/if}
          </section>

          {#if writable}
            <section class="pl-sec pl-move">
              <select class="input sm" aria-label="Envoyer sur une autre feuille" onchange={(e) => { const s = e.target as HTMLSelectElement; if (s.value !== '') toPage(Number(s.value)); s.value = ''; }}><option value="">Envoyer sur une autre feuille</option>{#each sheet.pages as _, i}{#if i !== cur}<option value={i}>Feuille {i + 1}</option>{/if}{/each}<option value={sheet.pages.length}>Nouvelle feuille</option></select>
              <button class="btn sm ghost danger" title="Retirer de la feuille (Suppr)" onclick={removeSel}><Icon name="trash" size="sm" />Retirer</button>
            </section>
          {/if}
        {:else}
          {#if writable}
            <section class="pl-sec">
              <span class="lbl-t">Racks</span>
              {#each bays as c (c.id)}
                {@const w = where(c.id)}
                <div class="pl-lib"><span class="pl-n"><b>{c.name}</b><small>{locName(c.location_id)?.code || locName(c.location_id)?.name || ''} · {c.height_u} U</small></span>{#if w.length}<span class="pl-ok" title="Déjà sur la feuille {w.join(', ')}">Feuille {w.join(', ')}</span>{/if}<button class="btn sm" class:ghost={w.length > 0} aria-label="{w.length ? 'Ajouter encore' : 'Ajouter'} {c.name}" onclick={() => add({ kind: 'rack', container_id: c.id, views: [...rackViews] })}><Icon name="plus" size="sm" />{w.length ? 'Ajouter encore' : 'Ajouter'}</button></div>
              {:else}<span class="muted small">Aucun rack.</span>{/each}
            </section>
            {#if synList.length}
              <section class="pl-sec">
                <span class="lbl-t">Synoptiques</span>
                {#each synList as c (c.id)}
                  {@const w = where(c.id, true)}
                  <div class="pl-lib"><span class="pl-n"><b>{c.name}</b><small>{locName(c.location_id)?.code || locName(c.location_id)?.name || ''} · synoptique</small></span>{#if w.length}<span class="pl-ok" title="Déjà sur la feuille {w.join(', ')}">Feuille {w.join(', ')}</span>{/if}<button class="btn sm" class:ghost={w.length > 0} aria-label="{w.length ? 'Ajouter encore' : 'Ajouter'} le synoptique {c.name}" onclick={() => addSyn(c.id)}><Icon name="plus" size="sm" />{w.length ? 'Ajouter encore' : 'Ajouter'}</button></div>
                {/each}
              </section>
            {/if}
            {#if desks.length}
              <section class="pl-sec">
                <span class="lbl-t">Mobilier</span>
                {#each desks as c (c.id)}
                  {@const w = where(c.id)}
                  <div class="pl-lib"><span class="pl-n"><b>{c.name}</b><small>{locName(c.location_id)?.code || locName(c.location_id)?.name || ''} · {ctx.desks[c.id]?.casq.length ?? 0} casquette{pl(ctx.desks[c.id]?.casq.length ?? 0)}</small></span>{#if w.length}<span class="pl-ok" title="Déjà sur la feuille {w.join(', ')}">Feuille {w.join(', ')}</span>{/if}<button class="btn sm" class:ghost={w.length > 0} aria-label="{w.length ? 'Ajouter encore' : 'Ajouter'} {c.name}" onclick={() => add({ kind: 'desk', container_id: c.id, views: [...deskViews] })}><Icon name="plus" size="sm" />{w.length ? 'Ajouter encore' : 'Ajouter'}</button></div>
                {/each}
              </section>
            {/if}
            <section class="pl-sec">
              <span class="lbl-t">Repères</span>
              <select class="input sm" aria-label="Ajouter un titre d’emplacement" title="Ajoute sur la feuille le nom d’un emplacement, en titre au-dessus de ses racks" onchange={(e) => { const s = e.target as HTMLSelectElement; if (s.value) add({ kind: 'location', location_id: s.value, w: 160 }); s.value = ''; }}><option value="">+ Titre d’emplacement</option>{#each view.locations as l}<option value={l.id}>{l.name}</option>{/each}</select>
              <button class="btn sm pl-start" title="Ajoute un cadre « Emplacement réservé » : la place d’un rack ou d’un meuble à venir" onclick={() => add({ kind: 'reserved', label: 'Emplacement réservé', w: 60, h: 80 })}><Icon name="plus" size="sm" />Emplacement réservé</button>
            </section>
          {/if}
          <section class="pl-sec">
            <span class="lbl-t">Cartouche</span>
            <label class="toggle-row"><input type="checkbox" bind:checked={sheet.titleBlock} disabled={!writable} onchange={() => changed()}><span>Imprimer le cartouche</span></label>
            {#if sheet.titleBlock}
              <div class="field"><label for="pl-t">Titre</label><input class="input" id="pl-t" bind:value={sheet.block.title} onchange={() => changed()} disabled={!writable} maxlength="80"></div>
              <div class="fg2">
                <div class="field"><label for="pl-ref">Référence</label><input class="input mono" id="pl-ref" bind:value={sheet.block.reference} onchange={() => changed()} disabled={!writable} maxlength="40"></div>
                <div class="field"><label for="pl-v">Version</label><input class="input mono" id="pl-v" bind:value={sheet.block.version} onchange={() => changed()} disabled={!writable} maxlength="10"></div>
                <div class="field"><label for="pl-a">Auteur</label><input class="input" id="pl-a" bind:value={sheet.block.author} onchange={() => changed()} disabled={!writable} maxlength="60"></div>
                <div class="field"><label for="pl-c">Vérificateur</label><input class="input" id="pl-c" bind:value={sheet.block.checker} onchange={() => changed()} disabled={!writable} maxlength="60"></div>
              </div>
              <div class="field"><label for="pl-n">Normes</label><input class="input" id="pl-n" bind:value={sheet.block.standards} onchange={() => changed()} disabled={!writable} maxlength="80"></div>
              <div class="field"><label for="pl-no">Notes</label><input class="input" id="pl-no" bind:value={sheet.block.notes} onchange={() => changed()} disabled={!writable} maxlength="120"></div>
              <span class="muted small">Site, projet, client, lieu, date, numéro de feuille et échelle sont remplis automatiquement.</span>
            {/if}
          </section>
          {#if writable && sheet.pages.length > 1}
            <section class="pl-sec">
              <span class="lbl-t">Feuille {cur + 1} sur {sheet.pages.length}</span>
              <button class="btn sm ghost danger pl-start" onclick={dropPage}><Icon name="trash" size="sm" />Supprimer la feuille {cur + 1}</button>
            </section>
          {/if}
        {/if}
      </aside>
    </div>
  </div>

  <div class="pl-export" bind:this={exportHost} aria-hidden="true">
    {#if exporting}{#each sheet.pages as p, i (p.id)}<SheetPage {sheet} page={p} {ctx} {info} shots={exportShots} index={i} total={sheet.pages.length} />{/each}{/if}
  </div>
{/if}

{#if relayout}
  <Confirm title="Disposer automatiquement ?" confirmLabel="Disposer" onconfirm={doLayout} onclose={() => (relayout = false)}>
    <p>Racks puis mobilier, emplacement par emplacement, de gauche à droite, sur autant de feuilles que nécessaire. La composition actuelle est remplacée (Ctrl+Z pour revenir en arrière).</p>
    <p class="lbl-t" style="margin:10px 0 4px">Vues des racks</p>
    <div class="pl-chips" role="group" aria-label="Vues des racks">{#each Object.entries(RACK_VIEWS) as [v, l]}{@const on = rackViews.includes(v)}<button type="button" class="fchip" class:on aria-pressed={on} onclick={() => (rackViews = on ? rackViews.filter((x) => x !== v) : Object.keys(RACK_VIEWS).filter((x) => x === v || rackViews.includes(x)))}>{#if on}<Icon name="check" size="sm" />{/if}{l}</button>{/each}</div>
    {#if desks.length}
      <p class="lbl-t" style="margin:10px 0 4px">Vues du mobilier</p>
      <div class="pl-chips" role="group" aria-label="Vues du mobilier">{#each Object.entries(DESK_VIEWS) as [v, l]}{@const on = deskViews.includes(v)}<button type="button" class="fchip" class:on aria-pressed={on} onclick={() => (deskViews = on ? deskViews.filter((x) => x !== v) : Object.keys(DESK_VIEWS).filter((x) => x === v || deskViews.includes(x)))}>{#if on}<Icon name="check" size="sm" />{/if}{l}</button>{/each}</div>
    {/if}
  </Confirm>
{/if}

<style>
  .pl { display: grid; gap: 10px; }
  /* Barre étroite : « Ajouter une feuille » et « Disposer » cèdent la place à leur icône (nom accessible et infobulle restent) : la barre tient sur une ligne à 1440 px. */
  @media (max-width: 1720px) { .pl-lbl { display: none; } }
  .pl-bar { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; padding: 8px 10px; border-radius: 12px; background: var(--surface); box-shadow: inset 0 0 0 1px var(--line); }
  .pl-bar select.input.sm { width: auto; }
  .pl-sep { width: 1px; height: 22px; background: var(--line-2); margin: 0 4px; }
  .pl-out { flex: none; display: flex; align-items: center; gap: 8px; margin-left: auto; white-space: nowrap; }
  .flip { display: inline-flex; transform: scaleX(-1); }
  .pl-exp { display: flex; flex-direction: column; align-items: flex-start; gap: 1px; }
  .pl-exp b { font-weight: 600; }
  .pl-exp small { font-size: var(--fs-xs); color: var(--fg-3); }
  .pl-exp[aria-current] b::after { content: " · dernier choix"; font-weight: 400; color: var(--fg-3); }
  .pl-body { display: grid; grid-template-columns: minmax(0, 1fr) 360px; gap: 12px; align-items: start; }
  .pl-stage { position: relative; min-width: 0; }
  .pl-out-chip { position: absolute; top: 10px; left: 10px; z-index: 2; display: flex; flex-wrap: wrap; gap: 6px; }
  .pl-out-chip .chip { box-shadow: var(--card-sh); }
  .pl-scale { align-items: flex-end; }
  .pl-zf { flex: 0 1 140px; min-width: 0; }
  .pl-tp { margin-bottom: 6px; }
  .pl-range { width: 100%; }
  .pl-note { margin: 0; color: var(--warn); }
  .pl-desk { padding: 28px; border-radius: 14px; background: radial-gradient(90% 70% at 50% 0%, color-mix(in srgb, var(--fg) 6%, var(--surface-2)), var(--surface-2)); overflow: auto; height: calc(100dvh - 242px); min-height: 420px; }
  .pl-paper { position: relative; flex: none; margin: 0 auto; background: #fff; box-shadow: 0 24px 50px -22px rgba(0, 0, 0, .55), 0 0 0 1px rgba(0, 0, 0, .08); touch-action: none; }
  .pl-zoom { display: flex; align-items: center; gap: 2px; }
  .pl-zv { min-width: 52px; text-align: center; }
  .pl-warn { margin: 0; padding: 10px 14px; color: var(--warn); background: var(--warn-soft); border-bottom: 1px solid var(--line); }
  /* Feuille vide : papier blanc fixe, textes aux gris du papier. */
  .pl-empty { position: absolute; left: 50%; top: 42%; transform: translate(-50%, -50%); display: grid; gap: 6px; justify-items: center; text-align: center; max-width: 340px; color: #5B636D; font-size: 13px; pointer-events: none; z-index: 1; }
  .pl-empty b { color: #15171A; font-size: 15px; }
  .pl-empty .btn { pointer-events: auto; margin-top: 6px; }
  /* En-tête de projet (~112 px avec la description), barre d’outils et marges : feuille et panneau remplissent la hauteur sans défilement de page. */
  .pl-panel { position: sticky; top: 12px; max-height: calc(100dvh - 242px); overflow: auto; border-radius: 14px; background: var(--surface); box-shadow: inset 0 0 0 1px var(--line); }
  .pl-ph { display: flex; align-items: center; gap: 10px; padding: 12px 12px 12px 14px; border-bottom: 1px solid var(--line); position: sticky; top: 0; background: var(--surface); z-index: 1; }
  .pl-ic { width: 32px; height: 32px; display: grid; place-items: center; border-radius: 8px; background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--fg); flex: none; }
  .pl-pt { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.2; }
  .pl-pt b { font-size: 15px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pl-pt small { font-size: 12px; color: var(--fg-3); }
  .pl-sec { padding: 14px; display: grid; gap: 10px; border-bottom: 1px solid var(--line); }
  .pl-sec:last-child { border-bottom: 0; }
  .pl-fs { margin: 0; padding: 0; border: 0; min-width: 0; }
  .pl-move { grid-template-columns: minmax(0, 1fr) auto; align-items: center; }
  .pl-start, .pl-reset { justify-self: start; }
  .pl-chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .pl-acts { display: flex; flex-wrap: wrap; gap: 4px; }
  .pl-row { display: flex; align-items: center; gap: 10px; }
  .pl-row input[type='range'] { flex: 1; min-width: 0; }
  .pl-l { width: 58px; flex: none; }
  .pl-lib { display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 10px; border-radius: 9px; background: var(--surface-2); }
  .pl-n { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.2; }
  .pl-n b { font-size: 13.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pl-n small { font-size: 11.5px; color: var(--fg-3); }
  .pl-ok { font: 600 11px var(--mono); color: var(--ok); }
  .pl-export { position: fixed; left: -100000px; top: 0; width: 1200px; pointer-events: none; }
  /* Écran étroit : une seule colonne qui ne dépasse jamais la largeur (la feuille s'ajuste), panneau sous la feuille. */
  @media (max-width: 1100px) {
    .pl-body { grid-template-columns: minmax(0, 1fr); }
    .pl-panel { position: static; max-height: none; }
    .pl-desk { height: min(70vh, 720px); min-height: 320px; }
  }
  @media (max-width: 600px) {
    .pl-desk { padding: 12px; }
    .pl-out { flex-wrap: wrap; white-space: normal; margin-left: 0; }
  }
</style>
