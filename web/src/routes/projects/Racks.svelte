<script lang="ts">
  // Élévations de racks (section 9) : écran de conception. Une baie (avec prise de main), ou toutes
  // les baies côte à côte (`/racks/tout`, main prise le temps de chaque geste) : même scène, mêmes
  // panneaux, mêmes gestes. Vues cumulables (avant, arrière, côté) à la même échelle et alignées
  // sur les U ; positions au demi-U ; zoom (Ctrl+molette, pincement, Ctrl+= / Ctrl+- / Ctrl+0) ;
  // menu contextuel (clic droit, touche Menu, Maj+F10). Un équipement glissé d'une baie à une autre
  // change d'élément dans l'inventaire (un exemplaire à la fois) et y est posé à l'endroit lâché.
  import { untrack, onDestroy, tick } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Vu from '../../components/Vu.svelte';
  import Seg from '../../components/Seg.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Loading from '../../components/Loading.svelte';
  import ContextMenu, { type CtxItem } from '../../components/ContextMenu.svelte';
  import { api } from '../../lib/api';
  import { ask } from '../../lib/ask.svelte';
  import { canvasKeys } from '../../lib/canvaskeys';
  import { can } from '../../lib/session.svelte';
  import { toast, toasts, dismiss, fail } from '../../lib/toast.svelte';
  import { route, navigate } from '../../lib/router.svelte';
  import { join } from '../../lib/live.svelte';
  import { nf, relative, pl } from '../../lib/format';
  import { refById, loadRefs, familyColor } from '../../lib/catalog.svelte';
  import { rackLines, rackText, setRackText, modelOf, RACK_TEXT, type RackText } from '../../lib/racktext.svelte';
  import { ALU, decor, accFace, visualOf } from '../../lib/faceplate';
  import { type Accessory, type Article, type Instance, type ProjectView } from '../../lib/projects';
  import PanelFace from '../../components/PanelFace.svelte';
  import ProjectPanel from './ProjectPanel.svelte';
  import Rack3D from '../../components/Rack3D.svelte';
  import RackPicker from '../../components/RackPicker.svelte';
  import AddArticle from './AddArticle.svelte';
  import Inspector from '../../components/Inspector.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import { ApiError } from '../../lib/api';
  import { blankClick } from '../../lib/blankclick';
  import { isWhole, rangeTxt, occTxt, aimTxt, fmtU, parseU, sideGeom, FRONT_RAIL_MM } from '../../lib/rackgeom';
  import { ZMIN, ZMAX, zoomStep, readZoom, saveZoom } from '../../lib/rackzoom';
  import { textWidth } from '../../lib/fit';

  // `fixed` : éditeur d'un seul élément, intégré à une autre page (casquette d'un mobilier).
  let { view, fixed = null, reload }: { view: ProjectView; fixed?: string | null; reload?: () => Promise<void> } = $props();
  type Face = 'front' | 'rear' | 'rear_inner';
  /** Vue affichée : une face, ou le côté (profondeurs). */
  type View = Face | 'side';
  type W = 'full' | 'half' | 'third' | 'quarter';
  interface Placement { id: string; article_id: string; instance_id: string | null; face: Face; u_start: number; width: W; col: number; circuit_id: string | null; shelf_id: string | null; rev: number }
  /** `cid` : baie de l'élément (ajouté au chargement). */
  interface RackEl { cid: string; placement: Placement; label: string; tag: string | null; custom?: string | null; nature: string; height_u: number; depth_mm: number | null; rear_reserve_mm: number | null; weight_kg: number | null; power_w: number | null; function: string | null; category: string | null; brand: string | null; accessory: Accessory | null; article_rev: number; type_id?: string | null; visual?: string | null; item_id?: string | null }
  interface Pending { cid: string; article_id: string; article_rev?: number; instance_id?: string; tag?: string; label: string; nature: string; height_u: number | null; width: W | null; rackable: boolean; function: string | null; accessory: Accessory | null; type_id?: string | null; visual?: string | null }
  interface VPdu { article_id: string; rev: number; label: string; accessory: { kind: 'vertical_pdu'; length_mm: number; outlets: number; side: 'left' | 'right'; circuit_id?: string | null }; circuit: string | null; fed: number; free: number; over: boolean }
  interface Circuit { id: string; name: string; phase: 'L1' | 'L2' | 'L3'; rating_a: number; rev: number }
  interface Rack {
    container: any; elements: RackEl[]; pending: Pending[]; circuits: Circuit[]; faces: Face[]; vpdus?: VPdu[];
    electrical: { circuits: { id: string; name: string; phase: string; rating_a: number; watts: number; amps: number; over: boolean }[]; phases: { phase: string; watts: number; amps: number }[]; imbalance_pct: number; unbalanced: boolean; unassigned_watts: number };
    totals: { used_u: number; total_u: number | null; weight_kg: number; power_w: number; over_weight: boolean; over_power: boolean; over_u: boolean };
    warnings: { placement_id: string; message: string }[];
    lock: { account_id: string; account_name: string; taken_at: string; mine: boolean } | null;
  }

  loadRefs();
  // Les casquettes s'équipent depuis le mobilier : elles ne figurent pas dans les racks.
  const racks = $derived(view.containers.filter((c) => c.rackable && c.kind !== 'casquette'));
  const all = $derived(!fixed && route.path.split('/')[4] === 'tout');
  /** Vue côte à côte : emplacement choisi (dans l'adresse), sinon tous. */
  const loc = $derived(all ? (route.path.split('/')[5] ?? null) : null);
  const cid = $derived(fixed ?? (all ? null : (route.path.split('/')[4] ?? racks[0]?.id ?? null)));
  $effect(() => { if (!fixed && !route.path.split('/')[4] && racks[0]) navigate(`/projets/${view.project.id}/racks/${racks[0].id}`, true); });
  /** Baies côte à côte : rangées par emplacement (puis celles sans emplacement). */
  const allList = $derived.by(() => {
    const out = view.locations.flatMap((l) => (loc && l.id !== loc ? [] : racks.filter((c) => c.location_id === l.id)));
    if (!loc) out.push(...racks.filter((c) => !view.locations.some((l) => l.id === c.location_id)));
    return out;
  });
  const withRacks = $derived(view.locations.filter((l) => racks.some((c) => c.location_id === l.id)));
  const ids = $derived<string[]>(all ? allList.map((c) => c.id) : cid ? [cid] : []);
  const idsKey = $derived(ids.join(','));
  const contOf = (id: string | null | undefined) => (id ? (view.containers.find((c) => c.id === id) ?? null) : null);

  /** Élévations chargées, par baie. */
  let store = $state<Record<string, Rack>>({});
  let loaded = $state(false);
  /** Vue d'une baie : son élévation. */
  const rack = $derived(!all && cid ? (store[cid] ?? null) : null);
  /** Baies affichées (une, ou toutes côte à côte). */
  const shownRacks = $derived(ids.map((id) => store[id]).filter((r): r is Rack => !!r));
  const rid = (R: Rack): string => R.container.id;
  let mode = $state<'real' | 'schematic'>('real');
  let show3d = $state(false);
  let armed = $state<Pending | null>(null);
  let sel = $state<string | null>(null);
  let newer = $state(false);
  let requested = $state<string | null>(null);
  // « Ajouter du matériel » : la boîte de l'inventaire, réglée sur la baie choisie.
  let adding = $state(false);
  let addCid = $state<string | null>(null);
  const canAdd = $derived(can('projets', 'ecriture'));
  const shownC = $derived(contOf(cid));
  const addC = $derived(contOf(addCid));
  const canDesign = $derived(can('conception', 'ecriture'));
  /** Vue d'une baie : la main est à soi. */
  const mine = $derived(!!rack?.lock?.mine);
  /** Modifiable ? Vue d'une baie : main prise ; côte à côte : rôle de conception et personne d'autre n'a la main. */
  function canEdit(id: string | null | undefined): boolean {
    const R = id ? store[id] : null;
    if (!R) return false;
    return all ? canDesign && (!R.lock || R.lock.mine) : !!R.lock?.mine;
  }
  const anyEdit = $derived(all ? shownRacks.some((R) => canEdit(rid(R))) : mine);
  // Proportions réelles d'un 19" : façade 482,6 mm pour 44,45 mm par U (≈ 10,86).
  const U = 34;
  const BW = Math.round(U * 10.86) - 22;
  const unitsOf = (R: Rack | null | undefined): number => R?.container.height_u ?? 0;

  // ----- Vues cumulables : avant, arrière (intérieure), côté ; au moins une, choix mémorisé (par écran) -----
  const VIEW_ORDER: View[] = ['front', 'rear', 'rear_inner', 'side'];
  const viewsKey = () => (all ? 'rack-views-all' : 'rack-views');
  const readViews = (key: string): View[] => {
    try { const v = JSON.parse(localStorage.getItem(key) ?? 'null'); if (Array.isArray(v)) { const ok = v.filter((x): x is View => VIEW_ORDER.includes(x)); if (ok.length) return ok; } } catch { /* stockage indisponible */ }
    return ['front'];
  };
  let views = $state<View[]>(readViews(untrack(viewsKey)));
  function setViews(v: View[]) { views = v; try { localStorage.setItem(viewsKey(), JSON.stringify(v)); } catch { /* stockage indisponible */ } }
  /** Vues affichées pour une baie (ses faces, plus le côté). */
  function shownOf(R: Rack | null | undefined): View[] {
    const av: View[] = [...(R?.faces ?? ['front', 'rear']), 'side'];
    const v = VIEW_ORDER.filter((x) => views.includes(x) && av.includes(x));
    return v.length ? v : ['front'];
  }
  /** Faces proposées dans la barre : celles de la baie, ou de l'une des baies côte à côte. */
  const barFaces = $derived<Face[]>(all ? (['front', 'rear', 'rear_inner'] as Face[]).filter((f) => shownRacks.some((R) => R.faces.includes(f))) : (rack?.faces ?? ['front', 'rear']));
  const shownT = $derived.by((): View[] => {
    const av: View[] = [...barFaces, 'side'];
    const v = VIEW_ORDER.filter((x) => views.includes(x) && av.includes(x));
    return v.length ? v : ['front'];
  });
  /** Face des poses quand la vue ne la dit pas (vue de côté seule) : la première face affichée. */
  const mainFace = (R: Rack | null | undefined): Face => (shownOf(R).find((v): v is Face => v !== 'side') ?? 'front');
  function toggleView(v: View) {
    if (shownT.includes(v)) { if (shownT.length > 1) setViews(shownT.filter((x) => x !== v)); }
    else setViews(VIEW_ORDER.filter((x) => x === v || shownT.includes(x)));
    aim = null;
  }
  /** Montre une face (ajoutée aux vues affichées) : un équipement déplacé ou désigné reste visible. */
  function reveal(f: Face) { if (!shownT.includes(f)) setViews(VIEW_ORDER.filter((x) => x === f || shownT.includes(x))); }

  // Téléphone : la baie est mise à l'échelle de la largeur disponible (pas de défilement
  // horizontal de la page) ; la liste « À placer » devient un panneau repliable fixé en bas.
  let narrow = $state(false);
  $effect(() => {
    const mq = window.matchMedia('(max-width: 767px)');
    const on = () => (narrow = mq.matches); on();
    mq.addEventListener('change', on);
    return () => mq.removeEventListener('change', on);
  });
  let stageW = $state(0), stageH = $state(0);
  let scroller = $state<HTMLElement>();
  let sheetOpen = $state(false);
  // Vue de côté à la même échelle que les façades : 34 px pour 44,45 mm.
  const PX_MM = U / 44.45;
  const depthOf = (R: Rack | null | undefined): number => R?.container.depth_mm ?? 800;
  const swOf = (R: Rack) => Math.round(depthOf(R) * PX_MM);
  /** Largeur naturelle (zoom 1) d'une vue, graduations comprises (CSS : .rack, .rack-scale, .side-body). */
  function viewW(R: Rack, v: View, i: number, n: number): number {
    const scales = (i === 0 ? 28 : 0) + (i === n - 1 ? 28 : 0);
    if (v === 'side') return scales + 10 + swOf(R) + 10;
    const vp = (vpOf(R, v).some((x) => x.accessory.side === 'left') ? 26 : 0) + (vpOf(R, v).some((x) => x.accessory.side === 'right') ? 26 : 0);
    return scales + 10 + BW + 22 + vp;
  }
  const natW = (R: Rack) => { const s = shownOf(R); return s.reduce((n, v, i) => n + viewW(R, v, i, s.length), 0) + (s.length - 1) * 14; };
  /** Hauteur naturelle : légende de vue (22 + 6), cadre (22 + 24), socle (9), cote de profondeur. */
  const natH = (R: Rack) => unitsOf(R) * U + 22 + 6 + 46 + 9 + (shownOf(R).includes('side') ? 30 : 0);
  /** Côte à côte : écart entre baies ; en-tête de baie (voir .all-racks, .ar-head), à taille fixe quel que soit le zoom. */
  const AR_GAP = 28, AR_HEAD = 38;
  const natural = $derived.by(() => {
    if (all) return { w: shownRacks.reduce((n, R) => n + natW(R), 0) + Math.max(0, shownRacks.length - 1) * AR_GAP, h: Math.max(0, ...shownRacks.map(natH)) };
    return rack ? { w: natW(rack), h: natH(rack) } : { w: 0, h: 0 };
  });

  // ----- Zoom : « Ajuster » (toutes les vues dans la place disponible) ou paliers de 25 à 400 %, mémorisé par écran -----
  const zKey = () => (all ? 'rack-zoom-all' : fixed ? 'rack-zoom-equip' : 'rack-zoom');
  let zset = $state<number | 'fit'>(readZoom(untrack(zKey)));
  /** Marges de la scène (voir .rk-stage) : 12 px autour, 56 px sous les vues pour le contrôle de zoom. */
  const PAD_W = 24, PAD_H = 68;
  const fitZ = $derived.by(() => {
    if (!stageW || !natural.w || !natural.h) return 1;
    const fw = (stageW - PAD_W) / natural.w, fh = (stageH - PAD_H - (all ? AR_HEAD : 0)) / natural.h;
    if (narrow) return Math.max(0.15, Math.min(1, fw));
    // « Ajuster » montre tout : jusqu'à 30 % pour une baie (comme avant), plus petit côte à côte.
    return Math.max(all ? 0.1 : 0.3, Math.min(1, fw, fh));
  });
  const zoom = $derived(zset === 'fit' ? fitZ : zset);
  const fitOn = $derived(zset === 'fit');
  let zAnchor: { fx: number; fy: number; x: number; y: number } | null = null;
  /** Change le zoom en gardant fixe le point visé (pointeur, sinon le centre de la scène). */
  function setZoom(z: number | 'fit', at?: { x: number; y: number }) {
    const s = scroller, content = s?.querySelector<HTMLElement>('.rk-zoomed');
    if (s && content && z !== 'fit') {
      const sr = s.getBoundingClientRect(), r = content.getBoundingClientRect();
      const x = at?.x ?? sr.left + sr.width / 2, y = at?.y ?? sr.top + sr.height / 2;
      zAnchor = { fx: (x - r.left) / (r.width || 1), fy: (y - r.top) / (r.height || 1), x, y };
    } else zAnchor = null;
    zset = z === 'fit' ? 'fit' : Math.min(ZMAX, Math.max(ZMIN, Math.round(z * 100) / 100));
    saveZoom(zKey(), zset);
    tick().then(() => {
      const a = zAnchor, s2 = scroller, c2 = s2?.querySelector<HTMLElement>('.rk-zoomed');
      zAnchor = null;
      if (!a || !s2 || !c2) return;
      const r = c2.getBoundingClientRect();
      s2.scrollLeft += r.left + a.fx * r.width - a.x;
      s2.scrollTop += r.top + a.fy * r.height - a.y;
    });
  }
  /** Palier suivant ou précédent ; sous 25 % (« Ajuster » côte à côte), seul le zoom avant agit. */
  function zoomBy(dir: 1 | -1) { const z = zoomStep(zoom, dir); if ((z - zoom) * dir > 1e-9) setZoom(z); }
  // Ctrl+molette (et pincement sur pavé tactile, vu comme Ctrl+molette) : zoom autour du pointeur ;
  // molette seule : défilement de la scène. Les crans d'une même image sont cumulés.
  let wheelAcc: { f: number; x: number; y: number } | null = null;
  function onWheel(e: WheelEvent) {
    if (!(e.ctrlKey || e.metaKey) || show3d) return;
    e.preventDefault();
    const d = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaMode === 2 ? e.deltaY * 400 : e.deltaY;
    if (!wheelAcc) {
      wheelAcc = { f: 1, x: e.clientX, y: e.clientY };
      requestAnimationFrame(() => { const w = wheelAcc; wheelAcc = null; if (w && !(w.f < 1 && zoom <= ZMIN + 1e-9)) setZoom(Math.max(Math.min(zoom, ZMIN), zoom * w.f), w); });
    }
    wheelAcc.f *= Math.exp(-Math.max(-60, Math.min(60, d)) * 0.006);
    wheelAcc.x = e.clientX; wheelAcc.y = e.clientY;
  }
  $effect(() => {
    const s = scroller;
    if (!s) return;
    s.addEventListener('wheel', onWheel, { passive: false });
    return () => s.removeEventListener('wheel', onWheel);
  });
  /** Fait défiler la scène (jamais la page) pour montrer un élément. */
  function showInStage(el: Element | null) {
    const s = scroller;
    if (!s || !el) return;
    const r = el.getBoundingClientRect(), sr = s.getBoundingClientRect();
    s.scrollTo({ top: s.scrollTop + r.top - sr.top - (sr.height - r.height) / 2, left: s.scrollLeft + r.left - sr.left - (sr.width - r.width) / 2, behavior: 'smooth' });
  }
  const cols = (w: W) => ({ full: 1, half: 2, third: 3, quarter: 4 })[w] ?? 1;

  /** Recharge des baies (par défaut : celles affichées). L'élément armé ou déplacé suit le rechargement. */
  let scopeNow = '';
  async function load(which: string[] = ids) {
    const scope = scopeNow;
    await Promise.all([...new Set(which)].map(async (id) => {
      try {
        const r = await api.get<Rack>(`/containers/${id}/rack`);
        if (scope !== scopeNow) return;
        for (const e of r.elements) e.cid = id;
        for (const p of r.pending) p.cid = id;
        store[id] = r;
      } catch (e) { fail(e); }
    }));
    if (scope !== scopeNow) return;
    if (!all) newer = false;
    loaded = true;
    if (armed) { const k = pk(armed), c = armed.cid; armed = store[c]?.pending.find((x) => pk(x) === k) ?? null; }
    if (moving) { const id = moving.placement.id; moving = elById(id); }
  }
  const pk = (p: { instance_id?: string | null; article_id: string }) => p.instance_id ?? p.article_id;
  const allEls = $derived(shownRacks.flatMap((R) => R.elements));
  const elById = (id: string): RackEl | null => { for (const R of Object.values(store)) { const e = R.elements.find((x) => x.placement.id === id); if (e) return e; } return null; };

  // Verrou (vue d'une baie) : prise de main à l'ouverture, battement toutes les 20 s, libération à
  // la sortie ou après 10 minutes sans action. Côte à côte : la main est prise le temps du geste (act).
  let beat: ReturnType<typeof setInterval> | null = null;
  let lastAction = Date.now();
  async function take() {
    if (!cid || all || !canDesign) return;
    try {
      const r = await api.post<{ mine: boolean; account_name: string }>(`/containers/${cid}/rack/lock`);
      if (r.mine && !beat) beat = setInterval(heartbeat, 20_000);
      await load();
    } catch (e) { fail(e); }
  }
  async function heartbeat() {
    if (!cid) return;
    if (Date.now() - lastAction > 10 * 60_000) { await release(); toast('Main cédée après 10 minutes sans action.', 'info'); return; }
    try { const r = await api.post<{ mine: boolean }>(`/containers/${cid}/rack/lock`); if (!r.mine) { stopBeat(); await load(); } } catch { /* réseau : on réessaie au prochain battement */ }
  }
  function stopBeat() { if (beat) { clearInterval(beat); beat = null; } }
  // Page quittée : plus aucune lecture de l'état dérivé (cid…) après la destruction (avertissement derived_inert).
  let destroyed = false;
  async function release(id = cid) {
    stopBeat();
    if (!id) return;
    try { await api.del(`/containers/${id}/rack/lock`); } catch { /* déjà libre */ }
    if (!destroyed && !all && id === cid) await load([id]);
  }
  async function requestHand() {
    try { await api.post(`/containers/${cid}/rack/lock/request`); toast(`Demande envoyée à ${rack?.lock?.account_name}.`, 'info'); } catch (e) { fail(e); }
  }
  /** Exécute une modification. Côte à côte : prend la main sur les baies du geste (un geste à la
   *  fois), exécute, puis rend la main. Vue d'une baie : la main est déjà prise. */
  let chain: Promise<unknown> = Promise.resolve();
  let busyN = $state(0);
  function act<T>(cids: string[], run: () => Promise<T>): Promise<T> {
    lastAction = Date.now();
    if (!all) return run();
    const go = async () => {
      busyN++;
      const held: string[] = [];
      try {
        for (const id of new Set(cids)) {
          if (store[id]?.lock?.mine) continue;
          const r = await api.post<{ mine: boolean; account_name: string }>(`/containers/${id}/rack/lock`);
          if (!r.mine) throw new ApiError(409, 'locked', `${r.account_name} modifie « ${contOf(id)?.name ?? 'ce rack'} » : réessayez plus tard.`);
          held.push(id);
        }
        return await run();
      } finally {
        for (const id of held) { try { await api.del(`/containers/${id}/rack/lock`); } catch { /* déjà libre */ } }
        busyN--;
      }
    };
    const p = chain.then(go, go);
    chain = p.catch(() => undefined);
    return p;
  }

  let current: string | null = null;
  let lastMode: boolean | null = null;
  // Changement de baie ou d'écran : tout repart de zéro (sélection, pose, menu) ; vues et zoom de l'écran.
  $effect(() => {
    const key = all ? `all:${loc ?? ''}` : (cid ?? '');
    if (!key) return;
    untrack(() => {
      const single = all ? null : cid;
      if (current && current !== single) release(current);
      current = single;
      if (lastMode !== null && lastMode !== all) { views = readViews(viewsKey()); zset = readZoom(zKey()); }
      lastMode = all;
      scopeNow = key; store = {}; loaded = false;
      sel = null; armed = null; moving = null; requested = null; ctx = null; show3d = all ? false : show3d;
      load().then(() => { if (single) take(); });
    });
  });
  // Côte à côte : une baie ajoutée à la liste est chargée.
  $effect(() => { void idsKey; untrack(() => { if (all && loaded) { const miss = ids.filter((id) => !store[id]); if (miss.length) load(miss); } }); });
  const soon: Record<string, ReturnType<typeof setTimeout>> = {};
  $effect(() => join(`project:${view.project.id}`, (c) => {
    if (all) {
      // Côte à côte : une baie modifiée ou dont la main change est relue (regroupé sur 250 ms).
      if (!ids.includes(c.id) || (c.kind !== 'rack' && c.kind !== 'lock')) return;
      clearTimeout(soon[c.id]); soon[c.id] = setTimeout(() => { if (!destroyed && all) load([c.id]); }, 250);
      return;
    }
    if (c.id !== cid) return;
    if (c.kind === 'rack' && c.action === 'saved' && !mine) newer = true;
    if (c.kind === 'lock' && c.action === 'request' && mine) requested = c.actor;
    // Main prise ailleurs (y compris par soi-même dans un autre onglet) : cet onglet passe en lecture.
    if (c.kind === 'lock' && c.action === 'taken') { if (mine) stopBeat(); load(); }
    if (c.kind === 'lock' && c.action === 'released' && !mine) load();
  }));
  const leave = () => { if (current && beat) fetch(`/api/containers/${current}/rack/lock`, { method: 'DELETE', headers: { 'X-Strata-Request': '1' }, keepalive: true }); };
  onDestroy(() => { destroyed = true; stopBeat(); for (const t of Object.values(soon)) clearTimeout(t); if (current) release(current); });

  // ?i=<exemplaire> (lien « Poser » de l'inventaire) : l'exemplaire posé est sélectionné et montré,
  // sinon il est armé dans « À placer », prêt à poser.
  let handledI: string | null = null;
  $effect(() => {
    const iid = route.query.get('i'), ok = loaded, at = all ? `all:${loc ?? ''}` : cid;
    if (!iid || !ok || !at || handledI === `${at}:${iid}`) return;
    untrack(() => {
      handledI = `${at}:${iid}`;
      const el = allEls.find((e) => e.placement.instance_id === iid);
      if (el) {
        armed = null; sel = el.placement.id;
        if (!el.placement.shelf_id) reveal(el.placement.face);
        show3d = false;
        tick().then(() => showInStage(document.querySelector(`.rack-screen .ritem[data-pl="${CSS.escape(el.placement.shelf_id ?? el.placement.id)}"]`)));
        return;
      }
      const p = shownRacks.flatMap((R) => R.pending).find((x) => x.instance_id === iid);
      if (p) { sel = null; show3d = false; arm(p, true); }
      if (p && !p.rackable) toast(`« ${p.tag ?? p.label} » est hors format rack : posez-le sur une étagère du rack.`, 'info');
    });
  });

  // ----- Pose et déplacement -----
  /** Éléments posés sur une face, dans l'ordre visuel du haut vers le bas (parcours au clavier). */
  const faceEls = (R: Rack | null | undefined, f: Face) => (R?.elements.filter((e) => e.placement.face === f && !e.placement.shelf_id) ?? []).sort((a, b) => b.placement.u_start - a.placement.u_start || a.placement.col - b.placement.col);
  const onShelf = (e: RackEl) => store[e.cid]?.elements.filter((x) => x.placement.shelf_id === e.placement.id) ?? [];
  const isShelf = (e: RackEl) => e.accessory?.kind === 'shelf';
  const panelOf = (e: { accessory: Accessory | null }) => (e.accessory?.kind === 'panel' ? e.accessory.panel : null);
  let editPanel = $state<RackEl | null>(null);
  const shelfOf = (e: RackEl) => store[e.cid]?.elements.find((x) => x.placement.id === e.placement.shelf_id);
  /** Étagères posées sur une face (toutes faces si `f` est omis), du haut vers le bas. */
  const shelvesOf = (R: Rack | null | undefined, f?: Face) => (R?.elements.filter((e) => isShelf(e) && !e.placement.shelf_id && (!f || e.placement.face === f)) ?? []).sort((a, b) => b.placement.u_start - a.placement.u_start || a.placement.col - b.placement.col);
  /** Nom court d'une étagère : « Étagère 2U · U14 » (son libellé s'il en a un). */
  const shelfName = (e: RackEl) => `${e.custom || `Étagère ${fmtU(hOf(e))}U`} · ${rangeTxt(e.placement.u_start, hOf(e))}`;
  const SHELF_MAX = 10;
  /** Étiquette d'un élément posé sur une étagère : son libellé, sinon son modèle (ID et marque en infobulle). */
  const chipName = (x: RackEl) => x.custom || x.label;
  /** Étiquettes affichées sur une étagère : autant que la largeur en tient, sinon « +n ». */
  function shelfLoad(e: RackEl) {
    const all = onShelf(e), room = BW / cols(e.placement.width) - 16 - (mode === 'schematic' ? Math.min(140, textWidth(e.label, '500 11px "Inter Variable", Inter, sans-serif') + 16) : 0);
    const w = (x: RackEl) => Math.min(120, textWidth(chipName(x), '600 10px "Inter Variable", Inter, sans-serif') + 14) + 4;
    let used = 0, n = 0;
    for (const [i, x] of all.entries()) { const need = w(x) + (i < all.length - 1 ? 30 : 0); if (used + need > room && n > 0) break; used += w(x); n++; }
    return { shown: all.slice(0, n), more: all.length - n, rest: all.slice(n).map(chipName).join(', ') };
  }
  /** Pose un élément hors format rack sur une étagère (depuis « À placer », éventuellement d'un autre rack). */
  async function putOnShelf(shelf: RackEl, p: Pending) {
    const to = shelf.cid, from = p.cid, dest = contOf(to);
    if (!canEdit(to) || placing) return;
    placing = true;
    try {
      const id = await act([to], async (): Promise<string | null> => {
        let article = p.article_id;
        if (from !== to) article = (await api.post<{ id: string }>(`/articles/${p.article_id}/move`, { rev: p.article_rev, container_id: to, location_id: dest?.location_id ?? null, instance_id: p.instance_id ?? null })).id;
        let r: { placement: Placement };
        try { r = await api.post(`/containers/${to}/placements`, { article_id: article, instance_id: p.instance_id ?? null, face: shelf.placement.face, u_start: shelf.placement.u_start, shelf_id: shelf.placement.id }); }
        catch (x) { if (from === to) throw x; toast(`« ${p.tag ?? p.label} » est dans « ${dest?.name} », à placer : ${(x as Error).message}`, 'warn'); return null; }
        const id = r.placement.id;
        toast(`« ${p.tag ?? p.label} » posé sur ${shelfName(shelf)}.`, 'ok', { run: async () => {
          await act([to], async () => { await api.del(`/placements/${id}`); if (from !== to) await sendBack({ instance_id: p.instance_id ?? null, article_id: article }, to, from); });
          if (sel === id) sel = null;
          await load([to, from]); await reload?.();
        } });
        armed = null; aim = null; sel = id;
        return id;
      });
      await load([to, from]);
      if (id) flash(id);
      if (from !== to) await reload?.();
    } catch (e) { fail(e); await load([to, from]); } finally { placing = false; }
  }
  /** Passe un élément posé sur une étagère vers une autre étagère du même rack (Annuler : retour). */
  async function moveToShelf(e: RackEl, shelf: RackEl) {
    const c = e.cid, back = e.placement.shelf_id, old = shelfOf(e);
    if (!canEdit(c) || !back || back === shelf.placement.id) return;
    const body = (rev: number, sid: string, at: RackEl | undefined) => ({ rev, face: at?.placement.face ?? e.placement.face, u_start: at?.placement.u_start ?? e.placement.u_start, width: e.placement.width, col: e.placement.col, circuit_id: e.placement.circuit_id, shelf_id: sid });
    try {
      await act([c], () => api.patch(`/placements/${e.placement.id}`, body(e.placement.rev, shelf.placement.id, shelf)));
      await load([c]); sel = e.placement.id; flash(e.placement.id);
      toast(`« ${nameOf(e)} » posé sur ${shelfName(shelf)}.`, 'ok', { run: async () => {
        const now = store[c]?.elements.find((x) => x.placement.id === e.placement.id);
        if (!now) return;
        await act([c], () => api.patch(`/placements/${e.placement.id}`, body(now.placement.rev, back, old)));
        await load([c]);
      } });
    } catch (x) { fail(x); await load([c]); }
  }
  const selected = $derived(sel ? (allEls.find((e) => e.placement.id === sel) ?? null) : null);
  /** Baie de l'élément sélectionné. */
  const selR = $derived(selected ? (store[selected.cid] ?? null) : null);
  let placing = false;
  /** Pose un élément « à placer » dans la baie `to` (côte à côte : il y change d'élément s'il venait d'une autre). */
  async function placeAt(u: number, col: number, p: Pending | null, f: Face, to: string | null = p?.cid ?? null) {
    // Une pose à la fois : le clic suivant attend la réponse du serveur.
    if (!p || !to || !canEdit(to) || placing) return;
    const from = p.cid, dest = contOf(to), wasArmed = armed === p, h = p.height_u ?? 1;
    placing = true;
    try {
      const id = await act([to], async (): Promise<string | null> => {
        let article = p.article_id;
        if (from !== to) article = (await api.post<{ id: string }>(`/articles/${p.article_id}/move`, { rev: p.article_rev, container_id: to, location_id: dest?.location_id ?? null, instance_id: p.instance_id ?? null })).id;
        let r: { adjusted: boolean; warning: string | null; placement: Placement };
        try { r = await api.post(`/containers/${to}/placements`, { article_id: article, instance_id: p.instance_id ?? null, face: f, u_start: u, col }); }
        catch (x) { if (from === to) throw x; toast(`« ${p.tag ?? p.label} » est dans « ${dest?.name} », à placer : ${(x as Error).message}`, 'warn'); return null; }
        const id = r.placement.id;
        toast(`« ${p.tag ?? p.label} » posé ${from !== to ? `dans « ${dest?.name} » ` : ''}en ${rangeTxt(r.placement.u_start, h)}.`, 'ok', { run: async () => {
          await act([to], async () => { await api.del(`/placements/${id}`); if (from !== to) await sendBack({ instance_id: p.instance_id ?? null, article_id: article }, to, from); });
          if (sel === id) sel = null;
          await load([to, from]); await reload?.();
        } });
        if (r.warning) toast(r.warning, 'warn');
        armed = null; aim = null; sel = id;
        return id;
      });
      await load([to, from]);
      if (id) flash(id);
      if (from !== to) await reload?.();
      const next = wasArmed ? store[from]?.pending.find((x) => x.article_id === p.article_id && x.rackable) : null;
      if (next) armed = next;
    } catch (e) { fail(e); await load([to, from]); } finally { placing = false; }
  }
  /** Annulation d'un changement de baie : l'élément « à placer » dans `at` retourne dans `home`. */
  async function sendBack(k: { instance_id: string | null; article_id: string }, at: string, home: string): Promise<string | null> {
    const r = await api.get<Rack>(`/containers/${at}/rack`);
    const q = r.pending.find((x) => (k.instance_id ? x.instance_id === k.instance_id : x.article_id === k.article_id));
    if (!q) return null;
    const m = await api.post<{ id: string }>(`/articles/${q.article_id}/move`, { rev: q.article_rev, container_id: home, location_id: contOf(home)?.location_id ?? null, instance_id: q.instance_id ?? null });
    return m.id;
  }
  async function patch(e: RackEl, change: Partial<Placement>) {
    const p = { ...e.placement, ...change };
    try { await act([e.cid], () => api.patch(`/placements/${p.id}`, { rev: e.placement.rev, face: p.face, u_start: p.u_start, width: p.width, col: p.col, circuit_id: p.circuit_id })); await load([e.cid]); }
    catch (x) { fail(x); await load([e.cid]); }
  }
  /** Retire un équipement du rack (ou de l'étagère) : il redevient « à placer ». Notification avec
   *  Annuler (reposé au même U, à la même face, à la même colonne), focus sur son entrée « À placer ». */
  async function unplace(e: RackEl) {
    const p = e.placement, c = e.cid, key = p.instance_id ?? p.article_id, where = p.shelf_id ? 'de l’étagère' : 'du rack';
    try {
      await act([c], () => api.del(`/placements/${p.id}`)); sel = null; await load([c]);
      toast(`« ${e.tag ?? e.label} » retiré ${where} : il est à placer.`, 'ok', { run: async () => {
        await act([c], () => api.post(`/containers/${c}/placements`, { article_id: p.article_id, instance_id: p.instance_id, face: p.face, u_start: p.u_start, width: p.width, col: p.col, circuit_id: p.circuit_id, shelf_id: p.shelf_id }));
        await load([c]);
      } });
      await tick();
      (document.querySelector<HTMLElement>(`[data-pend="${CSS.escape(key)}"]`) ?? document.querySelector<HTMLElement>('.rack-screen .rack-body'))?.focus({ preventScroll: true });
    } catch (x) { fail(x); await load([c]); }
  }
  // ----- Panneau de l'équipement sélectionné : position et libellé modifiables -----
  const hOf = (e: { height_u: number }) => e.height_u || 1;
  /** Place libre et dans les limites ? Renvoie le message d'erreur, sinon null. */
  function checkSpot(e: RackEl, u: number, f: Face, col = e.placement.col, R: Rack | null = store[e.cid] ?? null): string | null {
    const h = hOf(e), units = unitsOf(R), top = units - h + 1;
    if (!Number.isFinite(u) || !isWhole(u * 2)) return 'Un U entier ou un demi-U est attendu, par exemple 12 ou 12,5.';
    if (top < 1) return 'L’équipement est plus haut que le rack.';
    if (u < 1 || u > top + 1e-9) return `Entre U1 et U${fmtU(top)} pour cet équipement de ${fmtU(e.height_u)} U.`;
    const hit = blocker(R, h, e.placement.width, col, u, f, e.placement.id);
    return hit ? `Position occupée par « ${hit.custom || hit.label} ».` : null;
  }
  const nameOf = (e: RackEl) => e.custom || e.tag || e.label;
  /** Élément posé qui occupe déjà une partie de la place visée (même face, colonnes qui se chevauchent). */
  function blocker(R: Rack | null | undefined, h: number, w: W, col: number, u: number, f: Face, skip?: string): RackEl | null {
    const n = cols(w), a0 = col / n, a1 = (col + 1) / n;
    return R?.elements.find((o) => {
      if (o.placement.id === skip || o.placement.shelf_id || o.placement.face !== f) return false;
      const m = cols(o.placement.width), b0 = o.placement.col / m, b1 = (o.placement.col + 1) / m;
      const ou = o.placement.u_start, oh = hOf(o);
      return u < ou + oh - 1e-9 && ou < u + h - 1e-9 && a0 < b1 - 1e-9 && b0 < a1 - 1e-9;
    }) ?? null;
  }
  /** Champ « U de départ » : texte libre (12, 12,5, 12.5, U12,5), flèches au demi-U. */
  let uDraft = $state('');
  let posErr = $state<{ field: 'u' | 'face'; message: string } | null>(null);
  let labelDraft = $state('');
  let labelErr = $state<string | null>(null);
  // Brouillons repris à chaque changement de sélection ou rechargement de l'élévation.
  $effect(() => {
    const e = selected;
    untrack(() => { uDraft = e ? fmtU(e.placement.u_start) : ''; labelDraft = e?.custom ?? ''; });
  });
  $effect(() => { void sel; untrack(() => { posErr = null; labelErr = null; }); });
  let movingNow = false;
  /** Suite de petits déplacements (flèches, menu) d'un même élément : une seule notification, dont
   *  « Annuler » ramène à la position d'avant la suite. */
  let series: { id: string; from: { u: number; face: Face; col: number }; toast: number } | null = null;
  /** Déplace l'équipement dans sa baie (même appel que le glisser) ; erreur affichée sous le champ, toast avec Annuler. */
  async function moveTo(e: RackEl, change: { u_start?: number; face?: Face; col?: number }, field: 'u' | 'face', nudged = false) {
    const p = e.placement, c = e.cid, u = change.u_start ?? p.u_start, f = change.face ?? p.face, col = change.col ?? p.col;
    if (u === p.u_start && f === p.face && col === p.col) { posErr = null; return; }
    const bad = checkSpot(e, u, f, col);
    if (bad) { if (nudged) toast(`« ${nameOf(e)} » : ${bad.charAt(0).toLowerCase()}${bad.slice(1)}`, 'warn'); else posErr = { field, message: bad }; return; }
    if (movingNow) return;
    posErr = null;
    const name = nameOf(e);
    const go = series && series.id === p.id && nudged && toasts.some((t) => t.id === series!.toast) ? series : null;
    const from = go ? go.from : { u: p.u_start, face: p.face, col: p.col };
    movingNow = true;
    try {
      await act([c], () => api.patch(`/placements/${p.id}`, { rev: p.rev, face: f, u_start: u, width: p.width, col, circuit_id: p.circuit_id }));
      await load([c]);
      // Changement de face : la face d'arrivée est montrée (ajoutée aux vues).
      if (f !== p.face) reveal(f);
      flash(p.id);
      if (go) dismiss(go.toast);
      const tid = toast(`« ${name} » déplacé en ${aimTxt(u, hOf(e))}${f !== p.face ? `, face ${FACES[f].toLowerCase()}` : ''}.`, 'ok', { run: async () => {
        if (series?.id === p.id) series = null;
        const now = elById(p.id);
        if (!now) return false;
        await act([c], () => api.patch(`/placements/${p.id}`, { rev: now.placement.rev, face: from.face, u_start: from.u, width: p.width, col: from.col, circuit_id: now.placement.circuit_id }));
        await load([c]);
      } });
      series = nudged ? { id: p.id, from, toast: tid } : null;
    } catch (x) {
      const m = x instanceof ApiError ? (x.field('u_start') ?? x.field('face')) : undefined;
      if (m && !nudged) posErr = { field, message: m }; else fail(x);
      await load([c]);
    } finally { movingNow = false; }
    if (queued && queued.id === p.id) { const q = queued; queued = null; const now = elById(q.id); if (now) nudge(now, q.d); }
  }
  /** Petit déplacement (flèches, menu) : ± 0,5 U ou ± 1 U ; les appuis pendant l'enregistrement s'additionnent. */
  let queued: { id: string; d: number } | null = null;
  function nudge(e: RackEl, d: number) {
    if (!canEdit(e.cid) || e.placement.shelf_id) return;
    if (movingNow) { queued = { id: e.placement.id, d: (queued?.id === e.placement.id ? queued.d : 0) + d }; return; }
    moveTo(e, { u_start: e.placement.u_start + d }, 'u', true);
  }
  /** Raison pour laquelle un petit déplacement est impossible (ou null). */
  const nudgeWhy = (e: RackEl, d: number) => checkSpot(e, e.placement.u_start + d, e.placement.face);
  /** Déplace un équipement posé dans une autre baie (côte à côte) : retiré, changé d'élément, posé ; avec Annuler. */
  async function moveAcross(e: RackEl, to: string, u: number, f: Face, col: number) {
    const from = e.cid, p = e.placement, dest = contOf(to), name = nameOf(e);
    if (movingNow || !canEdit(from) || !canEdit(to)) return;
    movingNow = true;
    try {
      await act([from, to], async () => {
        await api.del(`/placements/${p.id}`);
        const moved = await api.post<{ id: string }>(`/articles/${p.article_id}/move`, { rev: e.article_rev, container_id: to, location_id: dest?.location_id ?? null, instance_id: p.instance_id });
        let r: { adjusted: boolean; placement: Placement };
        try { r = await api.post(`/containers/${to}/placements`, { article_id: moved.id, instance_id: p.instance_id, face: f, u_start: u, width: p.width, col }); }
        catch (x) { sel = null; toast(`« ${name} » est dans « ${dest?.name} », à placer : ${(x as Error).message}`, 'warn'); return; }
        const nid = r.placement.id;
        sel = nid;
        toast(`« ${name} » déplacé dans « ${dest?.name} » en ${aimTxt(r.placement.u_start, hOf(e))}.`, 'ok', { run: async () => {
          await act([to, from], async () => {
            await api.del(`/placements/${nid}`);
            const back = await sendBack({ instance_id: p.instance_id, article_id: moved.id }, to, from);
            if (back) await api.post(`/containers/${from}/placements`, { article_id: back, instance_id: p.instance_id, face: p.face, u_start: p.u_start, width: p.width, col: p.col, circuit_id: p.circuit_id });
          });
          if (sel === nid) sel = null;
          await load([from, to]); await reload?.();
        } });
      });
      await load([from, to]);
      if (sel) flash(sel);
      await reload?.();
    } catch (x) { fail(x); await load([from, to]); } finally { movingNow = false; }
  }
  function commitU(e: RackEl) {
    const u = parseU(uDraft);
    if (u === e.placement.u_start) { posErr = null; uDraft = fmtU(u); return; }
    if (u == null) { posErr = { field: 'u', message: 'Un numéro d’U est attendu, par exemple 12 ou 12,5.' }; return; }
    moveTo(e, { u_start: u }, 'u');
  }
  /** Flèches dans le champ « U de départ » : ± 0,5 U (Maj : ± 1 U), à valider par Entrée. */
  function uKey(k: KeyboardEvent, e: RackEl) {
    if (k.key === 'ArrowUp' || k.key === 'ArrowDown') {
      k.preventDefault();
      const top = Math.max(1, unitsOf(store[e.cid]) - hOf(e) + 1), cur = parseU(uDraft) ?? e.placement.u_start;
      const d = (k.shiftKey ? 1 : 0.5) * (k.key === 'ArrowUp' ? 1 : -1);
      uDraft = fmtU(Math.min(top, Math.max(1, Math.round((cur + d) * 2) / 2)));
      return;
    }
    cellKey(k, () => { uDraft = fmtU(e.placement.u_start); posErr = null; });
  }
  // Libellé personnalisé de l'exemplaire (même appel que l'inventaire) ; révision suivie localement.
  const instCache = $state<Record<string, Instance>>({});
  const instOf = (e: RackEl): Instance | null => (e.placement.instance_id ? (instCache[e.placement.instance_id] ?? view.instances.find((i) => i.id === e.placement.instance_id) ?? null) : null);
  async function saveLabel(i: Instance, label: string) {
    const cur = instCache[i.id] ?? i;
    const r = await api.patch<Instance>(`/instances/${i.id}`, { rev: cur.rev, label, serial: cur.serial, mac: cur.mac, inventory_tag: cur.inventory_tag });
    instCache[i.id] = r;
    return r;
  }
  async function commitLabel(e: RackEl) {
    const i = instOf(e);
    if (!i) return;
    const before = (instCache[i.id] ?? i).label ?? '', next = labelDraft.trim();
    if (next === before.trim()) { labelErr = null; labelDraft = before; return; }
    if (next.length > 80) { labelErr = '80 caractères au plus.'; return; }
    labelErr = null;
    try {
      await saveLabel(i, next);
      await load([e.cid]);
      toast(next ? `Libellé de « ${i.tag} » enregistré.` : `Libellé de « ${i.tag} » retiré.`, 'ok', { run: async () => { await saveLabel(i, before); await load([e.cid]); } });
    } catch (x) {
      const m = x instanceof ApiError ? x.field('label') : undefined;
      if (m) labelErr = m; else fail(x);
    }
  }
  /** Touches d'un champ du panneau : Entrée valide, Échap rétablit la valeur et rend la main au panneau. */
  function cellKey(k: KeyboardEvent, reset: () => void) {
    if (k.key === 'Enter') { k.preventDefault(); (k.currentTarget as HTMLInputElement).blur(); }
    else if (k.key === 'Escape') { k.preventDefault(); k.stopPropagation(); reset(); (k.currentTarget as HTMLInputElement).blur(); }
  }
  // Ports de la fiche (catalogue), chargés à la sélection quand le rôle permet de lire le catalogue.
  const portsCache = $state<Record<string, { total: number; inn: number; out: number } | null>>({});
  $effect(() => {
    const id = selected?.item_id;
    if (!id || id in portsCache || !can('catalogue', 'lecture')) return;
    portsCache[id] = null;
    api.get<{ item: { data: { ports?: { direction?: string; quantity?: number }[] } } }>(`/catalog/items/${id}`).then((r) => {
      const ps = r.item.data?.ports ?? [];
      const q = (d?: string) => ps.filter((x) => !d || x.direction === d).reduce((n, x) => n + (x.quantity ?? 1), 0);
      portsCache[id] = { total: q(), inn: q('In'), out: q('Out') };
    }).catch(() => { /* fiche illisible : pas de ports affichés */ });
  });
  const portsTxt = (p: { total: number; inn: number; out: number }) => `${p.total}${p.inn || p.out ? ` · ${[p.inn ? `${p.inn} entrée${pl(p.inn)}` : '', p.out ? `${p.out} sortie${pl(p.out)}` : ''].filter(Boolean).join(', ')}` : ''}`;
  /** Vignette de façade : même rendu que dans le rack, réduit à la largeur du panneau. */
  let thumbW = $state(0);
  const thumbZoom = (e: RackEl) => Math.max(0.3, Math.min(1, (thumbW - 20) / (BW + 24), 120 / (Math.max(1, e.height_u) * U)));

  async function clearRack() {
    const n = rack?.elements.length ?? 0;
    await ask({
      title: 'Vider le rack ?', eyebrow: rack?.container.name,
      text: `Tous les équipements posés (${n}) redeviennent « à placer ». L’inventaire ne change pas.`,
      confirmLabel: 'Vider le rack', danger: true,
      run: async () => { const r = await api.post<{ removed: number }>(`/containers/${cid}/placements/clear`); toast(`${r.removed} équipement${pl(r.removed)} remis à placer.`); await load(); },
    });
  }
  // ----- Visée : fantôme de l'équipement à poser ou à déplacer, aimanté au U ou au demi-U -----
  // Sujet : équipement glissé depuis le rack, déplacé au clavier (« Déplacer »), glissé depuis
  // « À placer », ou armé (clic, clavier).
  let dragging = $state<RackEl | null>(null);
  let moving = $state<RackEl | null>(null);
  let dragP = $state<Pending | null>(null);
  /** Place visée : baie, U de départ (bas de l'équipement), colonne, face et vue où elle s'affiche. */
  /** `y` : hauteur du pointeur en U depuis le bas ; `x` : position en largeur (0 à 1). Servent à viser une étagère. */
  interface Aim { cid: string; u: number; col: number; face: Face; view: View; y?: number; x?: number }
  let aim = $state<Aim | null>(null);
  /** Écart, en U, entre le bas de l'équipement et le pointeur (le fantôme suit la prise). */
  let grabOff = 0;
  type Subject = { move: RackEl; p?: undefined } | { p: Pending; move?: undefined };
  const subject = $derived<Subject | null>(dragging ? { move: dragging } : moving ? { move: moving } : dragP ? { p: dragP } : armed ? { p: armed } : null);
  const subjH = (s: Subject) => (s.move ? hOf(s.move) : (s.p.height_u || 1));
  const subjW = (s: Subject): W => (s.move ? s.move.placement.width : (s.p.width ?? 'full'));
  const subjFrom = (s: Subject) => (s.move ? s.move.cid : s.p.cid);
  /** Pas de la visée : un équipement déjà posé se déplace toujours au demi-U ; une pose depuis
   *  « À placer » s'aimante au U entier (au demi-U pour un élément d'un demi-U, ou avec Alt). */
  function stepFor(s: Subject | null, alt = false): number {
    if (alt) return 0.5;
    if (!s) return 1;
    if (s.move) return 0.5;
    return isWhole(subjH(s)) ? 1 : 0.5;
  }
  /** Face visée depuis une vue : la vue de côté garde la face de l'équipement déplacé. */
  const faceOfView = (R: Rack, v: View, s: Subject | null = subject): Face => (v === 'side' ? (s?.move && s.move.cid === rid(R) ? s.move.placement.face : mainFace(R)) : v);
  /** Sujet qui se pose sur une étagère : hors format rack, à placer ou déjà sur une étagère. */
  const shelfSubj = (s: Subject | null) => !!s && (s.p ? !s.p.rackable : !!s.move.placement.shelf_id);
  /** Étagère sous la place visée (face, hauteur et colonne du pointeur). */
  function shelfAt(R: Rack, at: Aim): RackEl | null {
    if (at.view === 'side') return null;
    const y = at.y ?? at.u - 0.5, x = at.x ?? (at.col + 0.5);
    return shelvesOf(R, at.face).find((e) => y >= e.placement.u_start - 1 - 1e-9 && y < e.placement.u_start - 1 + hOf(e) && Math.min(cols(e.placement.width) - 1, Math.floor(x * cols(e.placement.width))) === e.placement.col) ?? null;
  }
  /** Visée sur une étagère précise (clavier, « Déplacer »). */
  const aimOn = (sh: RackEl, v: View): Aim => ({ cid: sh.cid, u: sh.placement.u_start, col: sh.placement.col, face: sh.placement.face, view: v, y: sh.placement.u_start - 1 + hOf(sh) / 2, x: (sh.placement.col + 0.5) / cols(sh.placement.width) });
  /** Place visée : hauteur, colonne, plage « U36–U39 » et raison courte si la pose est impossible. */
  function previewAt(s: Subject | null, at: Aim | null) {
    const R = at ? store[at.cid] : null;
    if (!s || !at || !R || show3d) return null;
    if (shelfSubj(s)) {
      // Hors format rack : la cible est une étagère posée (elle s'éclaire), ailleurs la pose est refusée.
      const sh = shelfAt(R, at), name = s.move ? nameOf(s.move) : s.p.label;
      let why: string | null = null;
      if (!canEdit(at.cid)) why = R.lock && !R.lock.mine ? `${R.lock.account_name} a la main` : 'Lecture seule';
      else if (!sh) why = shelvesOf(R).length ? 'Hors format rack : posez-le sur une étagère' : 'Hors format rack : ajoutez une étagère au rack';
      else if (s.move && s.move.cid !== at.cid) why = 'Étagère d’un autre rack : retirez-le d’abord';
      else if (s.move?.placement.shelf_id !== sh.placement.id && onShelf(sh).length >= SHELF_MAX) why = `Étagère pleine (${SHELF_MAX} éléments)`;
      const same = !!s.move && !!sh && s.move.placement.shelf_id === sh.placement.id;
      const u = sh ? sh.placement.u_start : Math.max(1, Math.floor(at.y ?? at.u - 1) + 1), h = sh ? hOf(sh) : 1;
      return { cid: at.cid, u, h, n: sh ? cols(sh.placement.width) : 1, col: sh ? sh.placement.col : 0, why, same, view: at.view, face: at.face, name, range: sh ? `Poser sur ${shelfName(sh)}` : `U${fmtU(u)}`, shelf: sh };
    }
    const units = unitsOf(R), h = subjH(s), w = subjW(s), n = cols(w), col = Math.min(n - 1, Math.max(0, at.col)), u = at.u;
    let why: string | null = null;
    if (!canEdit(at.cid)) why = R.lock && !R.lock.mine ? `${R.lock.account_name} a la main` : 'Lecture seule';
    else if (u - 1 + h > units + 1e-9) why = 'Dépasse le haut du rack';
    else if (u < 1) why = 'Dépasse le bas du rack';
    else { const b = blocker(R, h, w, col, u, at.face, s.move?.placement.id); if (b) why = `Occupé par ${b.custom || b.label}`; }
    const same = !!s.move && s.move.cid === at.cid && s.move.placement.face === at.face && s.move.placement.u_start === u && s.move.placement.col === col;
    return { cid: at.cid, u, h, n, col, why, same, view: at.view, face: at.face, name: s.move ? (s.move.custom || s.move.label) : s.p.label, range: aimTxt(u, h), shelf: null as RackEl | null };
  }
  const preview = $derived(previewAt(subject, aim));
  /** U visé par le pointeur (aimanté au pas), moins la prise ; colonne d'après la largeur du sujet.
   *  Coordonnées d'écran de part et d'autre (rectangle affiché, zoom compris). */
  function aimFromEvent(ev: MouseEvent, el: HTMLElement, R: Rack, v: View, s: Subject | null = subject): Aim {
    const r = el.getBoundingClientRect(), f = faceOfView(R, v, s), units = unitsOf(R);
    const per = r.height / (units || 1) || 1;
    const fb = Math.max(0, Math.min(units - 1e-6, (r.bottom - ev.clientY) / per));
    const st = stepFor(s, ev.altKey);
    const u = Math.floor((fb - grabOff) / st + 1e-9) * st + 1;
    const n = s ? cols(subjW(s)) : 1;
    const col = v === 'side' ? (s?.move?.placement.col ?? 0) : Math.min(n - 1, Math.max(0, Math.floor(((ev.clientX - r.left) / r.width) * n)));
    return { cid: rid(R), u, col, face: f, view: v, y: fb, x: (ev.clientX - r.left) / (r.width || 1) };
  }
  /** Part d'un numéro d'U touchée par la place visée : tout l'U, sa moitié haute ou sa moitié basse. */
  function hotOf(R: Rack, n: number): '' | 'full' | 'top' | 'bot' {
    const pv = preview;
    if (!pv || pv.same || pv.cid !== rid(R)) return '';
    const a = Math.max(pv.u - 1, n - 1), b = Math.min(pv.u - 1 + pv.h, n);
    if (b - a < 1e-9) return '';
    if (b - a > 1 - 1e-9) return 'full';
    return a > n - 1 + 1e-9 ? 'top' : 'bot';
  }
  /** Bandes libres d'une face pendant une visée, au demi-U. */
  function freeRuns(R: Rack, f: Face) {
    if (!subject || shelfSubj(subject)) return [];
    const busy = new Set<number>();
    for (const e of faceEls(R, f)) {
      if (subject.move && e.placement.id === subject.move.placement.id) continue;
      const a = Math.round((e.placement.u_start - 1) * 2), b = Math.round((e.placement.u_start - 1 + hOf(e)) * 2);
      for (let k = a; k < b; k++) busy.add(k);
    }
    const out: { u: number; n: number }[] = [];
    for (let k = 0; k < unitsOf(R) * 2; k++) if (!busy.has(k)) { const last = out[out.length - 1]; if (last && last.u + last.n === k) last.n++; else out.push({ u: k, n: 1 }); }
    return out.map((r) => ({ b: r.u / 2, h: r.n / 2 }));
  }
  // Pose réussie : l'équipement clignote (accent) ; place impossible : le fantôme tremble.
  let flashId = $state<string | null>(null);
  function flash(id: string) { flashId = id; setTimeout(() => { if (flashId === id) flashId = null; }, 1300); }
  let shake = $state(false);
  function reject(why: string) {
    toast(`Impossible de poser ici : ${why.charAt(0).toLowerCase()}${why.slice(1)}.`, 'warn');
    shake = true; setTimeout(() => (shake = false), 420);
  }
  /** Valide la visée : pose, déplacement (dans la baie ou vers une autre), ou refus indiqué (rien n'est écrit). */
  function commit(s: Subject | null, at: Aim | null) {
    const pv = previewAt(s, at);
    if (!s || !pv || !at) return;
    if (pv.why) { reject(pv.why); return; }
    if (pv.shelf || shelfSubj(s)) {
      moving = null; aim = null;
      if (pv.same || !pv.shelf) return;
      if (s.move) moveToShelf(s.move, pv.shelf); else putOnShelf(pv.shelf, s.p);
      return;
    }
    if (s.move) {
      moving = null; aim = null;
      if (pv.same) return;
      sel = s.move.placement.id;
      if (s.move.cid === at.cid) moveTo(s.move, { u_start: pv.u, face: at.face, col: pv.col }, 'u');
      else moveAcross(s.move, at.cid, pv.u, at.face, pv.col);
      return;
    }
    placeAt(pv.u, pv.col, s.p, at.face, at.cid);
  }
  // Image de glisser transparente : seul le fantôme aimanté montre la place.
  const BLANK = typeof Image !== 'undefined' ? Object.assign(new Image(), { src: 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7' }) : null;
  function startMove(ev: DragEvent, e: RackEl) {
    const r = (ev.currentTarget as HTMLElement).getBoundingClientRect(), h = hOf(e);
    const st = stepFor({ move: e });
    grabOff = e.placement.shelf_id ? 0 : Math.max(0, Math.min(h - st, Math.floor((((r.bottom - ev.clientY) / (r.height || 1)) * h) / st) * st));
    dragging = e; armed = null; moving = null; aim = null; ctx = null;
    if (ev.dataTransfer) { ev.dataTransfer.effectAllowed = 'move'; ev.dataTransfer.setData('text/plain', nameOf(e)); if (BLANK) ev.dataTransfer.setDragImage(BLANK, 0, 0); }
  }
  const grabOf = (h: number, st: number) => Math.floor(h / 2 / st) * st;
  function startPend(ev: DragEvent, p: Pending) {
    const h = p.height_u || 1;
    grabOff = grabOf(h, isWhole(h) ? 1 : 0.5);
    dragP = p; armed = null; moving = null; aim = null; ctx = null;
    if (ev.dataTransfer) { ev.dataTransfer.effectAllowed = 'copyMove'; ev.dataTransfer.setData('text/plain', p.label); if (BLANK) ev.dataTransfer.setDragImage(BLANK, 0, 0); }
  }
  /** Fin du glisser : un lâcher sur une place impossible (refusé par le navigateur) est indiqué. */
  let lastBad: string | null = null, leftAt = 0;
  function endDrag() {
    // Lâché sur une place impossible : le navigateur refuse le dépôt (pas d'événement drop) et
    // envoie aussitôt dragleave puis dragend. Une sortie du rack plus ancienne n'est pas un refus.
    if (lastBad && (!leftAt || Date.now() - leftAt < 80)) reject(lastBad);
    dragging = null; dragP = null; aim = null; lastBad = null; leftAt = 0;
  }
  function dragOver(ev: DragEvent, R: Rack, v: View) {
    const s = subject;
    if (!s || !(dragging || dragP) || !canEdit(subjFrom(s))) return;
    ev.preventDefault();
    aim = aimFromEvent(ev, ev.currentTarget as HTMLElement, R, v);
    leftAt = 0;
    const pv = previewAt(s, aim);
    lastBad = pv?.why ?? null;
    if (ev.dataTransfer) ev.dataTransfer.dropEffect = pv?.why ? 'none' : 'move';
  }
  function dragLeave(ev: DragEvent, R: Rack, v: View) {
    if ((ev.currentTarget as HTMLElement).contains(ev.relatedTarget as Node)) return;
    if (aim?.view === v && aim.cid === rid(R)) aim = null;
    leftAt = Date.now();
  }
  function drop(ev: DragEvent, R: Rack, v: View) {
    ev.preventDefault();
    const s = subject, at = aimFromEvent(ev, ev.currentTarget as HTMLElement, R, v, s);
    lastBad = null;
    commit(s, at);
  }
  /** Survol, sortie et clic d'une vue (pose armée ou déplacement au clavier). */
  const aiming = $derived(!!armed || !!moving);
  function bodyMove(e: MouseEvent, R: Rack, v: View) { if (aiming) aim = aimFromEvent(e, e.currentTarget as HTMLElement, R, v); }
  function bodyLeave(R: Rack, v: View) { if (aiming && !moving && aim?.view === v && aim.cid === rid(R)) aim = null; }
  function bodyClick(e: MouseEvent, R: Rack, v: View) {
    if (aiming) commit(subject, aimFromEvent(e, e.currentTarget as HTMLElement, R, v));
    else if (!(e.target as Element).closest('.ritem, .sbar')) sel = null;
  }

  // ----- Circuits (vue d'une baie) -----
  let circ = $state({ name: '', phase: 'L1' as 'L1' | 'L2' | 'L3', rating_a: 16 });
  async function addCircuit(ev: SubmitEvent) {
    ev.preventDefault();
    try { await api.post(`/containers/${cid}/circuits`, { ...circ, name: circ.name || `PDU · ${circ.phase}` }); circ.name = ''; await load(); } catch (e) { fail(e); }
  }
  async function setPduCircuit(v: VPdu, circuit_id: string | null) {
    try { await api.patch(`/articles/${v.article_id}`, { rev: v.rev, quantity: 1, accessory: { ...v.accessory, circuit_id } }); await load(); } catch (e) { fail(e); }
  }
  async function delCircuit(c: Circuit) {
    try {
      await api.del(`/circuits/${c.id}`); await load();
      toast(`Circuit « ${c.name} » supprimé : ses équipements n’ont plus de circuit.`, 'ok', { run: async () => { await api.post(`/containers/${cid}/circuits`, { name: c.name, phase: c.phase, rating_a: c.rating_a }); await load(); } });
    } catch (e) { fail(e); }
  }

  // ----- Rendu -----
  // Couleur = famille de signal (charte § 4), jamais la catégorie : source unique familyColor().
  const famColor = (f: string | null | undefined) => familyColor(f, { screen: true });
  /** Légende « Familles de signal » du rendu schématique : familles des équipements posés. */
  const legend = $derived([...new Map((rack?.elements ?? []).filter((e) => e.nature !== 'accessory').map((e) => [e.function ?? '', e.function ? (refById('functions', e.function)?.name ?? e.function) : 'Sans famille'] as const)).entries()]);
  function style(e: RackEl) {
    const n = cols(e.placement.width), l = (100 * e.placement.col) / n, w = 100 / n, c = famColor(e.function);
    return `--c:${c};bottom:calc(${e.placement.u_start - 1} * var(--u) + 1px);height:calc(${e.height_u} * var(--u) - 2px);left:calc(${l}% + 2px);width:calc(${w}% - 4px)` + (mode === 'schematic' ? `;--fp-bg:linear-gradient(180deg,color-mix(in srgb,${c} 72%,#fff),${c});--fp-ink:#fff` : '');
  }
  /** Corps du libellé des façades (px, avant zoom) : 11 px, davantage au zoom faible pour
   *  rester lisible (même calcul que --rl-fs, styles/refine.css). */
  const rlFs = $derived(Math.max(11, 7.5 / zoom));
  /** Libellé seul : la façade n'a pas la hauteur des deux lignes (libellé puis modèle, ≈ 72 %) avec une marge. */
  const oneLine = (e: RackEl) => e.height_u * U - 2 < rlFs * (1.2 + 0.72 * 1.25) + 4;
  /** Textes : en schématique, les accessoires gardent leur nom (pas de façade dessinée). */
  const rl = (e: RackEl): [string, string] => (mode === 'schematic' && e.nature === 'accessory' ? [e.label, ''] : rackLines(e));
  /** PDU verticaux visibles : seulement dans les vues arrière. */
  const vpOf = (R: Rack, f: View): VPdu[] => (f === 'rear' || f === 'rear_inner' ? (R.vpdus ?? []) : []);
  const typeOf = (e: RackEl) => (e.nature === 'accessory' ? accFace(e as any) : visualOf(e));
  const dw = (e: RackEl) => !!store[e.cid]?.warnings.some((w) => w.placement_id === e.placement.id);
  /** Infobulle : texte complet (libellé, ID, marque et modèle), même quand la façade le raccourcit. */
  const elTitle = (e: RackEl) => `${e.custom ? `${e.custom} · ` : ''}${e.tag ? `${e.tag} · ` : ''}${modelOf(e)} · U${fmtU(e.placement.u_start)}`;
  // Vue de côté (même logique que la planche, lib/rackgeom) : profondeur de chaque élément depuis
  // ses rails, réserve de câblage (secours) derrière, cotes en mm centrées.
  const sideBarsOf = (R: Rack) => sideGeom(R.elements.filter((e) => !e.placement.shelf_id), depthOf(R), R.container.rear_rails_mm);
  // À placer, rangé par catégorie : équipements par fonction (Vidéo, Audio…), puis panels et
  // accessoires ; dans chaque catégorie, par nom.
  const catOf = (p: Pending): { key: string; label: string; color: string; order: number } => {
    if (p.accessory?.kind === 'panel') return { key: '~panel', label: 'Panels de brassage', color: 'var(--fg-3)', order: 1000 };
    if (p.nature === 'accessory') return { key: '~acc', label: 'Accessoires', color: 'var(--fg-3)', order: 1001 };
    // Fonction de la fiche, sinon celle de son type (ou du type parent).
    const t = p.type_id ? refById('types', p.type_id) : null;
    const fk = p.function ?? t?.data?.function ?? (t?.parent_id ? refById('types', t.parent_id)?.data?.function : null) ?? null;
    const f = fk ? refById('functions', fk) : null;
    return f ? { key: f.id, label: f.name, color: famColor(f.id), order: f.position ?? 500 } : { key: '~none', label: 'Autres équipements', color: famColor(null), order: 999 };
  };
  const byCat = (a: Pending, b: Pending) => catOf(a).order - catOf(b).order || a.label.localeCompare(b.label, 'fr') || (a.tag ?? '').localeCompare(b.tag ?? '');
  const wanted = $derived(rack?.pending.filter((p) => p.rackable) ?? []);
  const notRackable = $derived(rack?.pending.filter((p) => !p.rackable) ?? []);
  const wantedGroups = $derived.by(() => {
    const m = new Map<string, { key: string; label: string; color: string; order: number; list: Pending[] }>();
    for (const p of wanted) { const c = catOf(p); if (!m.has(c.key)) m.set(c.key, { ...c, list: [] }); m.get(c.key)!.list.push(p); }
    for (const g of m.values()) g.list.sort((a, b) => a.label.localeCompare(b.label, 'fr') || (a.tag ?? '').localeCompare(b.tag ?? ''));
    return [...m.values()].sort((a, b) => a.order - b.order || a.label.localeCompare(b.label, 'fr'));
  });
  /** Côte à côte : « À placer » de chaque baie. */
  const pendGroups = $derived(shownRacks.map((R) => ({ R, list: R.pending.filter((p) => p.rackable).sort(byCat), off: R.pending.filter((p) => !p.rackable) })));
  const nPend = $derived(pendGroups.reduce((n, g) => n + g.list.length, 0));
  /** Éléments « à placer » de la baie (vue d'une baie : sa liste ; côte à côte : celle de la baie visée). */
  const wantedOf = (id: string) => store[id]?.pending.filter((p) => p.rackable) ?? [];
  const FACES: Record<View, string> = { front: 'Avant', rear: 'Arrière', rear_inner: 'Arrière intérieure', side: 'Côté' };
  const widthTxt = (w: W | null) => (w && w !== 'full' ? ` · ${w === 'half' ? '½' : w === 'third' ? '⅓' : '¼'}` : '');

  // ----- Occupation : U réellement posés (pas le matériel affecté encore à placer) -----
  /** U posés sur une face (hors étagères), au prorata de la largeur (½, ⅓, ¼). */
  const posedOn = (R: Rack | null | undefined, f: Face) => (R?.elements ?? []).filter((e) => e.placement.face === f && !e.placement.shelf_id).reduce((n, e) => n + e.height_u / cols(e.placement.width), 0);
  const posedOf = (R: Rack | null | undefined) => (R ? Math.max(0, ...R.faces.map((f) => posedOn(R, f))) : 0);
  const posedU = $derived(posedOf(rack));
  const pendingU = $derived(wanted.reduce((n, p) => n + (p.height_u ?? 0), 0));
  /** Côte à côte : totaux des baies affichées. */
  const sum = $derived(shownRacks.reduce((s, R) => ({ u: s.u + posedOf(R), total: s.total + unitsOf(R), kg: s.kg + R.totals.weight_kg, w: s.w + R.totals.power_w }), { u: 0, total: 0, kg: 0, w: 0 }));

  // ----- Menu contextuel (clic droit, touche Menu, Maj+F10) : une ligne courte par action -----
  let ctx = $state<{ x: number; y: number; items: CtxItem[]; label: string; back: HTMLElement | null } | null>(null);
  /** Pose à faire après l'ajout de matériel lancé depuis le menu (« Ajouter un équipement »). */
  let addAt = $state<{ cid: string; u: number; face: Face; before: Set<string> } | null>(null);
  const RO = 'Prenez la main sur l’élévation pour la modifier.';
  const roWhy = (id: string) => { const R = store[id]; return all && R?.lock && !R.lock.mine ? `${R.lock.account_name} a la main sur ce rack.` : all ? 'Votre rôle ne permet pas de concevoir.' : RO; };
  /** Raison pour laquelle une hauteur ne tient pas à ce U (ou null). */
  function noFit(R: Rack | null | undefined, h: number, u: number, f: Face, w: W = 'full', col = 0): string | null {
    if (u - 1 + h > unitsOf(R) + 1e-9) return 'Dépasse le haut du rack';
    const b = blocker(R, h, w, col, u, f);
    return b ? `Occupé par ${b.custom || b.label}` : null;
  }
  /** Accessoires proposés au U cliqué : plaques (dont 0,5 U), passe-câbles, étagère, tiroir, laçage. */
  const ACCS: { kind: string; label: string; hs: number[]; make: (h: number, f: Face) => Accessory }[] = [
    { kind: 'blank', label: 'Plaque pleine', hs: [0.5, 1, 2, 3, 4], make: (h) => ({ kind: 'plate', variant: 'blank', height_u: h }) },
    { kind: 'brush', label: 'Passe-câbles', hs: [1, 2], make: (h) => ({ kind: 'plate', variant: 'brush', height_u: h }) },
    { kind: 'vented', label: 'Plaque aérée', hs: [1, 2], make: (h) => ({ kind: 'plate', variant: 'vented', height_u: h }) },
    { kind: 'shelf', label: 'Étagère', hs: [1, 2], make: (h) => ({ kind: 'shelf', depth_mm: 450, height_u: h }) },
    { kind: 'drawer', label: 'Tiroir', hs: [1, 2, 3, 4], make: (h) => ({ kind: 'drawer', depth_mm: 400, height_u: h }) },
    { kind: 'lacing', label: 'Barre de laçage', hs: [1], make: (h, f) => ({ kind: 'lacing_bar', depth_mm: f === 'front' ? 0 : 100, height_u: h }) },
  ];
  async function addAccAt(acc: Accessory, label: string, u: number, f: Face, at: string) {
    if (!canEdit(at)) return;
    lastAction = Date.now();
    try {
      const r = await api.post<{ article: Article }>(`/projects/${view.project.id}/articles`, { container_id: at, location_id: null, nature: 'accessory', accessory: acc, quantity: 1 });
      const art = r.article.id;
      try {
        const pl = await act([at], () => api.post<{ placement: Placement }>(`/containers/${at}/placements`, { article_id: art, instance_id: null, face: f, u_start: u, col: 0 }));
        const id = pl.placement.id;
        sel = id; await load([at]); flash(id);
        toast(`« ${label} » ajouté en ${rangeTxt(pl.placement.u_start, (acc as { height_u?: number }).height_u ?? 1)}.`, 'ok', { run: async () => { await api.del(`/articles/${art}`); if (sel === id) sel = null; await load([at]); } });
      } catch (x) { await load([at]); toast(`« ${label} » ajouté à placer : ${(x as Error).message}`, 'warn'); }
    } catch (x) { fail(x); }
  }
  /** Ajoute une étagère 1 U au premier U libre en partant du haut (face principale). */
  async function addShelf(R: Rack) {
    const f = mainFace(R), at = rid(R);
    let u: number | null = null;
    for (let c = unitsOf(R); c >= 1 && u == null; c--) if (!noFit(R, 1, c, f)) u = c;
    if (u == null) { toast('Plus de place libre pour une étagère sur cette face.', 'warn'); return; }
    await addAccAt({ kind: 'shelf', depth_mm: 450, height_u: 1 } as Accessory, 'Étagère 1 U', u, f, at);
  }
  /** Duplique un accessoire simple (plaque, étagère, tiroir, laçage) juste au-dessus, sinon au plus près. */
  async function duplicate(e: RackEl) {
    const a = e.accessory, R = store[e.cid];
    if (!a || !canEdit(e.cid)) return;
    const h = hOf(e), f = e.placement.face, units = unitsOf(R);
    let u: number | null = null;
    for (let d = 0; d <= units * 2 && u == null; d++) for (const c of [e.placement.u_start + h + d * 0.5, e.placement.u_start - h - d * 0.5]) if (u == null && c >= 1 && !noFit(R, h, c, f) && isWhole(c * 2)) u = c;
    if (u == null) { toast('Plus de place libre pour un double sur cette face.', 'warn'); return; }
    await addAccAt(a, e.label, u, f, e.cid);
  }
  /** Supprime l'élément de l'inventaire (après confirmation) : exemplaire seul (son ID part avec lui) ou accessoire. */
  async function removeFromInventory(e: RackEl) {
    const art = view.articles.find((a) => a.id === e.placement.article_id);
    const iid = e.placement.instance_id, name = nameOf(e), c = e.cid;
    await ask({
      title: `Supprimer « ${name} » de l’inventaire ?`, eyebrow: 'Inventaire',
      text: iid ? 'L’exemplaire quitte le rack et l’inventaire du projet, avec son ID. Le câblage qui le relie disparaît aussi.' : 'L’accessoire quitte le rack et l’inventaire du projet.',
      confirmLabel: 'Supprimer de l’inventaire', danger: true,
      run: async () => {
        lastAction = Date.now();
        if (iid && art && art.quantity > 1) {
          // Plusieurs exemplaires : celui-ci est d'abord séparé (sa propre ligne), puis supprimé.
          const moved = await api.post<{ id: string }>(`/articles/${art.id}/move`, { rev: art.rev, container_id: c, location_id: contOf(c)?.location_id ?? null, instance_id: iid });
          await api.del(`/articles/${moved.id}`);
        } else await api.del(`/articles/${e.placement.article_id}`);
        if (sel === e.placement.id) sel = null;
        await load([c]);
        toast(`« ${name} » supprimé de l’inventaire.`);
      },
    });
  }
  /** Focus sur le premier champ du panneau de droite (Modifier). */
  async function editSel(e: RackEl) {
    sel = e.placement.id; await tick();
    document.querySelector<HTMLElement>('.rack-screen .pane.right :is(input, select, button)')?.focus();
  }
  const bodySel = (c: string, v: View) => `.rack-screen [data-rack="${CSS.escape(c)}"][data-view="${v}"] :is(.rack-body, .side-body)`;
  /** Déplacer au clavier ou à la souris : la visée part de la position actuelle. */
  async function startMoving(e: RackEl, v: View) {
    if (!canEdit(e.cid)) return;
    armed = null; sel = e.placement.id; moving = e;
    grabOff = 0;
    const vv: View = v === 'side' || v === e.placement.face ? v : e.placement.face;
    if (vv !== 'side') reveal(vv);
    const sh = e.placement.shelf_id ? shelfOf(e) : null;
    aim = sh ? aimOn(sh, vv) : { cid: e.cid, u: e.placement.u_start, col: e.placement.col, face: e.placement.face, view: vv };
    await tick();
    document.querySelector<HTMLElement>(bodySel(e.cid, vv))?.focus({ preventScroll: true });
  }
  function elementMenu(e: RackEl, x: number, y: number, v: View, back: HTMLElement | null) {
    sel = e.placement.id;
    const ok = canEdit(e.cid), ro = ok ? undefined : roWhy(e.cid);
    const simple = e.nature === 'accessory' && !!e.accessory && e.accessory.kind !== 'panel' && e.accessory.kind !== 'vertical_pdu';
    const step = (d: number): CtxItem => { const why = ok ? nudgeWhy(e, d) : ro; return { label: d > 0 ? 'Monter' : 'Descendre', right: `${fmtU(Math.abs(d))} U`, disabled: !!why, title: why ?? undefined, run: () => nudge(e, d) }; };
    const R = store[e.cid];
    const others = e.placement.shelf_id ? shelvesOf(R).filter((x) => x.placement.id !== e.placement.shelf_id) : [];
    const items: CtxItem[] = [
      { label: ok ? 'Modifier' : 'Voir', icon: 'pen', run: () => editSel(e) },
      e.placement.shelf_id
        ? { label: 'Changer d’étagère', icon: 'hand', disabled: !ok || !others.length, title: ro ?? (!others.length ? 'Pas d’autre étagère dans ce rack.' : undefined),
            sub: [{ label: 'Choisir l’étagère', icon: 'hand', run: () => startMoving(e, v) }, { sep: true }, ...others.map((x): CtxItem => { const full = onShelf(x).length >= SHELF_MAX; return { label: shelfName(x), right: `${onShelf(x).length} / ${SHELF_MAX}`, disabled: full, title: full ? 'Étagère pleine.' : undefined, run: () => moveToShelf(e, x) }; })] }
        : { label: 'Déplacer', icon: 'hand', disabled: !ok, title: ro, sub: [{ label: 'Choisir la place', icon: 'hand', run: () => startMoving(e, v) }, { sep: true }, step(0.5), step(-0.5), step(1), step(-1)] },
    ];
    if (isShelf(e)) {
      // Étagère : « Poser ici » liste les éléments hors format rack à placer.
      const off = R?.pending.filter((x) => !x.rackable) ?? [], full = onShelf(e).length >= SHELF_MAX;
      items.push({ label: 'Poser ici', icon: 'server', disabled: !ok || !off.length || full, title: ro ?? (full ? 'Étagère pleine.' : !off.length ? 'Aucun élément hors format rack à placer.' : undefined),
        sub: off.map((x): CtxItem => ({ label: x.tag ? `${x.label} · ${x.tag}` : x.label, run: () => putOnShelf(e, x) })) });
    }
    if (simple) items.push({ label: 'Dupliquer', icon: 'copy', disabled: !ok || !canAdd, title: ro, run: () => duplicate(e) });
    if (panelOf(e)) items.push({ label: ok ? 'Modifier le panel' : 'Voir le panel', icon: 'grid', run: () => (editPanel = e) });
    if (e.item_id && e.nature !== 'accessory') items.push({ label: 'Ouvrir la fiche', icon: 'file', run: () => navigate(`/catalogue/equipment/${e.item_id}`) });
    items.push({ sep: true });
    items.push({ label: e.placement.shelf_id ? 'Retirer de l’étagère' : 'Retirer du rack', icon: 'x', danger: true, disabled: !ok, title: ro, run: () => unplace(e) });
    items.push({ label: 'Supprimer', icon: 'trash', danger: true, disabled: !ok || !canAdd, title: ro ?? (!canAdd ? 'Votre rôle ne permet pas de modifier l’inventaire.' : undefined), run: () => removeFromInventory(e) });
    ctx = { x, y, items, label: `Actions sur ${nameOf(e)}`, back };
  }
  function emptyMenu(R: Rack, u: number, f: Face, x: number, y: number, back: HTMLElement | null) {
    const at = rid(R), ok = canEdit(at), ro = ok ? null : roWhy(at), list = wantedOf(at);
    const accItems: CtxItem[] = ACCS.flatMap((a) => a.hs.map((h): CtxItem => { const why = noFit(R, h, u, f); return { label: a.label, right: `${fmtU(h)} U`, disabled: !!why, title: why ?? undefined, run: () => addAccAt(a.make(h, f), `${a.label} ${fmtU(h)} U`, u, f, at) }; }));
    const pendItems: CtxItem[] = list.map((p) => { const why = noFit(R, p.height_u || 1, u, f, p.width ?? 'full'); const twin = list.some((o) => o !== p && o.label === p.label); return { label: twin && p.tag ? `${p.label} · ${p.tag}` : p.label, right: `${fmtU(p.height_u ?? 1)} U${widthTxt(p.width)}`, disabled: !!why, title: why ?? p.tag ?? undefined, run: () => placeAt(u, 0, p, f, at) }; });
    const items: CtxItem[] = [
      { heading: `U${fmtU(u)}` },
      { label: 'Ajouter un accessoire', icon: 'panel', sub: accItems, disabled: !ok || !canAdd, title: ro ?? (!canAdd ? 'Votre rôle ne permet pas de modifier l’inventaire.' : undefined) },
      { label: 'Ajouter un équipement', icon: 'plus', disabled: !ok || !canAdd, title: ro ?? undefined, run: () => { addAt = { cid: at, u, face: f, before: new Set(R.pending.map(pk)) }; addCid = at; adding = true; } },
      { label: 'Poser ici', icon: 'server', sub: pendItems, disabled: !ok || !list.length, title: ro ?? (!list.length ? 'Rien à placer.' : undefined) },
    ];
    ctx = { x, y, items, label: `Actions à U${fmtU(u)}, ${FACES[f].toLowerCase()}`, back };
  }
  /** Clic droit dans une vue : sur un élément, ses actions ; dans le vide, les ajouts au U cliqué. */
  function onCtx(ev: MouseEvent, R: Rack, v: View) {
    if (show3d || aiming || dragging || dragP) return;
    ev.preventDefault();
    const body = ev.currentTarget as HTMLElement;
    const hit = (ev.target as Element).closest<HTMLElement>('[data-pl]');
    const e = hit ? R.elements.find((x) => x.placement.id === hit.dataset.pl) : null;
    if (e) { elementMenu(e, ev.clientX, ev.clientY, v, hit); return; }
    const f = faceOfView(R, v, null), units = unitsOf(R);
    const r = body.getBoundingClientRect(), per = r.height / (units || 1) || 1;
    const fb = Math.max(0, Math.min(units - 1e-6, (r.bottom - ev.clientY) / per));
    const under = Math.floor(fb) + 1;
    // Depuis le bas de l'U cliqué s'il est libre, sinon depuis sa moitié haute (demi-U).
    const lowFree = !noFit(R, 0.5, under, f), highFree = !noFit(R, 0.5, under + 0.5, f);
    const u = lowFree ? under : highFree ? under + 0.5 : fb % 1 < 0.5 ? under : under + 0.5;
    emptyMenu(R, u, f, ev.clientX, ev.clientY, body);
  }
  /** Touche Menu ou Maj+F10 sur un élément (ou sur l'élément sélectionné). */
  function keyMenu(e: RackEl, el: HTMLElement | null, v: View) {
    const r = (el ?? document.querySelector<HTMLElement>(`.rack-screen .ritem[data-pl="${CSS.escape(e.placement.id)}"]`))?.getBoundingClientRect();
    elementMenu(e, r ? r.left + Math.min(40, r.width / 2) : innerWidth / 2, r ? r.top + r.height / 2 : innerHeight / 2, v, el);
  }
  const isMenuKey = (k: KeyboardEvent) => k.key === 'ContextMenu' || (k.key === 'F10' && k.shiftKey);
  /** Matériel ajouté depuis « Ajouter un équipement » : le premier exemplaire rackable arrivé est posé au U choisi. */
  async function afterAdd() {
    const a = addAt;
    await load(a ? [a.cid] : addCid ? [addCid] : ids);
    const R = a ? store[a.cid] : null;
    if (!a || !R) return;
    const p = R.pending.find((x) => x.rackable && !a.before.has(pk(x)));
    if (!p) return;
    addAt = null;
    const why = noFit(R, p.height_u || 1, a.u, a.face, p.width ?? 'full');
    if (why) { toast(`« ${p.label} » est à placer : ${why.toLowerCase()} en U${fmtU(a.u)}.`, 'warn'); return; }
    await placeAt(a.u, 0, p, a.face, a.cid);
  }

  // ----- Clavier -----
  // Échap remonte d'un seul niveau (menu, déplacement ou équipement armé, puis sélection) ;
  // Suppr retire l'équipement sélectionné (annulable) ; Menu ou Maj+F10 ouvre ses actions ;
  // ↑↓ déplacent l'équipement sélectionné de 0,5 U (Maj : 1 U) ; Ctrl+= / Ctrl+- / Ctrl+0 : zoom.
  $effect(() => canvasKeys({
    onEscape: () => { if (moving) { moving = null; aim = null; } else if (armed) { armed = null; aim = null; } else if (sel) sel = null; },
    onDelete: () => { if (selected && canEdit(selected.cid) && !moving) unplace(selected); },
    onKey: (k) => {
      const mod = (k.ctrlKey || k.metaKey) && !k.altKey;
      if (mod && !show3d) {
        if (k.key === '+' || k.key === '=' || k.code === 'NumpadAdd') { zoomBy(1); return true; }
        if (k.key === '-' || k.code === 'NumpadSubtract') { zoomBy(-1); return true; }
        if (k.key === '0' || k.code === 'Digit0' || k.code === 'Numpad0') { setZoom('fit'); return true; }
      }
      if (isMenuKey(k) && selected && !show3d) { keyMenu(selected, null, selected.placement.shelf_id ? mainFace(selR) : selected.placement.face); return true; }
      if ((k.key === 'ArrowUp' || k.key === 'ArrowDown') && !mod && !k.altKey && selected && !aiming && !show3d && !selected.placement.shelf_id && canEdit(selected.cid)) {
        nudge(selected, (k.shiftKey ? 1 : 0.5) * (k.key === 'ArrowUp' ? 1 : -1)); return true;
      }
    },
  }));
  // Clic dans le vide (fond du dessin, zone vide, hors du rack) : désélectionne. Pas pendant une pose.
  $effect(() => blankClick({ enabled: () => !!sel && !armed && !moving && !dragging && !dragP && !ctx, onBlank: () => (sel = null) }));
  /** Pose au clavier : une fois l'équipement armé (ou « Déplacer »), flèches haut/bas pour choisir le U
   *  (déplacement : 0,5 U, Maj 1 U ; pose : 1 U, Alt pour le demi-U, Maj 5 pas), Entrée pour poser. */
  function rackKey(e: KeyboardEvent, R: Rack, v: View) {
    const s = subject;
    if (!s || !aiming) return;
    const f = faceOfView(R, v, s), h = subjH(s), st = stepFor(s, e.altKey), c = rid(R);
    if (shelfSubj(s)) {
      // Hors format rack : ↑↓ passent d'une étagère à l'autre, Entrée pose.
      const list = shelvesOf(R, f), cur = aim && aim.cid === c && aim.view === v ? shelfAt(R, aim) : null, i = cur ? list.indexOf(cur) : -1;
      const go = (k: number) => { const t = list[Math.max(0, Math.min(list.length - 1, k))]; if (t) aim = aimOn(t, v); };
      if (e.key === 'ArrowUp') go(i < 0 ? 0 : i - 1);
      else if (e.key === 'ArrowDown') go(i < 0 ? 0 : i + 1);
      else if (e.key === 'Enter' || e.key === ' ') commit(s, aim && aim.cid === c ? aim : list[0] ? aimOn(list[0], v) : { cid: c, u: 1, col: 0, face: f, view: v });
      else return;
      e.preventDefault();
      return;
    }
    const top = Math.max(1, Math.floor((unitsOf(R) - h) / st + 1e-9) * st + 1);
    const at: Aim = aim && aim.view === v && aim.cid === c ? aim : s.move && s.move.cid === c ? { cid: c, u: s.move.placement.u_start, col: s.move.placement.col, face: f, view: v } : { cid: c, u: top, col: 0, face: f, view: v };
    const n = cols(subjW(s)), d = s.move ? (e.shiftKey ? 1 : 0.5) : (e.shiftKey ? 5 : 1) * st;
    if (e.key === 'ArrowUp') aim = { ...at, u: Math.min(top, at.u + d) };
    else if (e.key === 'ArrowDown') aim = { ...at, u: Math.max(1, at.u - d) };
    else if (e.key === 'ArrowLeft' && n > 1 && v !== 'side') aim = { ...at, col: Math.max(0, at.col - 1) };
    else if (e.key === 'ArrowRight' && n > 1 && v !== 'side') aim = { ...at, col: Math.min(n - 1, at.col + 1) };
    else if (e.key === 'Enter' || e.key === ' ') commit(s, at);
    else return;
    e.preventDefault();
  }
  async function arm(p: Pending, force = false) {
    armed = !force && armed === p ? null : p;
    moving = null; aim = null; grabOff = armed ? grabOf(armed.height_u || 1, isWhole(armed.height_u || 1) ? 1 : 0.5) : 0;
    if (armed && narrow) sheetOpen = false;
    if (armed) { await tick(); document.querySelector<HTMLElement>(`${bodySel(armed.cid, mainFace(store[armed.cid]))}, .rack-screen :is(.rack-body, .side-body)`)?.focus({ preventScroll: true }); }
  }
  const bodyLabel = (R: Rack, v: View) => {
    const s = subject, name = all ? `${R.container.name}, ` : '';
    if (!s || !aiming) return v === 'side' ? `${name}vue de côté : profondeurs et réserves de câblage` : `${name}élévation ${FACES[v].toLowerCase()}`;
    const pv = preview && preview.view === v && preview.cid === rid(R) ? preview : null;
    if (shelfSubj(s)) return `${name}élévation ${FACES[v].toLowerCase()} : étagère pour ${s.move ? nameOf(s.move) : s.p.label}${pv ? `, ${pv.range}${pv.why ? `, ${pv.why.toLowerCase()}` : ''}` : ''} (flèches haut et bas d'une étagère à l'autre, Entrée pour poser, Échap pour annuler)`;
    return `${name}élévation ${FACES[v].toLowerCase()} : position de ${s.move ? nameOf(s.move) : s.p.label}${pv ? `, ${pv.range}${pv.why ? `, ${pv.why.toLowerCase()}` : ', libre'}` : ''} (flèches haut et bas${s.move ? ' : 0,5 U, Maj 1 U' : ', Alt pour le demi-U'}, Entrée pour poser, Échap pour annuler)`;
  };
  const RENDERS = [{ value: 'real', label: 'Réel' }, { value: 'schematic', label: 'Schématique' }, { value: '3d', label: '3D', icon: 'box' }] as const;
  const RENDERS2 = [{ value: 'real', label: 'Réel' }, { value: 'schematic', label: 'Schématique' }] as const;
</script>

<svelte:window onbeforeunload={leave} />

{#if !racks.length && !fixed}
  <EmptyState icon="server" message="Aucun rack dans ce projet" hint="Ajoutez un rack ou un flight-case depuis l’inventaire.">
    {#snippet action()}<a class="btn primary" href="/projets/{view.project.id}/inventaire"><Icon name="plus" size="sm" />Ajouter un élément</a>{/snippet}
  </EmptyState>
{:else if all ? loaded : rack}
  <div class="rack-screen card" class:solo={!fixed} class:all style="padding:0;overflow:hidden">
    {#if rack && (!mine || requested)}<div class="lockbar" class:mine class:other={!mine}>
      {#if mine}
        <span class="lk"><span class="key-i"><Icon name="lock" /></span><span><b>{requested}</b> demande la main.</span></span>
        <span class="sp"><button class="btn sm" onclick={() => { requested = null; release(); }}>Céder la main</button></span>
      {:else if rack.lock}
        <span class="lk"><span class="key-i"><Icon name="lock" /></span><span><b>{rack.lock.account_name}</b> a la main depuis {relative(rack.lock.taken_at)}. Vous voyez la dernière version enregistrée.</span></span>
        <span class="sp">{#if newer}<button class="btn sm" onclick={() => load()}><Icon name="refresh" size="sm" />Nouvelle version : recharger</button>{/if}{#if canDesign}<button class="btn sm" onclick={requestHand}>Demander la main</button>{/if}</span>
      {:else}
        <span class="lk"><span class="key-i"><Icon name="unlock" /></span><span>Personne ne modifie cette élévation.</span></span>
        <span class="sp">{#if canDesign}<button class="btn sm primary" onclick={take}>Prendre la main</button>{:else}<span class="muted small">Lecture seule : votre rôle ne permet pas de concevoir.</span>{/if}</span>
      {/if}
    </div>{/if}
    <!-- Barre : groupes qui passent à la ligne d'un bloc ; tous les contrôles en 34 px, centrés sur la même ligne. -->
    <div class="dtool rk-tool">
      {#if !fixed}<div class="tg tg-pick"><RackPicker {view} value={all ? (loc ? `tout:${loc}` : 'tout') : (cid ?? '')} onpick={(v) => navigate(`/projets/${view.project.id}/racks/${v.replace(':', '/')}`)} /></div>{/if}
      {#if all && withRacks.length > 1}
        <!-- Filtre d'emplacement : des liens (l'emplacement choisi est dans l'adresse). -->
        <div class="tg"><nav class="seg ar-locs" aria-label="Emplacement">
          <a href="/projets/{view.project.id}/racks/tout" class:on={!loc} aria-current={!loc ? 'page' : undefined}>Tous</a>
          {#each withRacks as l (l.id)}<a href="/projets/{view.project.id}/racks/tout/{l.id}" class:on={loc === l.id} aria-current={loc === l.id ? 'page' : undefined}>{l.name}</a>{/each}
        </nav></div>
      {/if}
      <div class="tg tg-view">
        <!-- En 3D, les vues et les textes n'ont pas d'effet : ils sont masqués (le cadrage est dans la vue 3D). -->
        {#if !show3d}
          <div class="seg" role="group" aria-label="Vues affichées (cumulables)">
            {#each [...barFaces, 'side' as const] as v (v)}
              {@const on = shownT.includes(v)}
              <button type="button" class:on aria-pressed={on} title={on && shownT.length === 1 ? 'Au moins une vue reste affichée' : on ? `Masquer la vue ${FACES[v].toLowerCase()}` : `Afficher aussi la vue ${FACES[v].toLowerCase()}`} onclick={() => toggleView(v)}>{v === 'rear_inner' ? 'Arr. int.' : FACES[v]}</button>
            {/each}
          </div>
        {/if}
        {#if all}<Seg label="Rendu" value={mode} options={[...RENDERS2]} onchange={(v) => (mode = v)} />
        {:else}<Seg label="Rendu" value={show3d ? '3d' : mode} options={[...RENDERS]} onchange={(v) => { if (v === '3d') { show3d = true; armed = null; moving = null; } else { mode = v; show3d = false; } }} />{/if}
      </div>
      {#if !show3d}<div class="tg tg-opt">
        <label class="rtx" title="Textes affichés sur les équipements"><span class="flabel">Textes</span><select class="input" value={rackText.v} onchange={(e) => setRackText((e.target as HTMLSelectElement).value as RackText)}>{#each Object.entries(RACK_TEXT) as [k, l]}<option value={k}>{l}</option>{/each}</select></label>
        {#if busyN}<span class="spin" role="status" aria-label="Enregistrement"></span>{/if}
      </div>{/if}
      <div class="tg tg-meters">
        {#if rack}
          {@const t = rack.totals}
          <span class="meter" class:err={!!t.total_u && posedU > t.total_u} title="U posés dans le rack{pendingU ? ` ; ${fmtU(pendingU)} U de matériel affecté reste à placer` : ''}"><span>U</span><Vu value={(posedU / (t.total_u || 1)) * 100} mode="load" n={16} /><span class="mono">{fmtU(posedU)} / {t.total_u} U</span>{#if pendingU}<span class="muted small mu-pend">+ {fmtU(pendingU)} U à placer</span>{/if}</span>
          <span class="meter" class:err={t.over_weight}><span>Poids</span><Vu value={rack.container.max_weight_kg ? (t.weight_kg / rack.container.max_weight_kg) * 100 : 0} mode="load" n={16} /><span class="mono">{nf(t.weight_kg, 1)}{rack.container.max_weight_kg ? ` / ${nf(rack.container.max_weight_kg)}` : ''} kg</span></span>
          <span class="meter" class:err={t.over_power}><span>Puiss.</span><Vu value={rack.container.max_power_w ? (t.power_w / rack.container.max_power_w) * 100 : 0} mode="load" n={16} /><span class="mono">{nf(t.power_w)}{rack.container.max_power_w ? ` / ${nf(rack.container.max_power_w)}` : ''} W</span></span>
        {:else}
          <span class="meter" title="U posés dans les racks affichés"><span>U</span><Vu value={(sum.u / (sum.total || 1)) * 100} mode="load" n={16} /><span class="mono">{fmtU(sum.u)} / {sum.total} U</span>{#if nPend}<span class="muted small mu-pend">+ {nPend} à placer</span>{/if}</span>
          <span class="meter"><span>Poids</span><span class="mono">{nf(sum.kg, 1)} kg</span></span>
          <span class="meter"><span>Puiss.</span><span class="mono">{nf(sum.w)} W</span></span>
        {/if}
      </div>
    </div>

    <div class="design-body" class:ro={!anyEdit}>
      <aside class="pane left" class:open={sheetOpen} aria-label="À placer">
        <button class="sheet-h" aria-expanded={sheetOpen} onclick={() => (sheetOpen = !sheetOpen)}>
          <span><b>À placer</b> <span class="muted">· {all ? nPend : wanted.length}</span></span>
          {#if armed}<span class="sheet-armed">{armed.label} · touchez une position</span>{/if}
          <Icon name="chevd" size="sm" />
        </button>
        <section>
          {#if rack}
            <h2 class="sheet-t" title={mine && !show3d ? 'Glissez un équipement dans le rack, ou choisissez-le puis sa position (clic, ou flèches et Entrée). Clic droit dans le rack pour ajouter ou poser à un U précis. Échap pour annuler.' : undefined}>À placer <span class="muted">· {wanted.length}</span></h2>
            {#if canAdd && shownC}<button class="btn sm add-mat" onclick={() => { addAt = null; addCid = cid; adding = true; }}><Icon name="plus" size="sm" />Ajouter du matériel</button>{/if}
            {#each wantedGroups as g (g.key)}
              <div class="pcat eyebrow"><i style="--c:{g.color}"></i><span>{g.label}</span><b>{g.list.length}</b></div>
              {#each g.list as p (pk(p))}{@render pendRow(p)}{/each}
            {:else}<p class="small muted">Tout est posé.</p>{/each}
            {#if notRackable.length}
              <h2 style="margin-top:8px" title="Hors format rack : glissez-le sur une étagère posée dans le rack, ou choisissez-le puis cliquez l’étagère.">Sur étagère <span class="muted">· {notRackable.length}</span></h2>
              {#if !shelvesOf(rack).length}
                <p class="small muted" style="margin:0">Ce rack n’a pas d’étagère pour les recevoir.</p>
                {#if mine && canAdd}<button class="btn sm add-mat" onclick={() => addShelf(rack!)}><Icon name="plus" size="sm" />Ajouter une étagère</button>{/if}
              {/if}
              {#each notRackable as p (pk(p))}{@render offRow(p)}{/each}
            {/if}
          {:else}
            <h2 class="sheet-t" title={anyEdit ? 'Glissez un élément dans un rack, ou choisissez-le puis sa position ; vers un autre rack, il change d’élément dans l’inventaire. Clic droit dans un rack pour ajouter ou poser à un U précis.' : undefined}>À placer <span class="muted">· {nPend}</span></h2>
            {#each pendGroups as g (rid(g.R))}
              {#if g.list.length || g.off.length}
                <div class="pcat eyebrow ar-pcat"><span>{g.R.container.name}</span><b>{g.list.length}</b>
                  {#if canAdd}<button class="icon-btn xs ghost" aria-label="Ajouter du matériel dans {g.R.container.name}" title="Ajouter du matériel" onclick={() => { addAt = null; addCid = rid(g.R); adding = true; }}><Icon name="plus" /></button>{/if}</div>
                {#each g.list as p (pk(p))}{@render pendRow(p)}{/each}
                {#each g.off as p (pk(p))}{@render offRow(p)}{/each}
              {/if}
            {/each}
            {#if !nPend}<p class="small muted">Tout est posé.</p>{/if}
            {#if canAdd && !pendGroups.some((g) => g.list.length || g.off.length)}
              <div class="ar-addall">{#each shownRacks as R (rid(R))}<button class="btn sm ghost" onclick={() => { addAt = null; addCid = rid(R); adding = true; }}><Icon name="plus" size="sm" />{R.container.name}</button>{/each}</div>
            {/if}
          {/if}
        </section>
      </aside>

      <div class="rk-stagebox" class:stage3d={show3d}>
        <div class="rack-stage rk-stage" class:stage3d={show3d} bind:this={scroller} bind:clientWidth={stageW} bind:clientHeight={stageH}>
          {#if show3d && rack}
            <Rack3D container={rack.container} elements={rack.elements} vpdus={rack.vpdus ?? []} title={rack.container.name} />
          {:else if all}
            <div class="all-racks rk-zoomed" style:zoom={zoom !== 1 ? zoom : undefined} style:--z={zoom}>
              {#each shownRacks as R (rid(R))}
                <div class="ar-col">
                  <div class="ar-head" style:zoom={zoom !== 1 ? 1 / zoom : undefined}><b>{R.container.name}</b><span class="muted small">{view.locations.find((l) => l.id === R.container.location_id)?.name ?? ''}{view.locations.some((l) => l.id === R.container.location_id) ? ' · ' : ''}{unitsOf(R)} U</span>
                    {#if R.lock && !R.lock.mine}<span class="chip warn" title="{R.lock.account_name} a la main : lecture seule"><Icon name="lock" />{R.lock.account_name}</span>{/if}
                    <a class="icon-btn sm ghost" href="/projets/{view.project.id}/racks/{rid(R)}" aria-label="Ouvrir le rack {R.container.name}" title="Ouvrir ce rack"><Icon name="arrowr" size="sm" /></a></div>
                  {@render rackRow(R)}
                </div>
              {:else}<p class="small muted">Aucun rack à cet emplacement.</p>{/each}
            </div>
          {:else if rack}
            <div class="rk-zoomed" style:zoom={zoom !== 1 ? zoom : undefined} style:--z={zoom}>{@render rackRow(rack)}</div>
          {/if}
        </div>
        {#if !show3d}
          <div class="rk-zoom" role="group" aria-label="Zoom">
            <button class="icon-btn sm ghost" title="Zoom arrière (Ctrl+-)" aria-label="Zoom arrière" disabled={zoom <= ZMIN + 1e-9} onclick={() => zoomBy(-1)}><Icon name="zout" size="sm" /></button>
            <button class="rk-zpct mono" title="Taille réelle (100 %)" aria-label="Zoom {Math.round(zoom * 100)} %, revenir à 100 %" onclick={() => setZoom(1)}>{Math.round(zoom * 100)} %</button>
            <button class="icon-btn sm ghost" title="Zoom avant (Ctrl+=)" aria-label="Zoom avant" disabled={zoom >= ZMAX - 1e-9} onclick={() => zoomBy(1)}><Icon name="zin" size="sm" /></button>
            <button class="btn sm ghost rk-fit" aria-pressed={fitOn} title="Ajuster les vues à la place disponible (Ctrl+0)" onclick={() => setZoom('fit')}><Icon name="fit" size="sm" />Ajuster</button>
          </div>
        {/if}
      </div>

      <aside class="pane right">
        {#if selected && selR}
          {@const p = selected.placement}
          {@const e = selected}
          {@const R = selR}
          {@const ok = canEdit(e.cid)}
          {@const inst = instOf(e)}
          {@const ports = e.item_id ? portsCache[e.item_id] : null}
          <Inspector eyebrow={e.nature === 'accessory' ? (panelOf(e) ? 'Panel' : 'Accessoire') : 'Équipement'} title={e.custom || e.label}
            id={e.tag} sub={e.custom ? modelOf(e) : e.brand && !e.label.toLowerCase().startsWith(e.brand.toLowerCase()) ? e.brand : null} onclose={() => (sel = null)}>
            {#snippet thumb()}
              <div class="ithumb" bind:clientWidth={thumbW}>
                <div class="ithumb-in" style="--u:{U}px;width:{BW}px;height:{e.height_u * U}px;zoom:{thumbZoom(e)}">
                  <div class="ritem {e.nature === 'accessory' ? 'acc' : ''} {ALU.has(typeOf(e)) && mode === 'real' ? 'alu' : ''} {e.height_u <= 1 ? 'u1' : ''} {e.height_u < 1 ? 'uh' : ''} {mode === 'schematic' ? 'sch' : ''}" style="{style(e).replace(/;?(bottom|height|left|width):[^;]*/g, '')};left:0;right:0;top:0;bottom:0" aria-hidden="true">
                    {#if e.nature !== 'accessory'}<i class="led"></i>{/if}<span class="rl" class:two={!!rl(e)[1]}>{#if rl(e)[0]}<b class="rn" class:id={!!e.tag && rl(e)[0] === e.tag}>{rl(e)[0]}</b>{/if}{#if rl(e)[1]}<small>{rl(e)[1]}</small>{/if}</span>{#if mode === 'real' && panelOf(e)}<span class="pdec"><PanelFace panel={panelOf(e)!} /></span>{:else if mode === 'real'}<span class="dec">{@html decor(typeOf(e))}</span>{:else}<span class="dec"></span>{/if}
                  </div>
                </div>
              </div>
            {/snippet}
            {#each R.warnings.filter((w) => w.placement_id === p.id) as w}<div class="alert warn"><Icon name="alert" /><span>{w.message}</span></div>{/each}
            <section class="insp-sec">
              <h3>Position</h3>
              {#if all}<div class="insp-row"><span class="insp-k">Rack</span><span class="insp-v">{R.container.name}</span></div>{/if}
              {#if p.shelf_id}
                <div class="insp-row"><span class="insp-k">Étagère</span><span class="insp-v">{shelfOf(e)?.custom || shelfOf(e)?.label || 'Étagère'} · U{fmtU(p.u_start)}</span></div>
              {:else}
                {@const top = Math.max(1, unitsOf(R) - hOf(e) + 1)}
                <div class="insp-row">
                  <label class="insp-k" for="rk-u">U de départ</label>
                  {#if ok}
                    <span class="insp-v insp-num"><input class="cell mono" id="rk-u" type="text" inputmode="decimal" autocomplete="off" spellcheck="false" role="spinbutton" aria-valuemin={1} aria-valuemax={top} aria-valuenow={p.u_start} aria-valuetext="U{fmtU(p.u_start)}"
                      title="Au demi-U près (12 ou 12,5) · ↑↓ 0,5 U · Maj+↑↓ 1 U" bind:value={uDraft} aria-invalid={posErr?.field === 'u' ? 'true' : undefined} aria-describedby={posErr?.field === 'u' ? 'rk-u-err' : 'rk-u-help'}
                      onblur={() => commitU(e)} onkeydown={(k) => uKey(k, e)}><span class="u">U</span></span>
                  {:else}<span class="insp-v mono" id="rk-u">U{fmtU(p.u_start)}</span>{/if}
                  {#if posErr?.field === 'u'}<FieldError message={posErr.message} for="rk-u" />{:else}<span class="help" id="rk-u-help">Occupe {occTxt(p.u_start, hOf(e))}.</span>{/if}
                </div>
                <div class="insp-row">
                  <label class="insp-k" for="rk-face">Face</label>
                  {#if ok && R.faces.length > 1}
                    <span class="insp-v"><select class="cell" id="rk-face" value={p.face} aria-invalid={posErr?.field === 'face' ? 'true' : undefined} aria-describedby={posErr?.field === 'face' ? 'rk-face-err' : undefined}
                      onchange={(ev) => { const f = (ev.target as HTMLSelectElement).value as Face; moveTo(e, { face: f }, 'face'); }}>{#each R.faces as f}<option value={f}>{FACES[f]}</option>{/each}</select></span>
                  {:else}<span class="insp-v" id="rk-face">{FACES[p.face]}</span>{/if}
                  {#if posErr?.field === 'face'}<FieldError message={posErr.message} for="rk-face" />{/if}
                </div>
              {/if}
              <div class="insp-row"><span class="insp-k">Hauteur</span><span class="insp-v">{fmtU(e.height_u)} U{p.width !== 'full' ? ` · ${p.width === 'half' ? '½' : p.width === 'third' ? '⅓' : '¼'} de largeur, colonne ${p.col + 1}` : ''}</span></div>
              {#if e.nature !== 'accessory' || p.shelf_id}
                <div class="insp-row">
                  <label class="insp-k" for="rk-c">Circuit</label>
                  {#if ok}
                    <span class="insp-v"><select class="cell" id="rk-c" value={p.circuit_id ?? ''} onchange={(ev) => patch(e, { circuit_id: (ev.target as HTMLSelectElement).value || null })}><option value="">Non branché</option>{#each R.circuits as c}<option value={c.id}>{c.name} · {c.phase} · {c.rating_a} A</option>{/each}</select></span>
                  {:else}<span class="insp-v" id="rk-c">{R.circuits.find((c) => c.id === p.circuit_id)?.name ?? 'Non branché'}</span>{/if}
                </div>
              {/if}
              {#if p.shelf_id}
                {@const others = shelvesOf(R).filter((x) => x.placement.id !== p.shelf_id)}
                {#if ok && others.length}
                  <div class="insp-row">
                    <label class="insp-k" for="rk-shelf">Déplacer vers</label>
                    <span class="insp-v"><select class="cell" id="rk-shelf" value="" onchange={(ev) => { const t = others.find((x) => x.placement.id === (ev.target as HTMLSelectElement).value); (ev.target as HTMLSelectElement).value = ''; if (t) moveToShelf(e, t); }}>
                      <option value="" disabled>Une autre étagère…</option>
                      {#each others as x (x.placement.id)}<option value={x.placement.id} disabled={onShelf(x).length >= SHELF_MAX}>{shelfName(x)} · {onShelf(x).length} / {SHELF_MAX}</option>{/each}
                    </select></span>
                  </div>
                {/if}
                <p class="insp-note">Posé sur une étagère : il en suit la position.</p>
              {/if}
            </section>
            <section class="insp-sec">
              <h3>Identification</h3>
              {#if inst}
                <div class="insp-row">
                  <label class="insp-k" for="rk-lbl">Libellé</label>
                  {#if canAdd}
                    <span class="insp-v"><input class="cell" id="rk-lbl" maxlength="80" placeholder="Ajouter un libellé" bind:value={labelDraft} aria-invalid={labelErr ? 'true' : undefined} aria-describedby={labelErr ? 'rk-lbl-err' : undefined}
                      onblur={() => commitLabel(e)} onkeydown={(k) => cellKey(k, () => { labelDraft = e.custom ?? ''; labelErr = null; })}></span>
                  {:else}<span class="insp-v" class:muted={!e.custom} id="rk-lbl">{e.custom || 'Aucun'}</span>{/if}
                  <FieldError message={labelErr} for="rk-lbl" />
                </div>
              {/if}
              {#if e.tag}<div class="insp-row"><span class="insp-k">ID</span><span class="insp-v mono">{e.tag}</span></div>{/if}
              {#if inst?.serial}<div class="insp-row"><span class="insp-k">N° de série</span><span class="insp-v mono">{inst.serial}</span></div>{/if}
              <div class="insp-row"><span class="insp-k">{e.nature === 'accessory' ? 'Accessoire' : 'Modèle'}</span><span class="insp-v">{modelOf(e)}</span></div>
            </section>
            {#if e.nature !== 'accessory' || e.weight_kg || e.depth_mm}
              <section class="insp-sec">
                <h3>Caractéristiques</h3>
                <div class="insp-row"><span class="insp-k">Poids</span><span class="insp-v" class:muted={!e.weight_kg}>{e.weight_kg ? `${nf(e.weight_kg, 1)} kg` : 'Non renseigné'}</span></div>
                {#if e.nature !== 'accessory'}<div class="insp-row"><span class="insp-k">Puissance</span><span class="insp-v" class:muted={!e.power_w}>{e.power_w ? `${nf(e.power_w)} W` : 'Non renseignée'}</span></div>{/if}
                <div class="insp-row"><span class="insp-k">Profondeur</span><span class="insp-v" class:muted={!e.depth_mm}>{e.depth_mm ? `${nf(e.depth_mm)} mm` : 'Non renseignée'}</span></div>
                {#if e.nature !== 'accessory'}<div class="insp-row"><span class="insp-k">Réserve câbles</span><span class="insp-v">{nf(e.rear_reserve_mm ?? 80)} mm{e.rear_reserve_mm == null ? ' (par défaut)' : ''}</span></div>{/if}
                {#if ports?.total}<div class="insp-row"><span class="insp-k">Ports</span><span class="insp-v">{portsTxt(ports)}</span></div>{/if}
              </section>
            {/if}
            {#if panelOf(e)}
              {@const pn = panelOf(e)!}
              <section class="insp-sec">
                <h3>Panel</h3>
                <div class="pside"><span class="muted small">{pn.slots.length} connecteur{pn.slots.length > 1 ? 's' : ''} sur {pn.height_u * pn.per_u} · {pn.groups.length} groupe{pn.groups.length > 1 ? 's' : ''}</span>
                  <button class="btn sm" onclick={() => (editPanel = e)}><Icon name="grid" size="sm" />{ok ? 'Modifier le panel' : 'Voir le panel'}</button></div>
              </section>
            {/if}
            {#if isShelf(e)}
              {@const off = R.pending.filter((x) => !x.rackable)}
              <section class="insp-sec">
                <h3>Sur l’étagère · {onShelf(e).length} / 10</h3>
                <div class="stack" style="gap:6px">
                  {#each onShelf(e) as x (x.placement.id)}<button class="pend" style="padding:8px 10px" onclick={() => (sel = x.placement.id)}><span class="pend-t"><b>{x.custom || x.label}</b><span>{x.tag ?? ''}</span></span></button>{:else}<p class="insp-note" style="margin:0">Rien n’est posé sur cette étagère.</p>{/each}
                  {#if ok && off.length && onShelf(e).length < 10}
                    {#each off as np (pk(np))}<button class="btn sm" style="justify-content:flex-start" onclick={() => putOnShelf(e, np)}><Icon name="plus" size="sm" />Poser {np.label}{np.tag ? ` · ${np.tag}` : ''}</button>{/each}
                  {/if}
                </div>
              </section>
            {/if}
            {#snippet actions()}
              {#if ok && !p.shelf_id}<button class="btn sm ghost" title="Déplacer au clavier (flèches puis Entrée) ou à la souris (clic à la nouvelle place)" onclick={() => startMoving(e, p.face)}><Icon name="hand" size="sm" />Déplacer</button>
              {:else if ok && shelvesOf(R).length > 1}<button class="btn sm ghost" title="Choisir une autre étagère (clic sur l’étagère, ou flèches puis Entrée)" onclick={() => startMoving(e, p.face)}><Icon name="hand" size="sm" />Changer d’étagère</button>{/if}
              {#if all}<a class="btn sm ghost" href="/projets/{view.project.id}/racks/{e.cid}{p.instance_id ? `?i=${p.instance_id}` : ''}"><Icon name="arrowr" size="sm" />Ouvrir dans le rack</a>{/if}
              {#if e.item_id && e.nature !== 'accessory'}<a class="btn sm ghost" href="/catalogue/equipment/{e.item_id}"><Icon name="file" size="sm" />Ouvrir la fiche</a>{/if}
              <a class="btn sm ghost" href="/projets/{view.project.id}/synoptique/element/{e.cid}"><Icon name="syn" size="sm" />Voir dans le synoptique</a>
              {#if ok}<button class="btn sm ghost danger" title="{p.shelf_id ? 'Retirer de l’étagère' : 'Retirer du rack'} (Suppr) : il redevient à placer" onclick={() => unplace(e)}><Icon name="x" size="sm" />{p.shelf_id ? 'Retirer de l’étagère' : 'Retirer du rack'}</button>{/if}
            {/snippet}
          </Inspector>
        {:else if rack}
        {@const t = rack.totals}
        <section>
          <h2>Énergie par phase</h2>
          {#each rack.electrical.phases as ph}
            {@const rating = rack.electrical.circuits.filter((c) => c.phase === ph.phase).reduce((s, c) => s + c.rating_a, 0)}
            <div class="meter" style="grid-template-columns:28px minmax(0,1fr) auto"><span class="mono">{ph.phase}</span><Vu value={rating ? (ph.amps / rating) * 100 : 0} mode="load" n={20} /><span class="mono">{nf(ph.amps, 1)}{rating ? ` / ${rating}` : ''} A</span></div>
          {/each}
          {#if rack.electrical.unbalanced}<div class="alert warn"><Icon name="alert" /><span>Phases déséquilibrées : écart de {rack.electrical.imbalance_pct} %.</span></div>{/if}
          {#if rack.electrical.unassigned_watts}<p class="small muted">{nf(rack.electrical.unassigned_watts)} W posés sans circuit.</p>{/if}
          {#each rack.electrical.circuits as c}
            <div class="circ" class:over={c.over}><span><b>{c.name}</b><span class="small muted">{' · '}{c.phase} · {c.rating_a} A</span></span><span class="mono">{nf(c.amps, 1)} A</span>{#if mine}<button class="icon-btn sm" aria-label="Supprimer le circuit {c.name}" title="Supprimer le circuit" onclick={() => delCircuit(rack!.circuits.find((x) => x.id === c.id)!)}><Icon name="x" /></button>{/if}</div>
          {/each}
          {#if mine}
            <form class="circ-form" onsubmit={addCircuit}>
              <div class="field circ-n"><label for="rk-cn">Nouveau circuit</label><input class="input sm" id="rk-cn" bind:value={circ.name} placeholder="PDU A · prise 1" title="Nom du circuit (par défaut « PDU · phase »)" data-no-dirty></div>
              <select class="input sm" bind:value={circ.phase} aria-label="Phase"><option>L1</option><option>L2</option><option>L3</option></select>
              <select class="input sm" bind:value={circ.rating_a} aria-label="Calibre"><option value={16}>16 A</option><option value={32}>32 A</option></select>
              <button class="btn sm"><Icon name="plus" size="sm" />Circuit</button>
            </form>
          {/if}
        </section>
        {#if rack.vpdus?.length}
          <section>
            <h2>PDU verticaux</h2>
            {#each rack.vpdus as v (v.article_id)}
              <div class="vp-row" class:over={v.over}>
                <span class="vp-n"><b>{v.accessory.side === 'left' ? 'Gauche' : 'Droite'} · {v.accessory.outlets} prises</b><small>{v.accessory.circuit_id ? `${v.fed} occupée${v.fed > 1 ? 's' : ''} · ${v.over ? `${v.fed - v.accessory.outlets} de trop` : `${v.free} libre${v.free > 1 ? 's' : ''}`}` : 'Rattachez un circuit pour compter les prises'}</small></span>
                <select class="input sm" aria-label="Circuit d’alimentation" disabled={!mine || !rack.circuits.length} value={v.accessory.circuit_id ?? ''} onchange={(e) => setPduCircuit(v, (e.target as HTMLSelectElement).value || null)}><option value="">{rack.circuits.length ? 'Sans circuit' : 'Aucun circuit'}</option>{#each rack.circuits as c}<option value={c.id}>{c.name} · {c.phase}</option>{/each}</select>
              </div>
            {/each}
            {#if rack.vpdus.some((v) => v.over)}<div class="alert warn"><Icon name="alert" /><span>Plus d’équipements alimentés que de prises sur un PDU : ajoutez un PDU ou répartissez les équipements sur un autre circuit.</span></div>{/if}
            <p class="small muted">Prises occupées : équipements posés, avec une consommation, rattachés au même circuit que le PDU.</p>
          </section>
        {/if}
        <section>
          <h2>Totaux</h2>
          <dl class="kv"><dt>Hauteur posée</dt><dd>{fmtU(posedU)} / {t.total_u} U</dd>{#if pendingU}<dt>À placer</dt><dd>{fmtU(pendingU)} U{t.total_u && posedU + pendingU > t.total_u ? ` · dépasse de ${fmtU(posedU + pendingU - t.total_u)} U une fois tout posé` : ''}</dd>{/if}<dt>Poids</dt><dd>{nf(t.weight_kg, 1)} kg</dd><dt>Puissance</dt><dd>{nf(t.power_w)} W</dd><dt>Dissipation</dt><dd>{nf(t.power_w * 3.412)} BTU/h</dd><dt>Profondeur utile</dt><dd>{nf(depthOf(rack))} mm</dd></dl>
          {#each rack.warnings as w}<div class="alert warn"><Icon name="alert" /><span>{w.message}</span></div>{/each}
          {#if mode === 'schematic' && !show3d && legend.length}<span class="lbl-t">Familles de signal</span><div class="legend" style="grid-template-columns:1fr">{#each legend as [k, name] (k)}<div><span class="sw" style="--c:{famColor(k || null)}"></span>{name}</div>{/each}</div>{/if}
        </section>
        {#if mine && rack.elements.length}
          <section class="rk-clear">
            <button class="btn sm ghost danger" title="Tous les équipements posés redeviennent « à placer » (après confirmation)" onclick={clearRack}><Icon name="undo" size="sm" />Vider le rack</button>
          </section>
        {/if}
        {:else}
          <section>
            <h2>Totaux</h2>
            <dl class="kv"><dt>Racks</dt><dd>{shownRacks.length}</dd><dt>Hauteur posée</dt><dd>{fmtU(sum.u)} / {sum.total} U</dd><dt>Poids</dt><dd>{nf(sum.kg, 1)} kg</dd><dt>Puissance</dt><dd>{nf(sum.w)} W</dd><dt>Dissipation</dt><dd>{nf(sum.w * 3.412)} BTU/h</dd>{#if nPend}<dt>À placer</dt><dd>{nPend} élément{pl(nPend)}</dd>{/if}</dl>
          </section>
          <section>
            <h2>Par rack</h2>
            {#each shownRacks as R (rid(R))}
              <a class="ar-tot" href="/projets/{view.project.id}/racks/{rid(R)}" title="Ouvrir le rack {R.container.name}">
                <span class="ar-tn"><b>{R.container.name}</b><small>{nf(R.totals.weight_kg, 1)} kg · {nf(R.totals.power_w)} W{R.warnings.length ? ` · ${R.warnings.length} alerte${pl(R.warnings.length)}` : ''}</small></span>
                <span class="ar-tu mono" class:err={R.totals.over_u}>{fmtU(posedOf(R))} / {unitsOf(R)} U</span>
              </a>
            {/each}
            {#if mode === 'schematic'}
              {@const lg = [...new Map(allEls.filter((e) => e.nature !== 'accessory').map((e) => [e.function ?? '', e.function ? (refById('functions', e.function)?.name ?? e.function) : 'Sans famille'] as const)).entries()]}
              {#if lg.length}<span class="lbl-t">Familles de signal</span><div class="legend" style="grid-template-columns:1fr">{#each lg as [k, name] (k)}<div><span class="sw" style="--c:{famColor(k || null)}"></span>{name}</div>{/each}</div>{/if}
            {/if}
          </section>
        {/if}
      </aside>
    </div>
  </div>
{:else}<Loading label={all ? 'Chargement des racks' : 'Chargement du rack'} />{/if}

<!-- Élément « à placer » : clic pour l'armer, ou glisser dans un rack. -->
{#snippet pendRow(p: Pending)}
  {@const ok = canEdit(p.cid)}
  <button class="pend mini" class:on={armed === p} class:dragging={dragP === p} aria-pressed={armed === p} data-pend={pk(p)} disabled={!ok || show3d} onclick={() => arm(p)}
    draggable={ok && !show3d} ondragstart={(ev) => startPend(ev, p)} ondragend={endDrag} title="{p.label}{p.tag ? ` · ${p.tag}` : ''}">
    <i class="pm-c" style="--c:{famColor(p.function)}"></i><span class="pm-l">{p.label}</span>{#if p.tag}<span class="pm-t">{p.tag}</span>{/if}<span class="pm-u">{fmtU(p.height_u ?? 0)} U{widthTxt(p.width)}</span>
  </button>
{/snippet}
<!-- Hors format rack : même geste qu'un équipement rackable, la cible est une étagère posée. -->
{#snippet offRow(p: Pending)}
  {@const ok = canEdit(p.cid)}
  <button class="pend mini" class:on={armed === p} class:dragging={dragP === p} aria-pressed={armed === p} data-pend={pk(p)} disabled={!ok || show3d} onclick={() => arm(p)}
    draggable={ok && !show3d} ondragstart={(ev) => startPend(ev, p)} ondragend={endDrag} title="{p.label}{p.tag ? ` · ${p.tag}` : ''} · hors format rack : à poser sur une étagère">
    <i class="pm-c" style="--c:{famColor(p.function)}"></i><span class="pm-l">{p.label}</span>{#if p.tag}<span class="pm-t">{p.tag}</span>{/if}<span class="pm-u"><Icon name="server" size="sm" /></span>
  </button>
{/snippet}

<!-- Vues d'une baie côte à côte, même échelle, alignées sur les U : graduations à gauche de la première et à droite de la dernière. -->
{#snippet rackRow(R: Rack)}
  {@const sv = shownOf(R)}
  <div class="rk-row">
    {#each sv as v, vi (v)}
      <div class="rk-view" data-view={v} data-rack={rid(R)}>
        <div class="rk-cap"><b>{FACES[v]}</b>{#if v === 'side'}<span class="mono">P {nf(depthOf(R))} mm</span>{/if}</div>
        {#if v === 'side'}{@render sideView(R, vi === 0, vi === sv.length - 1)}{:else}{@render faceView(R, v, vi === 0, vi === sv.length - 1)}{/if}
      </div>
    {/each}
  </div>
{/snippet}

<!-- Vue d'une face : élévation, glisser-déposer, pose armée, déplacement, clic droit. -->
{#snippet faceView(R: Rack, f: Face, first: boolean, last: boolean)}
  {@const vp = vpOf(R, f)}
  {@const units = unitsOf(R)}
  {@const ok = canEdit(rid(R))}
  {@const pv = preview && preview.cid === rid(R) && preview.view === f && !preview.same ? preview : null}
  <div class="rack" class:placing={aiming} class:vpl={vp.some((v) => v.accessory.side === 'left')} class:vpr={vp.some((v) => v.accessory.side === 'right')} style="--units:{units};--u:{U}px;--bw:{BW}px;grid-template-columns:{first ? '28px ' : ''}auto{last ? ' 28px' : ''}">
    <!-- PDU verticaux : couloirs arrière, gauche et droite vus de l'arrière. -->
    {#each vp as v, i (v.article_id)}
      {@const h = Math.min(units * U, (v.accessory.length_mm / 44.45) * U)}
      {@const left = v.accessory.side === 'left'}
      {@const k = vp.filter((x, j) => j < i && x.accessory.side === v.accessory.side).length}
      <div class="vpdu" style="{left ? 'left' : 'right'}:{6 + k * 20}px;bottom:calc(24px + {(units * U - h) / 2}px);height:{h}px;--n:{v.accessory.outlets}" title={v.label}><i></i></div>
    {/each}
    {#if first}{@render scale(R)}{/if}
    <div class="rack-body" class:aiming={!!subject} class:no={!!pv?.why} role="grid" tabindex={aiming ? 0 : -1} aria-label={bodyLabel(R, f)}
      onmousemove={(e) => bodyMove(e, R, f)}
      onmouseleave={() => bodyLeave(R, f)}
      onclick={(e) => bodyClick(e, R, f)}
      onkeydown={(e) => rackKey(e, R, f)}
      oncontextmenu={(e) => onCtx(e, R, f)}
      ondragover={(e) => dragOver(e, R, f)}
      ondragleave={(e) => dragLeave(e, R, f)}
      ondrop={(e) => drop(e, R, f)}>
      {#if subject}{#each freeRuns(R, f) as r (r.b)}<span class="rfree" style="bottom:calc({r.b} * var(--u));height:calc({r.h} * var(--u))"></span>{/each}{/if}
      {#if subject && shelfSubj(subject)}{#each shelvesOf(R, f) as sh (sh.placement.id)}<span class="rshelf" class:full={onShelf(sh).length >= SHELF_MAX} style="bottom:calc({sh.placement.u_start - 1} * var(--u));height:calc({hOf(sh)} * var(--u));left:calc({(100 * sh.placement.col) / cols(sh.placement.width)}% + 1px);width:calc({100 / cols(sh.placement.width)}% - 2px)"></span>{/each}{/if}
      {#each faceEls(R, f) as e (e.placement.id)}
        <div class="ritem {e.nature === 'accessory' ? 'acc' : ''} {ALU.has(typeOf(e)) && mode === 'real' ? 'alu' : ''} {e.height_u <= 1 ? 'u1' : ''} {e.height_u < 1 ? 'uh' : ''} {e.placement.width !== 'full' ? `half c${e.placement.col}` : ''} {e.placement.width === 'third' || e.placement.width === 'quarter' ? 'nw' : ''} {mode === 'schematic' ? 'sch' : ''} {oneLine(e) ? 'one' : ''}" class:sel={sel === e.placement.id} class:dw={dw(e)} class:draggable={ok} class:dragging={dragging === e || moving === e} class:flash={flashId === e.placement.id}
          style={style(e)} data-pl={e.placement.id} role="button" tabindex="0" draggable={ok} aria-pressed={sel === e.placement.id} aria-haspopup="menu" aria-label={elTitle(e)} title={elTitle(e)}
          onclick={() => { if (!aiming) sel = e.placement.id; }} onkeydown={(k) => { if (k.key === 'Enter' || k.key === ' ') { if (aiming) return; k.preventDefault(); sel = e.placement.id; } else if (isMenuKey(k)) { k.preventDefault(); k.stopPropagation(); keyMenu(e, k.currentTarget as HTMLElement, f); } }}
          ondragstart={(ev) => startMove(ev, e)} ondragend={endDrag}>
          {#if e.nature !== 'accessory'}<i class="led"></i>{/if}<span class="rl" class:two={!!rl(e)[1]}>{#if rl(e)[0]}<b class="rn" class:id={!!e.tag && rl(e)[0] === e.tag}>{rl(e)[0]}</b>{/if}{#if rl(e)[1]}<small>{rl(e)[1]}</small>{/if}</span>{#if isShelf(e) && onShelf(e).length}{@const ld = shelfLoad(e)}<span class="shelf-load">{#each ld.shown as x (x.placement.id)}<span class="sh-it" class:on={sel === x.placement.id} class:flash={flashId === x.placement.id} style="--c:{famColor(x.function)}" role="button" tabindex="0" data-shelf-it={x.placement.id}
            title="{elTitle(x)} · sur {shelfName(e)}" aria-label="{nameOf(x)}, sur {shelfName(e)}" draggable={ok}
            onclick={(ev) => { if (aiming) return; ev.stopPropagation(); sel = x.placement.id; }}
            onkeydown={(k) => { if (k.key === 'Enter' || k.key === ' ') { k.preventDefault(); k.stopPropagation(); sel = x.placement.id; } else if (isMenuKey(k)) { k.preventDefault(); k.stopPropagation(); keyMenu(x, k.currentTarget as HTMLElement, f); } }}
            oncontextmenu={(ev) => { if (show3d || aiming || dragging || dragP) return; ev.preventDefault(); ev.stopPropagation(); elementMenu(x, ev.clientX, ev.clientY, f, ev.currentTarget as HTMLElement); }}
            ondragstart={(ev) => { ev.stopPropagation(); startMove(ev, x); }} ondragend={endDrag}>{chipName(x)}</span>{/each}{#if ld.more}<span class="sh-more" title={ld.rest}>+{ld.more}</span>{/if}</span>{:else if mode === 'real' && panelOf(e)}<span class="pdec"><PanelFace panel={panelOf(e)!} /></span>{:else if mode === 'real'}<span class="dec">{@html decor(typeOf(e))}</span>{:else}<span class="dec"></span>{/if}
        </div>
      {/each}
      {#if pv}{@render ghost(pv)}{/if}
    </div>
    {#if last}{@render scale(R)}{/if}
  </div>
{/snippet}

<!-- Vue de côté : élévation technique à plat (profondeur de chaque élément depuis ses rails,
     réserve de câblage hachurée), cotes horizontales centrées en mm, profondeur utile en pied. -->
{#snippet sideView(R: Rack, first: boolean, last: boolean)}
  {@const pv = preview && preview.cid === rid(R) && preview.view === 'side' && !preview.same ? preview : null}
  {@const depth = depthOf(R)}
  {@const ok = canEdit(rid(R))}
  <div class="rack rk-side" class:placing={aiming} style="--units:{unitsOf(R)};--u:{U}px;--sw:{swOf(R)}px;grid-template-columns:{first ? '28px ' : ''}auto{last ? ' 28px' : ''}">
    {#if first}{@render scale(R)}{/if}
    <div class="side-wrap">
      <div class="side-body" class:aiming={!!subject} class:no={!!pv?.why} role="grid" tabindex={aiming ? 0 : -1} aria-label={bodyLabel(R, 'side')}
        onmousemove={(e) => bodyMove(e, R, 'side')}
        onmouseleave={() => bodyLeave(R, 'side')}
        onclick={(e) => bodyClick(e, R, 'side')}
        onkeydown={(e) => rackKey(e, R, 'side')}
        oncontextmenu={(e) => onCtx(e, R, 'side')}
        ondragover={(e) => dragOver(e, R, 'side')}
        ondragleave={(e) => dragLeave(e, R, 'side')}
        ondrop={(e) => drop(e, R, 'side')}>
        <i class="s-rail" style="left:{(FRONT_RAIL_MM / depth) * 100}%"></i>
        <i class="s-rail" style="left:{((depth - (R.container.rear_rails_mm ?? 70)) / depth) * 100}%"></i>
        {#each sideBarsOf(R) as b (b.e.placement.id)}
          {@const e = b.e as RackEl}
          {@const big = e.height_u >= 0.9}
          {#if b.res > 0}<span class="sres" class:over={b.over} style="bottom:calc({e.placement.u_start - 1} * var(--u) + 1px);height:calc({e.height_u} * var(--u) - 2px);left:{(b.rx / depth) * 100}%;width:{(b.rw / depth) * 100}%" title="Réserve de câblage : {nf(b.res)} mm{b.over ? ' (dépasse la profondeur utile)' : ''}">{#if big && b.rw * PX_MM > 40}<span class="cote"><i></i><b>{nf(Math.round(b.res))}</b></span>{/if}</span>{/if}
          <span class="sbar" class:acc={e.nature === 'accessory'} class:sel={sel === e.placement.id} class:dragging={dragging === e || moving === e} class:flash={flashId === e.placement.id} class:draggable={ok} data-pl={e.placement.id} role="button" tabindex="0" draggable={ok} aria-pressed={sel === e.placement.id} aria-haspopup="menu"
            style="--c:{famColor(e.function)};bottom:calc({e.placement.u_start - 1} * var(--u) + 1px);height:calc({e.height_u} * var(--u) - 2px);left:{(b.x / depth) * 100}%;width:{(b.w / depth) * 100}%"
            title="{elTitle(e)} · {FACES[e.placement.face].toLowerCase()} · profondeur {e.depth_mm ? `${nf(e.depth_mm)} mm` : `non renseignée (${nf(b.d)} mm supposés)`}{b.res ? ` · réserve câbles ${nf(b.res)} mm` : ''}" aria-label="{elTitle(e)}, profondeur {nf(b.d)} mm"
            onclick={() => { if (!aiming) sel = e.placement.id; }} onkeydown={(k) => { if (k.key === 'Enter' || k.key === ' ') { if (aiming) return; k.preventDefault(); sel = e.placement.id; } else if (isMenuKey(k)) { k.preventDefault(); k.stopPropagation(); keyMenu(e, k.currentTarget as HTMLElement, 'side'); } }}
            ondragstart={(ev) => startMove(ev, e)} ondragend={endDrag}>
            {#if big && b.w * PX_MM > 34}<span class="cote" class:est={!e.depth_mm}><i></i><b>{nf(Math.round(b.d))}{b.w * PX_MM > 90 ? ' mm' : ''}</b></span>{/if}
          </span>
        {/each}
        {#if pv}{@render ghost(pv)}{/if}
      </div>
      <!-- Profondeur totale utile : cote centrée sous la vue. -->
      <div class="s-total" aria-label="Profondeur utile {nf(depth)} mm"><span class="s-end">Avant</span><span class="cote tot"><i></i><b>{nf(depth)} mm</b></span><span class="s-end">Arrière</span></div>
    </div>
    {#if last}{@render scale(R)}{/if}
  </div>
{/snippet}

{#snippet scale(R: Rack)}
  {@const units = unitsOf(R)}
  <div class="rack-scale">{#each Array(units) as _, i}{@const hot = hotOf(R, units - i)}<span class:hot={!!hot} class:top={hot === 'top'} class:bot={hot === 'bot'} class:no={!!hot && !!preview?.why}>{units - i}</span>{/each}</div>
{/snippet}

{#snippet ghost(pv: NonNullable<typeof preview>)}
  {@const units = unitsOf(store[pv.cid])}
  {@const u0 = Math.max(1, pv.u)}
  {@const u1 = Math.min(units + 1, pv.u + pv.h)}
  <div class="aim" class:no={!!pv.why} class:shake aria-hidden="true" style="bottom:calc({u0 - 1} * var(--u) + 1px);height:calc({Math.max(0.5, u1 - u0)} * var(--u) - 2px);left:calc({pv.view === 'side' ? 0 : (100 * pv.col) / pv.n}% + 2px);width:calc({pv.view === 'side' ? 100 : 100 / pv.n}% - 4px)">
    <span class="aim-n">{pv.name}</span><span class="aim-u">{pv.range}</span>{#if pv.why}<span class="aim-why">{pv.why}</span>{/if}
  </div>
{/snippet}

{#if adding && addC}<AddArticle {view} container={addC} locationId={addC.location_id} onclose={() => { adding = false; addAt = null; }} ondone={afterAdd} />{/if}

{#if editPanel}
  <ProjectPanel article={{ id: editPanel.placement.article_id, rev: editPanel.article_rev, accessory: editPanel.accessory, label: editPanel.label } as Article} readonly={!canEdit(editPanel.cid)} eyebrow="Panel du rack" onclose={() => (editPanel = null)} ondone={() => load(editPanel ? [editPanel.cid] : ids)} />
{/if}

{#if ctx}<ContextMenu x={ctx?.x ?? 0} y={ctx?.y ?? 0} items={ctx?.items ?? []} label={ctx?.label ?? ''} back={ctx?.back ?? null} onclose={() => (ctx = null)} />{/if}

<style>
  .add-mat { align-self: stretch; justify-content: center; margin: 2px 0 6px; }
  .rack-screen :global(.rack-body) { width: var(--bw, 370px); }
  .pcat { display: flex; align-items: center; gap: 8px; margin: 10px 0 2px; padding: 0 2px; }
  .mu-pend { white-space: nowrap; }
  .rack-body:focus-visible, .side-body:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  /* Visée (pose et déplacement). La baie est un objet sombre fixe : couleurs d'outil claires fixes
     (accent du thème sombre, erreur du thème sombre), lisibles sur le métal dans les deux thèmes. */
  .rack { --aim: #9DB0FF; --aim-no: #FF7B70; }
  .ritem.draggable, .sbar.draggable { cursor: grab; }
  .ritem.draggable:active, .sbar.draggable:active { cursor: grabbing; }
  .ritem.dragging, .pend.dragging, .sbar.dragging { opacity: .4; }
  .ritem.dragging { outline: 1.5px dashed var(--aim); outline-offset: 1px; filter: none; }
  .rack-body.aiming, .side-body.aiming { cursor: copy; }
  .rack-body.aiming.no, .side-body.aiming.no { cursor: not-allowed; }
  .rack-body.aiming .ritem, .side-body.aiming .sbar { cursor: inherit; }
  .rack-body.aiming .ritem:not(.dragging) { filter: brightness(.62) saturate(.7); }
  .rfree { position: absolute; left: 0; right: 0; z-index: 0; pointer-events: none; background: color-mix(in srgb, var(--aim) 9%, transparent); box-shadow: inset 0 1px 0 color-mix(in srgb, var(--aim) 22%, transparent), inset 0 -1px 0 color-mix(in srgb, var(--aim) 22%, transparent); }
  .aim { position: absolute; z-index: 4; pointer-events: none; box-sizing: border-box; display: flex; flex-wrap: wrap; align-items: center; justify-content: center; align-content: center; column-gap: 8px; row-gap: 1px; padding: 0 8px; overflow: hidden; border-radius: 3px;
    background: color-mix(in srgb, var(--aim) 26%, rgba(8, 10, 14, .55)); box-shadow: inset 0 0 0 2px var(--aim), 0 0 0 1px rgba(0, 0, 0, .6), 0 6px 16px -6px rgba(0, 0, 0, .7); color: #F6F7FB; font: 600 11.5px/1.25 var(--font); text-align: center; white-space: nowrap; }
  .aim.no { background: repeating-linear-gradient(135deg, color-mix(in srgb, var(--aim-no) 34%, rgba(8, 10, 14, .6)) 0 7px, rgba(8, 10, 14, .6) 7px 14px); box-shadow: inset 0 0 0 2px var(--aim-no), 0 0 0 1px rgba(0, 0, 0, .6); }
  .aim-n { max-width: 100%; overflow: hidden; text-overflow: ellipsis; }
  .aim-u { font: 700 11.5px/1.25 var(--mono); color: var(--aim); font-variant-numeric: tabular-nums; }
  .aim.no .aim-u, .aim-why { color: var(--aim-no); }
  .aim-why { font-weight: 700; }
  .rack-scale span.hot { color: var(--aim); font-weight: 700; }
  .rack-scale span.hot.no { color: var(--aim-no); }
  /* Demi-U visé : la moitié touchée de l'U est marquée sur la graduation. */
  .rack-scale span.hot.top, .rack-scale span.hot.bot { --hb: color-mix(in srgb, var(--aim) 30%, transparent); }
  .rack-scale span.hot.no:is(.top, .bot) { --hb: color-mix(in srgb, var(--aim-no) 30%, transparent); }
  .rack-scale span.hot.top { background: linear-gradient(to bottom, var(--hb) 50%, transparent 50%); }
  .rack-scale span.hot.bot { background: linear-gradient(to top, var(--hb) 50%, transparent 50%); }
  .aim.shake { animation: aim-shake .4s ease; }
  .ritem.flash, .sbar.flash { animation: rk-flash 1.2s ease-out; z-index: 3; }
  @keyframes aim-shake { 20%, 60% { transform: translateX(-5px); } 40%, 80% { transform: translateX(5px); } }
  @keyframes rk-flash { 0%, 40% { box-shadow: 0 0 0 2px #fff, 0 0 0 6px var(--aim), 0 0 18px 6px var(--aim); } 100% { box-shadow: 0 0 0 0 transparent; } }
  @media (prefers-reduced-motion: reduce) {
    .aim.shake, .ritem.flash, .sbar.flash { animation: none; }
    .ritem.flash, .sbar.flash { outline: 2px solid var(--aim); outline-offset: 2px; }
  }
  /* Demi-U : façade de 17 px, textes sur une ligne, vis au milieu. */
  .ritem.uh { font-size: 11px; }
  .pcat i { width: 8px; height: 8px; border-radius: 50%; background: var(--c); }
  .pcat b { margin-left: auto; font: 600 11px var(--mono); padding: 1px 7px; border-radius: 99px; background: var(--surface-3); color: var(--fg-2); }
  .pend.mini { flex-direction: row; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 8px; min-height: 34px; }
  .pm-c { flex: none; width: 4px; align-self: stretch; border-radius: 2px; background: var(--c); }
  .pm-l { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13.5px !important; font-weight: 600; color: var(--fg) !important; }
  .pm-t { flex: none; font: 600 11px/1 var(--mono) !important; color: var(--fg-3); }
  .pm-u { flex: none; font: 600 11px/1 var(--mono) !important; padding: 3px 6px; border-radius: 5px; background: var(--sunken); color: var(--fg-2) !important; }
  .rack-stage.stage3d { padding: 0; display: block; align-self: stretch; }

  /* ----- Scène : vues côte à côte, sans marge perdue ; elle défile seule, le zoom flotte en bas à droite ----- */
  .rk-stagebox { position: relative; min-width: 0; min-height: 0; display: grid; grid-template-columns: minmax(0, 1fr); grid-template-rows: minmax(0, 1fr); }
  .rk-stagebox > .rack-stage { min-width: 0; min-height: 0; }
  .rk-stage:not(.stage3d) { padding: 12px 12px 56px; align-items: flex-start; }
  .rk-zoomed { flex: none; margin: 0 auto; }
  .rk-row { display: flex; align-items: flex-start; gap: 14px; margin: 0 auto; }
  /* Contrôle de zoom : même aspect que celui du synoptique. */
  .rk-zoom { position: absolute; right: 12px; bottom: 12px; z-index: 5; display: flex; align-items: center; gap: 2px; padding: 3px; border-radius: 9px; border: 1px solid var(--line); background: color-mix(in srgb, var(--panel) 88%, transparent); box-shadow: 0 4px 14px -6px rgba(0, 0, 0, .35); backdrop-filter: blur(6px); }
  .rk-zpct { min-width: 50px; height: var(--hit-min, 28px); padding: 0 4px; border: 0; border-radius: 6px; background: none; color: var(--text-2); font-size: 12px; text-align: center; cursor: pointer; font-variant-numeric: tabular-nums; }
  .rk-zpct:hover { background: var(--panel-3); color: var(--text); }
  .rk-fit[aria-pressed='true'] { color: var(--accent); }
  /* ----- Côte à côte : une colonne par baie, en-tête, vues ----- */
  .all-racks { display: flex; gap: 28px; align-items: flex-start; margin: 0 auto; }
  .ar-col { flex: none; display: flex; flex-direction: column; gap: 8px; }
  .ar-head { display: flex; align-items: center; gap: 8px; height: 30px; padding: 0 4px; min-width: 0; }
  .ar-head b { font: 650 var(--fs-md, 13.5px) var(--display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--fg); }
  .ar-head .muted { margin-right: auto; white-space: nowrap; }
  .ar-pcat .icon-btn { margin-left: 2px; }
  .ar-addall { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; }
  /* Liens du filtre d'emplacement : même aspect que les boutons d'une commande segmentée. */
  .ar-locs a { display: inline-flex; align-items: center; height: var(--btn-h-sm, 30px); padding: 0 11px; border-radius: var(--btn-r-sm, 7px); font-size: var(--fs-ctl-sm, 13px); font-weight: 550; color: var(--fg-2); text-decoration: none; white-space: nowrap; }
  .ar-locs a:hover { color: var(--fg); }
  .ar-locs a.on { background: var(--key-bg); color: var(--fg); font-weight: 650; box-shadow: var(--key); }
  .ar-locs a:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  /* Totaux par rack : une ligne cliquable par rack. */
  .ar-tot { display: flex; align-items: center; gap: 10px; padding: 8px; border-radius: var(--r-sm, 8px); color: var(--fg); text-decoration: none; border-bottom: 1px solid var(--line); }
  .ar-tot:hover { background: color-mix(in srgb, var(--accent) 7%, transparent); }
  .ar-tot:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .ar-tn { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.3; }
  .ar-tn b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ar-tn small { font-size: var(--fs-sm, 12.5px); color: var(--fg-3); }
  .ar-tu { flex: none; font-size: 12.5px; color: var(--fg-2); }
  .ar-tu.err { color: var(--err); font-weight: 700; }
  .pcat span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rk-view { flex: none; display: flex; flex-direction: column; gap: 6px; }
  /* Légende de vue : à plat, même hauteur pour toutes les vues (les U restent alignés). */
  .rk-cap { height: 22px; display: flex; align-items: center; justify-content: center; gap: 8px; font: 600 var(--fs-2xs, 11.5px)/1 var(--font); letter-spacing: .05em; text-transform: uppercase; color: var(--fg-3); }
  .rk-cap b { font-weight: 650; color: var(--fg-2); }
  .rk-cap .mono { letter-spacing: 0; text-transform: none; }
  .rk-row .rack { box-shadow: var(--rack-rim), inset 0 1px 0 rgba(255, 255, 255, .09), inset 0 -2px 0 rgba(0, 0, 0, .4), 0 10px 24px -14px rgba(0, 0, 0, .45); }

  /* ----- Vue de côté : élévation technique à plat, cotée ----- */
  .rk-side { padding-bottom: 8px; }
  .rk-side::before { display: none; }
  .side-wrap { display: flex; flex-direction: column; gap: 6px; padding: 0 5px; }
  .rk-row .side-body { position: relative; width: var(--sw); height: calc(var(--units) * var(--u)); background: repeating-linear-gradient(to top, #2A2F36 0 1px, #101317 1px var(--u)); border: 0; box-shadow: inset 2px 0 0 #9AA1A9, inset -2px 0 0 #9AA1A9; }
  .s-rail { position: absolute; top: 0; bottom: 0; width: 0; border-left: 1px dashed #5A6370; pointer-events: none; }
  .rk-row .sbar { position: absolute; z-index: 1; box-sizing: border-box; border-radius: 1px; background: color-mix(in srgb, var(--c) 42%, #1B1F25); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--c) 80%, #000); display: flex; align-items: center; justify-content: center; overflow: hidden; }
  .rk-row .sbar.acc { background: #30353C; box-shadow: inset 0 0 0 1px #5A6370; }
  .rk-row .sbar.sel { outline: 1.5px solid var(--accent); outline-offset: 1px; z-index: 2; }
  .rk-row .sbar:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .rk-row .sres { position: absolute; z-index: 1; box-sizing: border-box; display: flex; align-items: center; justify-content: center; background: repeating-linear-gradient(45deg, rgba(255, 255, 255, .16) 0 1px, transparent 1px 6px); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, .22); border-radius: 1px; }
  .rk-row .sres.over { background: repeating-linear-gradient(45deg, rgba(255, 123, 112, .55) 0 1px, transparent 1px 6px); box-shadow: inset 0 0 0 1px #FF7B70; }
  /* Cote horizontale centrée : trait avec flèches, valeur en mm au milieu. */
  .cote { position: relative; display: flex; align-items: center; justify-content: center; width: calc(100% - 6px); height: 100%; pointer-events: none; }
  .cote i { position: absolute; left: 0; right: 0; top: 50%; height: 0; border-top: 1px solid rgba(242, 244, 247, .75); }
  .cote i::before, .cote i::after { content: ""; position: absolute; top: -4px; width: 0; height: 7px; border-left: 1px solid rgba(242, 244, 247, .75); }
  .cote i::before { left: 0; } .cote i::after { right: 0; }
  .cote b { position: relative; padding: 0 4px; border-radius: 2px; background: #15181D; font: 700 10.5px/1.3 var(--mono); color: #F2F4F7; font-variant-numeric: tabular-nums; white-space: nowrap; }
  .cote.est b { color: #AEB6C0; font-style: italic; }
  .sres .cote b { background: #101317; color: #D3D7E2; }
  .sres.over .cote b { color: #FF9C93; }
  .s-total { display: flex; align-items: center; gap: 8px; height: 22px; }
  .s-total .cote { flex: 1; width: auto; height: 22px; }
  .s-total .cote b { background: #2A2E34; }
  .s-end { font: 600 10.5px/1 var(--font); letter-spacing: .05em; text-transform: uppercase; color: #AEB6C0; }

  /* Ce que porte une étagère : posé sur le plateau, de gauche à droite. */
  .shelf-load { flex: 1 1 0; min-width: 0; align-self: stretch; display: flex; gap: 4px; justify-content: flex-start; align-items: flex-end; padding: 0 2px 5px; overflow: hidden; }
  .sh-it { cursor: pointer; }
  .sh-it:hover { filter: brightness(1.15); }
  .sh-it.on { outline: 1.5px solid var(--accent); outline-offset: 1px; }
  .sh-it:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .sh-it.flash { animation: rk-flash 1.2s ease-out; }
  .sh-more { flex: none; font: 600 10px/1 var(--mono); padding: 3px 5px; border-radius: 3px; color: #fff; background: rgba(0, 0, 0, .45); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, .2); }
  /* Visée hors format rack : les étagères posées s'éclairent (pleines : en retrait). */
  .rshelf { position: absolute; z-index: 3; pointer-events: none; border-radius: 3px; box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--aim) 70%, transparent); background: color-mix(in srgb, var(--aim) 12%, transparent); }
  .rshelf.full { box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--fg-3) 50%, transparent); background: none; }
  .sh-it { flex: none; font: 600 10px/1 var(--font); padding: 3px 6px; border-radius: 3px; color: #fff; background: color-mix(in srgb, var(--c) 70%, #1C1F23); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, .15); max-width: 120px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .circ { display: grid; grid-template-columns: minmax(0, 1fr) auto auto; gap: 8px; align-items: center; padding: 8px 10px; border-radius: 10px; background: var(--surface-2); font-size: 14px; }
  .circ.over { background: var(--err-soft); }
  .circ.over .mono { color: var(--err); font-weight: 700; }
  /* Ajout d'un circuit : le nom sur toute la largeur, puis phase, calibre et bouton. */
  .circ-form { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) auto; gap: 6px; }
  .circ-n { grid-column: 1 / -1; display: grid; gap: 4px; }
  .rk-clear { padding-top: 14px; border-top: 1px solid var(--line); }
  .circ-form { align-items: end; }
  .pdec { flex: 1 1 0; min-width: 0; align-self: stretch; display: block; padding: 2px 0; margin-left: 8px; }
  .pside { display: grid; gap: 8px; justify-items: start; }
  /* Vignette de façade (panneau de droite) : l'équipement tel qu'il est dans le rack, réduit. */
  .ithumb { width: 100%; display: flex; justify-content: center; }
  .ithumb-in { position: relative; flex: none; margin: 0 12px; }
  .ithumb-in .ritem { position: absolute; cursor: default; }
  .ithumb-in .ritem:hover { filter: none; }
  /* Mobilier (casquette) : l'éditeur suit la hauteur que lui donne la page. */
  .rack-screen:not(.solo) .design-body { height: calc(100dvh - 216px); min-height: 560px; }
  @media (max-width: 1100px) { .rack-screen:not(.solo) .design-body { grid-template-columns: minmax(0, 1fr); height: auto; } }
  .sheet-h { display: none; }
  @media (max-width: 860px) { .rk-stagebox { order: 1; } }
  @media (max-width: 767px) {
    /* « À placer » : panneau fixé en bas, replié sur sa barre de titre. */
    .pane.left { position: fixed; z-index: 30; left: 0; right: 0; bottom: 0; order: 9; gap: 0; padding: 0 16px calc(10px + env(safe-area-inset-bottom, 0px)); max-height: 55vh; max-height: 55dvh; overflow: auto; border-radius: 16px 16px 0 0; background: var(--surface); box-shadow: 0 -10px 30px -12px rgba(0, 0, 0, .35), 0 -1px 0 var(--line); }
    .pane.left:not(.open) > section { display: none; }
    .sheet-h { position: sticky; top: 0; z-index: 1; display: flex; align-items: center; gap: 10px; width: 100%; min-height: 48px; padding: 6px 0; border: 0; background: var(--surface); color: var(--fg); text-align: left; font-size: 15px; }
    .sheet-h > span:first-child { flex: none; }
    .sheet-armed { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12.5px; font-weight: 600; color: var(--accent); }
    .sheet-h :global(.ic) { margin-left: auto; transform: rotate(180deg); transition: transform .2s; }
    .pane.left.open .sheet-h :global(.ic) { transform: none; }
    .sheet-t { display: none; }
    .pend.mini { min-height: 44px; }
    /* Place pour le panneau en bas. */
    .rack-screen .design-body { padding-bottom: 64px; }
  }
  .rtx { display: inline-flex; align-items: center; gap: 8px; }
  .rtx .input { width: auto; }
  .vp-row { display: grid; grid-template-columns: minmax(0, 1fr) 150px; gap: 8px; align-items: center; padding: 6px 0; border-bottom: 1px solid var(--line); }
  .vp-row .input { width: 100%; }
  .vp-n { display: flex; flex-direction: column; line-height: 1.25; min-width: 0; }
  .vp-n small { color: var(--fg-3); font-size: 12px; }
  .vp-row.over .vp-n small { color: var(--err); }
  /* PDU vertical : bandeau étroit, disjoncteur en tête, prises réparties sur la longueur. */
  .rack.vpl { padding-left: 26px; }
  .rack.vpr { padding-right: 26px; }
  .vpdu { position: absolute; z-index: 2; width: 16px; border-radius: 3px; background: #444A52; box-shadow: inset 0 0 0 1px #000, 0 0 0 1px rgba(255, 255, 255, .08); }
  .vpdu::before { content: ""; position: absolute; left: 4px; right: 4px; top: 5px; height: 9px; border-radius: 1px; background: linear-gradient(180deg, #A9B0B8 0 45%, #2A2E33 45%); }
  .vpdu i { position: absolute; left: 3px; right: 3px; top: 20px; bottom: 6px; border-radius: 1px; background: repeating-linear-gradient(180deg, #08090A 0 8px, transparent 8px 12px); }
</style>
