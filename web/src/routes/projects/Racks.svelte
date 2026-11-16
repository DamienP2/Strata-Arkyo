<script lang="ts">
  // Élévations de racks (section 9) : écran de conception, une personne à la fois.
  import { untrack, onDestroy, tick } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Vu from '../../components/Vu.svelte';
  import Seg from '../../components/Seg.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Loading from '../../components/Loading.svelte';
  import { api } from '../../lib/api';
  import { ask } from '../../lib/ask.svelte';
  import { canvasKeys } from '../../lib/canvaskeys';
  import { can, session } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { route, navigate } from '../../lib/router.svelte';
  import { join } from '../../lib/live.svelte';
  import { nf, relative, pl } from '../../lib/format';
  import { refById, loadRefs, familyColor } from '../../lib/catalog.svelte';
  import { rackLines, rackText, setRackText, modelOf, RACK_TEXT, type RackText } from '../../lib/racktext.svelte';
  import { faceType, ALU, decor, accFace, visualOf } from '../../lib/faceplate';
  import { CONTAINER_KINDS, type Accessory, type Article, type Instance, type ProjectView } from '../../lib/projects';
  import PanelFace from '../../components/PanelFace.svelte';
  import ProjectPanel from './ProjectPanel.svelte';
  import Rack3D from '../../components/Rack3D.svelte';
  import AllRacks from './AllRacks.svelte';
  import RackPicker from '../../components/RackPicker.svelte';
  import AddArticle from './AddArticle.svelte';
  import Inspector from '../../components/Inspector.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import { ApiError } from '../../lib/api';
  import { blankClick } from '../../lib/blankclick';

  // `fixed` : éditeur d'un seul élément, intégré à une autre page (casquette d'un mobilier).
  let { view, fixed = null }: { view: ProjectView; fixed?: string | null } = $props();
  type Face = 'front' | 'rear' | 'rear_inner';
  type W = 'full' | 'half' | 'third' | 'quarter';
  interface Placement { id: string; article_id: string; instance_id: string | null; face: Face; u_start: number; width: W; col: number; circuit_id: string | null; shelf_id: string | null; rev: number }
  interface RackEl { placement: Placement; label: string; tag: string | null; custom?: string | null; nature: string; height_u: number; depth_mm: number | null; rear_reserve_mm: number | null; weight_kg: number | null; power_w: number | null; function: string | null; category: string | null; brand: string | null; accessory: Accessory | null; article_rev: number; type_id?: string | null; visual?: string | null; item_id?: string | null }
  interface Pending { article_id: string; instance_id?: string; tag?: string; label: string; nature: string; height_u: number | null; width: W | null; rackable: boolean; function: string | null; accessory: Accessory | null; type_id?: string | null; visual?: string | null }
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
  const cid = $derived(fixed ?? (all ? null : (route.path.split('/')[4] ?? racks[0]?.id ?? null)));
  $effect(() => { if (!fixed && !route.path.split('/')[4] && racks[0]) navigate(`/projets/${view.project.id}/racks/${racks[0].id}`, true); });

  let rack = $state<Rack | null>(null);
  let face = $state<Face | 'side'>('front');
  let mode = $state<'real' | 'schematic'>('real');
  let show3d = $state(false);
  let armed = $state<Pending | null>(null);
  let sel = $state<string | null>(null);
  let newer = $state(false);
  let requested = $state<string | null>(null);
  // « Ajouter du matériel » : la boîte de l'inventaire, réglée sur l'élément affiché.
  let adding = $state(false);
  const canAdd = $derived(can('projets', 'ecriture'));
  const shownC = $derived(view.containers.find((c) => c.id === cid) ?? null);
  const canDesign = $derived(can('conception', 'ecriture'));
  const mine = $derived(!!rack?.lock?.mine);
  // Proportions réelles d'un 19" : façade 482,6 mm pour 44,45 mm par U (≈ 10,86).
  const U = 34;
  const BW = Math.round(U * 10.86) - 22;
  const units = $derived(rack?.container.height_u ?? 0);
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
  let sheetOpen = $state(false);
  // Ordinateur : « Ajuster à la hauteur » met toute la baie dans la hauteur disponible (réglage mémorisé).
  let fitH = $state((() => { try { return localStorage.getItem('rack-fit') === '1'; } catch { return false; } })());
  $effect(() => { try { localStorage.setItem('rack-fit', fitH ? '1' : '0'); } catch { /* indisponible */ } });
  const zoom = $derived.by(() => {
    if (narrow && stageW) {
      const vp = (vpOf().some((v) => v.accessory.side === 'left') ? 26 : 0) + (vpOf().some((v) => v.accessory.side === 'right') ? 26 : 0);
      const natural = 2 * 28 + BW + 22 + 10 + vp;
      return Math.min(1, (stageW - 24) / natural);
    }
    // Titre de la baie, marges et graduations : environ 90 px en plus des U.
    if (fitH && stageH && units) return Math.max(0.3, Math.min(1, (stageH - 40) / (units * U + 90)));
    return 1;
  });
  const cols = (w: W) => ({ full: 1, half: 2, third: 3, quarter: 4 })[w];

  async function load() {
    if (!cid) return;
    try {
      rack = await api.get<Rack>(`/containers/${cid}/rack`); newer = false;
      // L'élément armé suit le rechargement (mêmes données, nouvel objet).
      if (armed) { const k = armed.instance_id ?? armed.article_id; armed = rack.pending.find((x) => (x.instance_id ?? x.article_id) === k) ?? null; }
    } catch (e) { fail(e); }
  }

  // Verrou : prise de main à l'ouverture, battement toutes les 20 s, libération à la sortie
  // ou après 10 minutes sans action.
  let beat: ReturnType<typeof setInterval> | null = null;
  let lastAction = Date.now();
  async function take() {
    if (!cid || !canDesign) return;
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
    if (!destroyed && id === cid) await load();
  }
  async function requestHand() {
    try { await api.post(`/containers/${cid}/rack/lock/request`); toast(`Demande envoyée à ${rack?.lock?.account_name}.`, 'info'); } catch (e) { fail(e); }
  }

  let current: string | null = null;
  $effect(() => {
    const id = cid;
    if (!id) return;
    untrack(() => {
      if (current && current !== id) release(current);
      current = id; rack = null; sel = null; armed = null; requested = null;
      load().then(take);
    });
  });
  $effect(() => join(`project:${view.project.id}`, (c) => {
    if (c.id !== cid) return;
    if (c.kind === 'rack' && c.action === 'saved' && !mine) newer = true;
    if (c.kind === 'lock' && c.action === 'request' && mine) requested = c.actor;
    // Main prise ailleurs (y compris par soi-même dans un autre onglet) : cet onglet passe en lecture.
    if (c.kind === 'lock' && c.action === 'taken') { if (mine) stopBeat(); load(); }
    if (c.kind === 'lock' && c.action === 'released' && !mine) load();
  }));
  const leave = () => { if (current && beat) fetch(`/api/containers/${current}/rack/lock`, { method: 'DELETE', headers: { 'X-Strata-Request': '1' }, keepalive: true }); };
  onDestroy(() => { destroyed = true; stopBeat(); if (current) release(current); });

  // ?i=<exemplaire> (lien « Poser » de l'inventaire) : l'exemplaire posé est sélectionné et montré,
  // sinon il est armé dans « À placer », prêt à poser.
  let handledI: string | null = null;
  $effect(() => {
    const iid = route.query.get('i'), r = rack, at = cid;
    if (!iid || !r || !at || handledI === `${at}:${iid}`) return;
    untrack(() => {
      handledI = `${at}:${iid}`;
      const el = r.elements.find((e) => e.placement.instance_id === iid);
      if (el) {
        armed = null; sel = el.placement.id;
        if (!el.placement.shelf_id && face !== el.placement.face && face !== 'side') face = el.placement.face;
        show3d = false;
        tick().then(() => document.querySelector<HTMLElement>(`.rack-screen .ritem[data-pl="${CSS.escape(el.placement.shelf_id ?? el.placement.id)}"]`)?.scrollIntoView({ block: 'center', behavior: 'smooth' }));
        return;
      }
      const p = r.pending.find((x) => x.instance_id === iid);
      if (p?.rackable) { sel = null; if (face === 'side') face = 'front'; show3d = false; arm(p, true); }
      else if (p) toast(`« ${p.tag ?? p.label} » est hors format rack : posez-le sur une étagère du rack.`, 'info');
    });
  });

  // ----- Pose et déplacement -----
  // Ordre du document = ordre visuel, du haut vers le bas (parcours au clavier).
  const faceEls = $derived((rack?.elements.filter((e) => e.placement.face === face && !e.placement.shelf_id) ?? []).sort((a, b) => b.placement.u_start - a.placement.u_start || a.placement.col - b.placement.col));
  const onShelf = (id: string) => rack?.elements.filter((e) => e.placement.shelf_id === id) ?? [];
  const isShelf = (e: RackEl) => e.accessory?.kind === 'shelf';
  const panelOf = (e: { accessory: Accessory | null }) => (e.accessory?.kind === 'panel' ? e.accessory.panel : null);
  let editPanel = $state<RackEl | null>(null);
  const shelfOf = (e: RackEl) => rack?.elements.find((x) => x.placement.id === e.placement.shelf_id);
  async function putOnShelf(shelf: RackEl, p: Pending) {
    lastAction = Date.now();
    try { await api.post(`/containers/${cid}/placements`, { article_id: p.article_id, instance_id: p.instance_id ?? null, face: shelf.placement.face, u_start: shelf.placement.u_start, shelf_id: shelf.placement.id }); await load(); }
    catch (e) { fail(e); }
  }
  const selected = $derived(rack?.elements.find((e) => e.placement.id === sel) ?? null);
  let placing = false;
  async function placeAt(u: number, col: number, p: Pending | null = armed) {
    // Une pose à la fois : le clic suivant attend la réponse du serveur.
    if (!p || !mine || face === 'side' || placing) return;
    lastAction = Date.now();
    const wasArmed = armed === p;
    placing = true;
    try {
      const r = await api.post<{ adjusted: boolean; warning: string | null; placement: Placement }>(`/containers/${cid}/placements`, { article_id: p.article_id, instance_id: p.instance_id ?? null, face, u_start: u, col });
      const id = r.placement.id, h = Math.max(1, Math.ceil(p.height_u ?? 1));
      toast(`« ${p.tag ?? p.label} » posé en ${rangeTxt(r.placement.u_start, h)}.`, 'ok', { run: async () => { await api.del(`/placements/${id}`); if (sel === id) sel = null; await load(); } });
      if (r.warning) toast(r.warning, 'warn');
      armed = null; aim = null; sel = id; await load(); flash(id);
      const next = wasArmed ? rack?.pending.find((x) => x.article_id === p.article_id && x.rackable) : null;
      if (next) armed = next;
    } catch (e) { fail(e); } finally { placing = false; }
  }
  async function patch(e: RackEl, change: Partial<Placement>) {
    lastAction = Date.now();
    const p = { ...e.placement, ...change };
    try { await api.patch(`/placements/${p.id}`, { rev: e.placement.rev, face: p.face, u_start: p.u_start, width: p.width, col: p.col, circuit_id: p.circuit_id }); await load(); }
    catch (x) { fail(x); await load(); }
  }
  /** Retire un équipement du rack (ou de l'étagère) : il redevient « à placer ». Notification avec
   *  Annuler (reposé au même U, à la même face, à la même colonne), focus sur son entrée « À placer ». */
  async function unplace(e: RackEl) {
    lastAction = Date.now();
    const p = e.placement, key = p.instance_id ?? p.article_id, where = p.shelf_id ? 'de l’étagère' : 'du rack';
    try {
      await api.del(`/placements/${p.id}`); sel = null; await load();
      toast(`« ${e.tag ?? e.label} » retiré ${where} : il est à placer.`, 'ok', { run: async () => {
        await api.post(`/containers/${cid}/placements`, { article_id: p.article_id, instance_id: p.instance_id, face: p.face, u_start: p.u_start, width: p.width, col: p.col, circuit_id: p.circuit_id, shelf_id: p.shelf_id });
        await load();
      } });
      await tick();
      (document.querySelector<HTMLElement>(`[data-pend="${CSS.escape(key)}"]`) ?? document.querySelector<HTMLElement>('.rack-screen .rack-body'))?.focus();
    } catch (x) { fail(x); }
  }
  // ----- Panneau de l'équipement sélectionné : position et libellé modifiables -----
  /** Place libre et dans les limites ? Renvoie le message d'erreur, sinon null. */
  function checkSpot(e: RackEl, u: number, f: Face, col = e.placement.col): string | null {
    const h = Math.ceil(e.height_u || 1), top = units - h + 1;
    if (!Number.isInteger(u)) return 'Un numéro d’U entier est attendu.';
    if (top < 1) return 'L’équipement est plus haut que le rack.';
    if (u < 1 || u > top) return `Entre U1 et U${top} pour cet équipement de ${fmtU(e.height_u)} U.`;
    const hit = blocker(h, e.placement.width, col, u, f, e.placement.id);
    return hit ? `Position occupée par « ${hit.custom || hit.label} ».` : null;
  }
  const nameOf = (e: RackEl) => e.custom || e.tag || e.label;
  const rangeTxt = (u: number, h: number) => (h > 1 ? `U${u}–U${u + h - 1}` : `U${u}`);
  /** Élément posé qui occupe déjà une partie de la place visée (même face, colonnes qui se chevauchent). */
  function blocker(h: number, w: W, col: number, u: number, f: Face, skip?: string): RackEl | null {
    const n = cols(w), a0 = col / n, a1 = (col + 1) / n;
    return rack?.elements.find((o) => {
      if (o.placement.id === skip || o.placement.shelf_id || o.placement.face !== f) return false;
      const m = cols(o.placement.width), b0 = o.placement.col / m, b1 = (o.placement.col + 1) / m;
      const ou = o.placement.u_start, oh = Math.ceil(o.height_u || 1);
      return u < ou + oh && ou < u + h && a0 < b1 - 1e-9 && b0 < a1 - 1e-9;
    }) ?? null;
  }
  let uDraft = $state<number | null>(null);
  let posErr = $state<{ field: 'u' | 'face'; message: string } | null>(null);
  let labelDraft = $state('');
  let labelErr = $state<string | null>(null);
  // Brouillons repris à chaque changement de sélection ou rechargement de l'élévation.
  $effect(() => {
    const e = selected;
    untrack(() => { uDraft = e?.placement.u_start ?? null; labelDraft = e?.custom ?? ''; });
  });
  $effect(() => { void sel; untrack(() => { posErr = null; labelErr = null; }); });
  /** Déplace l'équipement (même appel que le glisser) ; erreur affichée sous le champ, toast avec Annuler. */
  async function moveTo(e: RackEl, change: { u_start?: number; face?: Face; col?: number }, field: 'u' | 'face') {
    const p = e.placement, u = change.u_start ?? p.u_start, f = change.face ?? p.face, col = change.col ?? p.col;
    if (u === p.u_start && f === p.face && col === p.col) { posErr = null; return; }
    const bad = checkSpot(e, u, f, col);
    if (bad) { posErr = { field, message: bad }; return; }
    posErr = null;
    lastAction = Date.now();
    const name = e.custom || e.tag || e.label;
    try {
      await api.patch(`/placements/${p.id}`, { rev: p.rev, face: f, u_start: u, width: p.width, col, circuit_id: p.circuit_id });
      await load();
      // Changement de face : la vue suit l'équipement.
      if (f !== p.face && face !== 'side') face = f;
      flash(p.id);
      toast(`« ${name} » déplacé en ${rangeTxt(u, Math.ceil(e.height_u || 1))}${f !== p.face ? `, face ${FACES[f].toLowerCase()}` : ''}.`, 'ok', { run: async () => {
        const now = rack?.elements.find((x) => x.placement.id === p.id);
        if (!now) return false;
        await api.patch(`/placements/${p.id}`, { rev: now.placement.rev, face: p.face, u_start: p.u_start, width: p.width, col: p.col, circuit_id: now.placement.circuit_id });
        await load();
      } });
    } catch (x) {
      const m = x instanceof ApiError ? (x.field('u_start') ?? x.field('face')) : undefined;
      if (m) posErr = { field, message: m }; else fail(x);
      await load();
    }
  }
  function commitU(e: RackEl) {
    const u = uDraft;
    if (u === e.placement.u_start) { posErr = null; return; }
    if (u == null || Number.isNaN(u)) { posErr = { field: 'u', message: 'Un numéro d’U est attendu, par exemple 12.' }; return; }
    moveTo(e, { u_start: u }, 'u');
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
      await load();
      toast(next ? `Libellé de « ${i.tag} » enregistré.` : `Libellé de « ${i.tag} » retiré.`, 'ok', { run: async () => { await saveLabel(i, before); await load(); } });
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
  // ----- Visée : fantôme de l'équipement à poser ou à déplacer, aimanté au U entier -----
  // Sujet : équipement glissé depuis le rack (déplacement), glissé depuis « À placer », ou armé (clic, clavier).
  let dragging = $state<RackEl | null>(null);
  let dragP = $state<Pending | null>(null);
  /** U de départ visé et colonne (bas de l'équipement). */
  let aim = $state<{ u: number; col: number } | null>(null);
  /** Écart, en U, entre le bas de l'équipement et le pointeur (le fantôme suit la prise). */
  let grabOff = 0;
  type Subject = { move: RackEl; p?: undefined } | { p: Pending; move?: undefined };
  const subject = $derived<Subject | null>(dragging ? { move: dragging } : dragP ? { p: dragP } : armed ? { p: armed } : null);
  const subjH = (s: Subject) => Math.max(1, Math.ceil((s.move ? s.move.height_u : s.p.height_u) ?? 1));
  const subjW = (s: Subject): W => (s.move ? s.move.placement.width : (s.p.width ?? 'full'));
  /** Place visée : hauteur, colonne, plage « U36–U39 » et raison courte si la pose est impossible. */
  function previewAt(s: Subject | null, at: { u: number; col: number } | null) {
    if (!s || !at || face === 'side' || show3d) return null;
    const h = subjH(s), w = subjW(s), n = cols(w), col = Math.min(n - 1, Math.max(0, at.col)), u = at.u;
    let why: string | null = null;
    if (u + h - 1 > units) why = 'Dépasse le haut du rack';
    else if (u < 1) why = 'Dépasse le bas du rack';
    else { const b = blocker(h, w, col, u, face as Face, s.move?.placement.id); if (b) why = `Occupé par ${b.custom || b.label}`; }
    const same = !!s.move && s.move.placement.face === face && s.move.placement.u_start === u && s.move.placement.col === col;
    return { u, h, n, col, why, same, name: s.move ? (s.move.custom || s.move.label) : s.p.label, range: rangeTxt(u, h) };
  }
  const preview = $derived(previewAt(subject, aim));
  /** U visé par le pointeur (aimanté au U entier), moins la prise ; colonne d'après la largeur du sujet. */
  function aimFromEvent(ev: MouseEvent, el: HTMLElement, s: Subject | null = subject) {
    const r = el.getBoundingClientRect();
    const under = Math.max(1, Math.min(units, Math.floor((r.bottom - ev.clientY) / (U * (r.height / (units * U) || 1))) + 1));
    const n = s ? cols(subjW(s)) : 1;
    const col = Math.min(n - 1, Math.max(0, Math.floor(((ev.clientX - r.left) / r.width) * n)));
    return { u: under - grabOff, col };
  }
  // Numéros de U de la zone visée, et bandes libres de la face affichée pendant une visée.
  const hotU = (n: number) => !!preview && n >= Math.max(1, preview.u) && n <= Math.min(units, preview.u + preview.h - 1);
  const freeRuns = $derived.by(() => {
    if (!subject || face === 'side' || !rack) return [];
    const busy = new Set<number>();
    for (const e of faceEls) if (!(subject.move && e.placement.id === subject.move.placement.id)) for (let k = 0; k < Math.ceil(e.height_u || 1); k++) busy.add(e.placement.u_start + k);
    const out: { u: number; n: number }[] = [];
    for (let u = 1; u <= units; u++) if (!busy.has(u)) { const last = out[out.length - 1]; if (last && last.u + last.n === u) last.n++; else out.push({ u, n: 1 }); }
    return out;
  });
  // Pose réussie : l'équipement clignote (accent) ; place impossible : le fantôme tremble.
  let flashId = $state<string | null>(null);
  function flash(id: string) { flashId = id; setTimeout(() => { if (flashId === id) flashId = null; }, 1300); }
  let shake = $state(false);
  function reject(why: string) {
    toast(`Impossible de poser ici : ${why.charAt(0).toLowerCase()}${why.slice(1)}.`, 'warn');
    shake = true; setTimeout(() => (shake = false), 420);
  }
  /** Valide la visée : pose, déplacement, ou refus indiqué (rien n'est écrit). */
  function commit(s: Subject | null, at: { u: number; col: number } | null) {
    const pv = previewAt(s, at);
    if (!s || !pv) return;
    if (pv.why) { reject(pv.why); return; }
    if (s.move) { if (!pv.same) { sel = s.move.placement.id; moveTo(s.move, { u_start: pv.u, face: face as Face, col: pv.col }, 'u'); } return; }
    placeAt(pv.u, pv.col, s.p);
  }
  // Image de glisser transparente : seul le fantôme aimanté montre la place.
  const BLANK = typeof Image !== 'undefined' ? Object.assign(new Image(), { src: 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7' }) : null;
  function startMove(ev: DragEvent, e: RackEl) {
    const r = (ev.currentTarget as HTMLElement).getBoundingClientRect(), h = Math.max(1, Math.ceil(e.height_u || 1));
    grabOff = Math.max(0, Math.min(h - 1, Math.floor(((r.bottom - ev.clientY) / (r.height || 1)) * h)));
    dragging = e; armed = null; aim = null;
    if (ev.dataTransfer) { ev.dataTransfer.effectAllowed = 'move'; ev.dataTransfer.setData('text/plain', nameOf(e)); if (BLANK) ev.dataTransfer.setDragImage(BLANK, 0, 0); }
  }
  function startPend(ev: DragEvent, p: Pending) {
    grabOff = Math.floor(Math.max(1, Math.ceil(p.height_u ?? 1)) / 2);
    dragP = p; armed = null; aim = null;
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
  function dragOver(ev: DragEvent) {
    if (!mine || !(dragging || dragP) || face === 'side') return;
    ev.preventDefault();
    aim = aimFromEvent(ev, ev.currentTarget as HTMLElement);
    leftAt = 0;
    const pv = previewAt(subject, aim);
    lastBad = pv?.why ?? null;
    if (ev.dataTransfer) ev.dataTransfer.dropEffect = pv?.why ? 'none' : 'move';
  }
  function drop(ev: DragEvent, el: HTMLElement) {
    ev.preventDefault();
    const s = subject, at = aimFromEvent(ev, el, s);
    lastBad = null;
    commit(s, at);
  }

  // ----- Circuits -----
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
  /** Textes : en schématique, les accessoires gardent leur nom (pas de façade dessinée). */
  const rl = (e: RackEl): [string, string] => (mode === 'schematic' && e.nature === 'accessory' ? [e.label, ''] : rackLines(e));
  /** PDU verticaux visibles : seulement en vue arrière. */
  const vpOf = (): VPdu[] => (face !== 'front' && face !== 'side' ? (rack?.vpdus ?? []) : []);
  const typeOf = (e: RackEl) => (e.nature === 'accessory' ? accFace(e as any) : visualOf(e));
  const dw = (e: RackEl) => !!rack?.warnings.some((w) => w.placement_id === e.placement.id);
  // Vue de côté : corps (profondeur) et réserve de câbles, avant depuis la gauche, arrière depuis la droite.
  const depth = $derived(rack?.container.depth_mm ?? 600);
  const SW = 300;
  const sideBars = $derived((rack?.elements ?? []).map((e) => {
    const d = e.depth_mm ?? (e.nature === 'accessory' ? 5 : depth * (e.placement.face === 'front' ? 0.45 : 0.3));
    const res = e.nature === 'accessory' ? 0 : (e.rear_reserve_mm ?? 80);
    const front = e.placement.face === 'front';
    const over = d + res > depth;
    return { e, x: front ? 0 : SW - (d / depth) * SW, w: (d / depth) * SW, rx: front ? (d / depth) * SW : SW - ((d + res) / depth) * SW, rw: (res / depth) * SW, over, front };
  }));
  const wanted = $derived(rack?.pending.filter((p) => p.rackable) ?? []);
  const notRackable = $derived(rack?.pending.filter((p) => !p.rackable) ?? []);
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
  const wantedGroups = $derived.by(() => {
    const m = new Map<string, { key: string; label: string; color: string; order: number; list: Pending[] }>();
    for (const p of wanted) { const c = catOf(p); if (!m.has(c.key)) m.set(c.key, { ...c, list: [] }); m.get(c.key)!.list.push(p); }
    for (const g of m.values()) g.list.sort((a, b) => a.label.localeCompare(b.label, 'fr') || (a.tag ?? '').localeCompare(b.tag ?? ''));
    return [...m.values()].sort((a, b) => a.order - b.order || a.label.localeCompare(b.label, 'fr'));
  });
  const FACES: Record<Face | 'side', string> = { front: 'Avant', rear: 'Arrière', rear_inner: 'Arrière intérieure', side: 'Côté' };

  // ----- Occupation : U réellement posés (pas le matériel affecté encore à placer) -----
  /** U posés sur une face (hors étagères), au prorata de la largeur (½, ⅓, ¼). */
  const posedOn = (f: Face) => (rack?.elements ?? []).filter((e) => e.placement.face === f && !e.placement.shelf_id).reduce((n, e) => n + e.height_u / cols(e.placement.width), 0);
  const posedU = $derived(rack ? Math.max(0, ...rack.faces.map(posedOn)) : 0);
  const pendingU = $derived(wanted.reduce((n, p) => n + (p.height_u ?? 0), 0));
  const fmtU = (u: number) => nf(u, u % 1 ? 1 : 0);

  // ----- Clavier -----
  // Échap remonte d'un seul niveau (équipement armé, puis sélection) ; Suppr retire l'équipement sélectionné (annulable).
  $effect(() => canvasKeys({
    onEscape: () => { if (armed) { armed = null; aim = null; } else if (sel) sel = null; },
    onDelete: () => { if (mine && selected) unplace(selected); },
  }));
  // Clic dans le vide (fond du dessin, zone vide, hors du rack) : désélectionne. Pas pendant une pose.
  $effect(() => blankClick({ enabled: () => !!sel && !armed && !dragging && !dragP, onBlank: () => (sel = null) }));
  /** Pose au clavier : une fois l'équipement armé, flèches haut/bas pour choisir le U, Entrée pour poser. */
  function rackKey(e: KeyboardEvent) {
    if (!armed || face === 'side') return;
    const h = Math.ceil(armed.height_u ?? 1), top = Math.max(1, units - h + 1);
    const at = aim ?? { u: top, col: 0 };
    const n = armed.width ? cols(armed.width) : 1;
    if (e.key === 'ArrowUp') aim = { ...at, u: Math.min(top, at.u + (e.shiftKey ? 5 : 1)) };
    else if (e.key === 'ArrowDown') aim = { ...at, u: Math.max(1, at.u - (e.shiftKey ? 5 : 1)) };
    else if (e.key === 'ArrowLeft' && n > 1) aim = { ...at, col: Math.max(0, at.col - 1) };
    else if (e.key === 'ArrowRight' && n > 1) aim = { ...at, col: Math.min(n - 1, at.col + 1) };
    else if (e.key === 'Enter' || e.key === ' ') commit(subject, at);
    else return;
    e.preventDefault();
  }
  async function arm(p: Pending, force = false) {
    armed = !force && armed === p ? null : p;
    aim = null; grabOff = armed ? Math.floor(Math.max(1, Math.ceil(armed.height_u ?? 1)) / 2) : 0;
    if (armed && narrow) sheetOpen = false;
    if (armed) { await tick(); document.querySelector<HTMLElement>('.rack-screen .rack-body')?.focus(); }
  }
  const RENDERS = [{ value: 'real', label: 'Réel' }, { value: 'schematic', label: 'Schématique' }, { value: '3d', label: '3D', icon: 'box' }] as const;
</script>

<svelte:window onbeforeunload={leave} />

{#if all && racks.length}
  <AllRacks {view} loc={route.path.split('/')[5] ?? null} />
{:else if !racks.length && !fixed}
  <EmptyState icon="server" message="Aucun rack dans ce projet" hint="Ajoutez un rack ou un flight-case depuis l’inventaire.">
    {#snippet action()}<a class="btn primary" href="/projets/{view.project.id}/inventaire"><Icon name="plus" size="sm" />Ajouter un élément</a>{/snippet}
  </EmptyState>
{:else if rack}
  {@const t = rack.totals}
  <div class="rack-screen card" class:solo={!fixed} style="padding:0;overflow:hidden">
    {#if !mine || requested}<div class="lockbar" class:mine class:other={!mine}>
      {#if mine}
        <span class="lk"><span class="key-i"><Icon name="lock" /></span><span><b>{requested}</b> demande la main.</span></span>
        <span class="sp"><button class="btn sm" onclick={() => { requested = null; release(); }}>Céder la main</button></span>
      {:else if rack.lock}
        <span class="lk"><span class="key-i"><Icon name="lock" /></span><span><b>{rack.lock.account_name}</b> a la main depuis {relative(rack.lock.taken_at)}. Vous voyez la dernière version enregistrée.</span></span>
        <span class="sp">{#if newer}<button class="btn sm" onclick={load}><Icon name="refresh" size="sm" />Nouvelle version : recharger</button>{/if}{#if canDesign}<button class="btn sm" onclick={requestHand}>Demander la main</button>{/if}</span>
      {:else}
        <span class="lk"><span class="key-i"><Icon name="unlock" /></span><span>Personne ne modifie cette élévation.</span></span>
        <span class="sp">{#if canDesign}<button class="btn sm primary" onclick={take}>Prendre la main</button>{:else}<span class="muted small">Lecture seule : votre rôle ne permet pas de concevoir.</span>{/if}</span>
      {/if}
    </div>{/if}
    <div class="dtool">
      {#if !fixed}<RackPicker {view} value={cid ?? ''} onpick={(v) => navigate(`/projets/${view.project.id}/racks/${v.replace(':', '/')}`)} />{/if}
      <!-- En 3D, la face et les textes n'ont pas d'effet : ils sont masqués (le cadrage est dans la vue 3D). -->
      {#if !show3d}<Seg label="Face" value={face} options={[...rack.faces, 'side' as const].map((f) => ({ value: f, label: FACES[f] }))} onchange={(f) => { face = f; armed = null; aim = null; }} />{/if}
      <Seg label="Rendu" value={show3d ? '3d' : mode} options={[...RENDERS]} onchange={(v) => { if (v === '3d') { show3d = true; armed = null; } else { mode = v; show3d = false; } }} />
      {#if !show3d}<label class="rtx" title="Textes affichés sur les équipements"><span class="flabel">Textes</span><select class="input sm" value={rackText.v} onchange={(e) => setRackText((e.target as HTMLSelectElement).value as RackText)}>{#each Object.entries(RACK_TEXT) as [k, l]}<option value={k}>{l}</option>{/each}</select></label>
        <button class="btn sm ghost" aria-pressed={fitH} title="Ajuster le rack à la hauteur de l’écran" onclick={() => (fitH = !fitH)}><Icon name="fit" size="sm" />Ajuster à la hauteur</button>{/if}
      <span class="gap"></span>
      <span class="meter" class:err={!!t.total_u && posedU > t.total_u} title="U posés dans le rack{pendingU ? ` ; ${fmtU(pendingU)} U de matériel affecté reste à placer` : ''}"><span>U</span><Vu value={(posedU / (t.total_u || 1)) * 100} mode="load" n={16} /><span class="mono">{fmtU(posedU)} / {t.total_u} U</span>{#if pendingU}<span class="muted small mu-pend">+ {fmtU(pendingU)} U à placer</span>{/if}</span>
      <span class="meter" class:err={t.over_weight}><span>Poids</span><Vu value={rack.container.max_weight_kg ? (t.weight_kg / rack.container.max_weight_kg) * 100 : 0} mode="load" n={16} /><span class="mono">{nf(t.weight_kg, 1)}{rack.container.max_weight_kg ? ` / ${nf(rack.container.max_weight_kg)}` : ''} kg</span></span>
      <span class="meter" class:err={t.over_power}><span>Puiss.</span><Vu value={rack.container.max_power_w ? (t.power_w / rack.container.max_power_w) * 100 : 0} mode="load" n={16} /><span class="mono">{nf(t.power_w)}{rack.container.max_power_w ? ` / ${nf(rack.container.max_power_w)}` : ''} W</span></span>
    </div>

    <div class="design-body" class:ro={!mine}>
      <aside class="pane left" class:open={sheetOpen} aria-label="À placer">
        <button class="sheet-h" aria-expanded={sheetOpen} onclick={() => (sheetOpen = !sheetOpen)}>
          <span><b>À placer</b> <span class="muted">· {wanted.length}</span></span>
          {#if armed}<span class="sheet-armed">{armed.label} · touchez une position</span>{/if}
          <Icon name="chevd" size="sm" />
        </button>
        <section>
          <h2 class="sheet-t">À placer <span class="muted">· {wanted.length}</span></h2>
          {#if mine && !show3d}<p class="small muted">Glissez un équipement dans le rack, ou choisissez-le puis sa position (clic, ou flèches et Entrée). Échap pour annuler.</p>{/if}
          {#if canAdd && shownC}<button class="btn sm add-mat" onclick={() => (adding = true)}><Icon name="plus" size="sm" />Ajouter du matériel</button>{/if}
          {#each wantedGroups as g (g.key)}
            <div class="pcat eyebrow"><i style="--c:{g.color}"></i><span>{g.label}</span><b>{g.list.length}</b></div>
          {#each g.list as p (p.instance_id ?? p.article_id)}
            <button class="pend mini" class:on={armed === p} class:dragging={dragP === p} aria-pressed={armed === p} data-pend={p.instance_id ?? p.article_id} disabled={!mine || face === 'side' || show3d} onclick={() => arm(p)}
              draggable={mine && face !== 'side' && !show3d} ondragstart={(ev) => startPend(ev, p)} ondragend={endDrag} title="{p.label}{p.tag ? ` · ${p.tag}` : ''}">
              <i class="pm-c" style="--c:{famColor(p.function)}"></i><span class="pm-l">{p.label}</span>{#if p.tag}<span class="pm-t">{p.tag}</span>{/if}<span class="pm-u">{p.height_u} U{p.width && p.width !== 'full' ? ` · ${p.width === 'half' ? '½' : p.width === 'third' ? '⅓' : '¼'}` : ''}</span>
            </button>
          {/each}
          {:else}<p class="small muted">Tout est posé.</p>{/each}
          {#if notRackable.length}
            <h2 style="margin-top:8px">Sur étagère <span class="muted">· {notRackable.length}</span></h2>
            <p class="small muted">Hors format rack : choisissez une étagère posée dans le rack, puis « Poser ».</p>
            {#each notRackable as p (p.instance_id ?? p.article_id)}<div class="pend mini" style="cursor:default" data-pend={p.instance_id ?? p.article_id} tabindex="-1" title="{p.label}{p.tag ? ` · ${p.tag}` : ''}"><i class="pm-c" style="--c:{famColor(p.function)}"></i><span class="pm-l">{p.label}</span>{#if p.tag}<span class="pm-t">{p.tag}</span>{/if}</div>{/each}
          {/if}
        </section>
      </aside>

      <div class="rack-stage" class:stage3d={show3d} bind:clientWidth={stageW} bind:clientHeight={stageH}>
        {#if show3d}
          <Rack3D container={rack.container} elements={rack.elements} vpdus={rack.vpdus ?? []} title={rack.container.name} />
        {:else if face === 'side'}
          <div class="rack-wrap" style:zoom={zoom < 1 ? zoom : undefined}>
            <span class="rack-title"><Icon name="server" />{rack.container.name} · côté</span>
            <div class="rack" style="--units:{units};--u:{U}px;grid-template-columns:28px auto">
              <div class="rack-scale">{#each Array(units) as _, i}<span>{units - i}</span>{/each}</div>
              <div>
                <div class="side-body" style="--w:{SW}px">
                  {#each sideBars as b (b.e.placement.id)}
                    <span class="sbar" style="--c:{famColor(b.e.function)};bottom:calc({b.e.placement.u_start - 1} * var(--u) + 1px);height:calc({b.e.height_u} * var(--u) - 3px);left:{b.x}px;width:{b.w}px" title="{b.e.label} · {b.e.depth_mm ?? '?'} mm"></span>
                    {#if b.rw > 0}<span class="sres" class:over={b.over} style="bottom:calc({b.e.placement.u_start - 1} * var(--u) + 1px);height:calc({b.e.height_u} * var(--u) - 3px);left:{Math.min(b.rx, b.rx + b.rw)}px;width:{Math.abs(b.rw)}px"></span>{/if}
                  {/each}
                </div>
                <div class="side-legend"><span>Avant</span><span>{depth} mm</span><span>Arrière</span></div>
              </div>
            </div>
          </div>
        {:else}
          <div class="rack-wrap" style:zoom={zoom < 1 ? zoom : undefined}>
            <span class="rack-title"><Icon name="server" />{rack.container.name} · {FACES[face].toLowerCase()}</span>
            <div class="rack" class:placing={!!armed} class:vpl={vpOf().some((v) => v.accessory.side === 'left')} class:vpr={vpOf().some((v) => v.accessory.side === 'right')} style="--units:{units};--u:{U}px;--bw:{BW}px">
              <!-- PDU verticaux : couloirs arrière, gauche et droite vus de l'arrière. -->
              {#each vpOf() as v, i (v.article_id)}
                {@const h = Math.min(units * U, (v.accessory.length_mm / 44.45) * U)}
                {@const left = v.accessory.side === 'left'}
                {@const k = vpOf().filter((x, j) => j < i && x.accessory.side === v.accessory.side).length}
                <div class="vpdu" style="{left ? 'left' : 'right'}:{6 + k * 20}px;bottom:calc(24px + {(units * U - h) / 2}px);height:{h}px;--n:{v.accessory.outlets}" title={v.label}><i></i></div>
              {/each}
              <div class="rack-scale">{#each Array(units) as _, i}<span class:hot={hotU(units - i)} class:no={hotU(units - i) && !!preview?.why}>{units - i}</span>{/each}</div>
              <div class="rack-body" class:aiming={!!subject} class:no={!!preview?.why} role="grid" tabindex={armed ? 0 : -1} aria-label={armed ? `Élévation : position de ${armed.label}${preview ? `, ${preview.range}${preview.why ? `, ${preview.why.toLowerCase()}` : ', libre'}` : ''} (flèches haut et bas, Entrée pour poser)` : 'Élévation'}
                onmousemove={(e) => { if (armed) aim = aimFromEvent(e, e.currentTarget as HTMLElement); }}
                onmouseleave={() => { if (armed) aim = null; }}
                onclick={(e) => { if (armed) commit(subject, aimFromEvent(e, e.currentTarget as HTMLElement)); else if (!(e.target as Element).closest('.ritem')) sel = null; }}
                onkeydown={rackKey}
                ondragover={dragOver}
                ondragleave={(e) => { if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) { aim = null; leftAt = Date.now(); } }}
                ondrop={(e) => drop(e, e.currentTarget as HTMLElement)}>
                {#each freeRuns as r (r.u)}<span class="rfree" style="bottom:calc({r.u - 1} * var(--u));height:calc({r.n} * var(--u))"></span>{/each}
                {#each faceEls as e (e.placement.id)}
                  <div class="ritem {e.nature === 'accessory' ? 'acc' : ''} {ALU.has(typeOf(e)) && mode === 'real' ? 'alu' : ''} {e.height_u <= 1 ? 'u1' : ''} {e.placement.width !== 'full' ? `half c${e.placement.col}` : ''}" class:sel={sel === e.placement.id} class:dw={dw(e)} class:draggable={mine} class:dragging={dragging === e} class:flash={flashId === e.placement.id}
                    style={style(e)} data-pl={e.placement.id} role="button" tabindex="0" draggable={mine} aria-pressed={sel === e.placement.id} aria-label="{e.custom ? `${e.custom} · ` : ''}{e.tag ? `${e.tag} · ` : ''}{e.label} · U{e.placement.u_start}" title="{e.custom ? `${e.custom} · ` : ''}{e.tag ? `${e.tag} · ` : ''}{e.label} · U{e.placement.u_start}"
                    onclick={() => { if (!armed) sel = e.placement.id; }} onkeydown={(k) => { if (k.key === 'Enter' || k.key === ' ') { k.preventDefault(); sel = e.placement.id; } }}
                    ondragstart={(ev) => startMove(ev, e)} ondragend={endDrag}>
                    {#if e.nature !== 'accessory'}<i class="led"></i>{/if}<span class="rl" class:two={!!rl(e)[1]}>{#if rl(e)[0]}<b class="rn" class:id={!!e.tag && rl(e)[0] === e.tag}>{rl(e)[0]}</b>{/if}{#if rl(e)[1]}<small>{rl(e)[1]}</small>{/if}</span>{#if isShelf(e) && onShelf(e.placement.id).length}<span class="shelf-load">{#each onShelf(e.placement.id) as x (x.placement.id)}<span class="sh-it" style="--c:{famColor(x.function)}">{x.label}</span>{/each}</span>{:else if mode === 'real' && panelOf(e)}<span class="pdec"><PanelFace panel={panelOf(e)!} /></span>{:else if mode === 'real'}<span class="dec">{@html decor(typeOf(e))}</span>{:else}<span class="dec"></span>{/if}
                  </div>
                {/each}
                {#if preview && !preview.same}
                  {@const u0 = Math.max(1, preview.u)}
                  {@const u1 = Math.min(units, preview.u + preview.h - 1)}
                  <div class="aim" class:no={!!preview.why} class:shake aria-hidden="true" style="bottom:calc({u0 - 1} * var(--u) + 1px);height:calc({Math.max(1, u1 - u0 + 1)} * var(--u) - 2px);left:calc({(100 * preview.col) / preview.n}% + 2px);width:calc({100 / preview.n}% - 4px)">
                    <span class="aim-n">{preview.name}</span><span class="aim-u">{preview.range}</span>{#if preview.why}<span class="aim-why">{preview.why}</span>{/if}
                  </div>
                {/if}
              </div>
              <div class="rack-scale">{#each Array(units) as _, i}<span class:hot={hotU(units - i)} class:no={hotU(units - i) && !!preview?.why}>{units - i}</span>{/each}</div>
            </div>
          </div>
        {/if}
      </div>

      <aside class="pane right">
        {#if selected}
          {@const p = selected.placement}
          {@const e = selected}
          {@const inst = instOf(e)}
          {@const h = Math.ceil(e.height_u || 1)}
          {@const ports = e.item_id ? portsCache[e.item_id] : null}
          <Inspector eyebrow={e.nature === 'accessory' ? (panelOf(e) ? 'Panel' : 'Accessoire') : 'Équipement'} title={e.custom || e.label}
            id={e.tag} sub={e.custom ? modelOf(e) : e.brand && !e.label.toLowerCase().startsWith(e.brand.toLowerCase()) ? e.brand : null} onclose={() => (sel = null)}>
            {#snippet thumb()}
              <div class="ithumb" bind:clientWidth={thumbW}>
                <div class="ithumb-in" style="--u:{U}px;width:{BW}px;height:{e.height_u * U}px;zoom:{thumbZoom(e)}">
                  <div class="ritem {e.nature === 'accessory' ? 'acc' : ''} {ALU.has(typeOf(e)) && mode === 'real' ? 'alu' : ''} {e.height_u <= 1 ? 'u1' : ''}" style="{style(e).replace(/;?(bottom|height|left|width):[^;]*/g, '')};left:0;right:0;top:0;bottom:0" aria-hidden="true">
                    {#if e.nature !== 'accessory'}<i class="led"></i>{/if}<span class="rl" class:two={!!rl(e)[1]}>{#if rl(e)[0]}<b class="rn" class:id={!!e.tag && rl(e)[0] === e.tag}>{rl(e)[0]}</b>{/if}{#if rl(e)[1]}<small>{rl(e)[1]}</small>{/if}</span>{#if mode === 'real' && panelOf(e)}<span class="pdec"><PanelFace panel={panelOf(e)!} /></span>{:else if mode === 'real'}<span class="dec">{@html decor(typeOf(e))}</span>{:else}<span class="dec"></span>{/if}
                  </div>
                </div>
              </div>
            {/snippet}
            {#each rack.warnings.filter((w) => w.placement_id === p.id) as w}<div class="alert warn"><Icon name="alert" /><span>{w.message}</span></div>{/each}
            <section class="insp-sec">
              <h3>Position</h3>
              {#if p.shelf_id}
                <div class="insp-row"><span class="insp-k">Étagère</span><span class="insp-v">{shelfOf(e)?.custom || shelfOf(e)?.label || 'Étagère'} · U{p.u_start}</span></div>
              {:else}
                <div class="insp-row">
                  <label class="insp-k" for="rk-u">U de départ</label>
                  {#if mine}
                    <span class="insp-v insp-num"><input class="cell mono" id="rk-u" type="number" inputmode="numeric" min="1" max={Math.max(1, units - h + 1)} step="1" bind:value={uDraft} aria-invalid={posErr?.field === 'u' ? 'true' : undefined} aria-describedby={posErr?.field === 'u' ? 'rk-u-err' : 'rk-u-help'}
                      onblur={() => commitU(e)} onkeydown={(k) => cellKey(k, () => { uDraft = p.u_start; posErr = null; })}><span class="u">U</span></span>
                  {:else}<span class="insp-v mono" id="rk-u">U{p.u_start}</span>{/if}
                  {#if posErr?.field === 'u'}<FieldError message={posErr.message} for="rk-u" />{:else}<span class="help" id="rk-u-help">{h > 1 ? `Occupe U${p.u_start} à U${p.u_start + h - 1}.` : `Occupe U${p.u_start}.`}</span>{/if}
                </div>
                <div class="insp-row">
                  <label class="insp-k" for="rk-face">Face</label>
                  {#if mine && rack.faces.length > 1}
                    <span class="insp-v"><select class="cell" id="rk-face" value={p.face} aria-invalid={posErr?.field === 'face' ? 'true' : undefined} aria-describedby={posErr?.field === 'face' ? 'rk-face-err' : undefined}
                      onchange={(ev) => { const f = (ev.target as HTMLSelectElement).value as Face; moveTo(e, { face: f }, 'face'); }}>{#each rack.faces as f}<option value={f}>{FACES[f]}</option>{/each}</select></span>
                  {:else}<span class="insp-v" id="rk-face">{FACES[p.face]}</span>{/if}
                  {#if posErr?.field === 'face'}<FieldError message={posErr.message} for="rk-face" />{/if}
                </div>
              {/if}
              <div class="insp-row"><span class="insp-k">Hauteur</span><span class="insp-v">{fmtU(e.height_u)} U{p.width !== 'full' ? ` · ${p.width === 'half' ? '½' : p.width === 'third' ? '⅓' : '¼'} de largeur, colonne ${p.col + 1}` : ''}</span></div>
              {#if e.nature !== 'accessory' || p.shelf_id}
                <div class="insp-row">
                  <label class="insp-k" for="rk-c">Circuit</label>
                  {#if mine}
                    <span class="insp-v"><select class="cell" id="rk-c" value={p.circuit_id ?? ''} onchange={(ev) => patch(e, { circuit_id: (ev.target as HTMLSelectElement).value || null })}><option value="">Non branché</option>{#each rack.circuits as c}<option value={c.id}>{c.name} · {c.phase} · {c.rating_a} A</option>{/each}</select></span>
                  {:else}<span class="insp-v" id="rk-c">{rack.circuits.find((c) => c.id === p.circuit_id)?.name ?? 'Non branché'}</span>{/if}
                </div>
              {/if}
              {#if p.shelf_id}<p class="insp-note">Posé sur une étagère : il en suit la position.</p>{/if}
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
                {#if ports?.total}<div class="insp-row"><span class="insp-k">Ports</span><span class="insp-v">{portsTxt(ports)}</span></div>{/if}
              </section>
            {/if}
            {#if panelOf(e)}
              {@const pn = panelOf(e)!}
              <section class="insp-sec">
                <h3>Panel</h3>
                <div class="pside"><span class="muted small">{pn.slots.length} connecteur{pn.slots.length > 1 ? 's' : ''} sur {pn.height_u * pn.per_u} · {pn.groups.length} groupe{pn.groups.length > 1 ? 's' : ''}</span>
                  <button class="btn sm" onclick={() => (editPanel = e)}><Icon name="grid" size="sm" />{mine ? 'Modifier le panel' : 'Voir le panel'}</button></div>
              </section>
            {/if}
            {#if isShelf(e)}
              <section class="insp-sec">
                <h3>Sur l’étagère · {onShelf(p.id).length} / 10</h3>
                <div class="stack" style="gap:6px">
                  {#each onShelf(p.id) as x (x.placement.id)}<button class="pend" style="padding:8px 10px" onclick={() => (sel = x.placement.id)}><span class="pend-t"><b>{x.custom || x.label}</b><span>{x.tag ?? ''}</span></span></button>{:else}<p class="insp-note" style="margin:0">Rien n’est posé sur cette étagère.</p>{/each}
                  {#if mine && notRackable.length && onShelf(p.id).length < 10}
                    {#each notRackable as np (np.instance_id ?? np.article_id)}<button class="btn sm" style="justify-content:flex-start" onclick={() => putOnShelf(e, np)}><Icon name="plus" size="sm" />Poser {np.label}{np.tag ? ` · ${np.tag}` : ''}</button>{/each}
                  {/if}
                </div>
              </section>
            {/if}
            {#snippet actions()}
              {#if e.item_id && e.nature !== 'accessory'}<a class="btn sm ghost" href="/catalogue/equipment/{e.item_id}"><Icon name="file" size="sm" />Ouvrir la fiche</a>{/if}
              {#if cid}<a class="btn sm ghost" href="/projets/{view.project.id}/synoptique/element/{cid}"><Icon name="syn" size="sm" />Voir dans le synoptique</a>{/if}
              {#if mine}<button class="btn sm ghost danger" title="{p.shelf_id ? 'Retirer de l’étagère' : 'Retirer du rack'} (Suppr) : il redevient à placer" onclick={() => unplace(e)}><Icon name="x" size="sm" />{p.shelf_id ? 'Retirer de l’étagère' : 'Retirer du rack'}</button>{/if}
            {/snippet}
          </Inspector>
        {:else}
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
          <dl class="kv"><dt>Hauteur posée</dt><dd>{fmtU(posedU)} / {t.total_u} U</dd>{#if pendingU}<dt>À placer</dt><dd>{fmtU(pendingU)} U{t.total_u && posedU + pendingU > t.total_u ? ` · dépasse de ${fmtU(posedU + pendingU - t.total_u)} U une fois tout posé` : ''}</dd>{/if}<dt>Poids</dt><dd>{nf(t.weight_kg, 1)} kg</dd><dt>Puissance</dt><dd>{nf(t.power_w)} W</dd><dt>Dissipation</dt><dd>{nf(t.power_w * 3.412)} BTU/h</dd></dl>
          {#each rack.warnings as w}<div class="alert warn"><Icon name="alert" /><span>{w.message}</span></div>{/each}
          {#if mode === 'schematic' && !show3d && legend.length}<span class="lbl-t">Familles de signal</span><div class="legend" style="grid-template-columns:1fr">{#each legend as [k, name] (k)}<div><span class="sw" style="--c:{famColor(k || null)}"></span>{name}</div>{/each}</div>{/if}
        </section>
        {#if mine && rack.elements.length}
          <section class="rk-clear">
            <button class="btn sm ghost danger" title="Tous les équipements posés redeviennent « à placer » (après confirmation)" onclick={clearRack}><Icon name="undo" size="sm" />Vider le rack</button>
          </section>
        {/if}
        {/if}
      </aside>
    </div>
  </div>
{:else}<Loading label="Chargement du rack" />{/if}

{#if adding && shownC}<AddArticle {view} container={shownC} locationId={shownC.location_id} onclose={() => (adding = false)} ondone={load} />{/if}

{#if editPanel}
  <ProjectPanel article={{ id: editPanel.placement.article_id, rev: editPanel.article_rev, accessory: editPanel.accessory, label: editPanel.label } as Article} readonly={!mine} eyebrow="Panel du rack" onclose={() => (editPanel = null)} ondone={load} />
{/if}

<style>
  .add-mat { align-self: stretch; justify-content: center; margin: 2px 0 6px; }
  .rack-screen :global(.rack-body) { width: var(--bw, 370px); }
  .pcat { display: flex; align-items: center; gap: 8px; margin: 10px 0 2px; padding: 0 2px; }
  .mu-pend { white-space: nowrap; }
  .rack-body:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  /* Visée (pose et déplacement). La baie est un objet sombre fixe : couleurs d'outil claires fixes
     (accent du thème sombre, erreur du thème sombre), lisibles sur le métal dans les deux thèmes. */
  .rack { --aim: #9DB0FF; --aim-no: #FF7B70; }
  .ritem.draggable { cursor: grab; }
  .ritem.draggable:active { cursor: grabbing; }
  .ritem.dragging, .pend.dragging { opacity: .4; }
  .ritem.dragging { outline: 1.5px dashed var(--aim); outline-offset: 1px; filter: none; }
  .rack-body.aiming { cursor: copy; }
  .rack-body.aiming.no { cursor: not-allowed; }
  .rack-body.aiming .ritem { cursor: inherit; }
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
  .aim.shake { animation: aim-shake .4s ease; }
  .ritem.flash { animation: rk-flash 1.2s ease-out; z-index: 3; }
  @keyframes aim-shake { 20%, 60% { transform: translateX(-5px); } 40%, 80% { transform: translateX(5px); } }
  @keyframes rk-flash { 0%, 40% { box-shadow: 0 0 0 2px #fff, 0 0 0 6px var(--aim), 0 0 18px 6px var(--aim); } 100% { box-shadow: 0 0 0 0 transparent; } }
  @media (prefers-reduced-motion: reduce) {
    .aim.shake, .ritem.flash { animation: none; }
    .ritem.flash { outline: 2px solid var(--aim); outline-offset: 2px; }
  }
  .pcat i { width: 8px; height: 8px; border-radius: 50%; background: var(--c); }
  .pcat b { margin-left: auto; font: 600 11px var(--mono); padding: 1px 7px; border-radius: 99px; background: var(--surface-3); color: var(--fg-2); }
  .pend.mini { flex-direction: row; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 8px; min-height: 34px; }
  .pm-c { flex: none; width: 4px; align-self: stretch; border-radius: 2px; background: var(--c); }
  .pm-l { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13.5px !important; font-weight: 600; color: var(--fg) !important; }
  .pm-t { flex: none; font: 600 11px/1 var(--mono) !important; color: var(--fg-3); }
  .pm-u { flex: none; font: 600 11px/1 var(--mono) !important; padding: 3px 6px; border-radius: 5px; background: var(--sunken); color: var(--fg-2) !important; }
  .rack-stage.stage3d { padding: 0; display: block; align-self: stretch; }
  /* Hauteur fixe : la baie défile seule, les panneaux latéraux restent visibles. */
  /* En-tête de projet commun (~90 px), barre du rack et marges de la page : la zone de travail remplit la hauteur sans défilement de page. */
  .rack-screen .design-body { height: calc(100dvh - 216px); min-height: 560px; }
  /* Barre de verrou affichée (49 px : bouton de 30 px + marges) : la zone de travail rétrécit d'autant, la page ne défile pas. */
  @media (min-width: 1101px) { .rack-screen:has(> .lockbar) .design-body { height: calc(100dvh - 265px); } }
  /* Page Racks seule : colonne flexible qui remplit la hauteur visible sous l'en-tête, quelle que soit
     la hauteur de la barre (une ou deux lignes, barre de verrou) : la page ne défile pas. */
  @media (min-width: 1101px) {
    .rack-screen.solo { display: flex; flex-direction: column; height: calc(100dvh - 226px); min-height: 600px; }
    .rack-screen.solo .design-body, .rack-screen.solo:has(> .lockbar) .design-body { flex: 1; height: auto; min-height: 0; }
  }
  .shelf-load { flex: 1 1 0; min-width: 0; display: flex; gap: 4px; justify-content: flex-end; overflow: hidden; }
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
  @media (max-width: 1100px) { .rack-screen .design-body { grid-template-columns: minmax(0, 1fr); height: auto; } }
  .sheet-h { display: none; }
  @media (max-width: 767px) {
    .rack-stage:not(.stage3d) { overflow-x: hidden; }
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
