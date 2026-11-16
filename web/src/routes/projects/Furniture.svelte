<script lang="ts">
  // Mobilier de régie (pupitres, consoles, meubles) : même écran que les racks. En haut, le
  // sélecteur, les vues (face, côté, dessus, casquettes), le rendu et les jauges ; à gauche le
  // matériel « À placer » (sur une casquette ou un poste) ; au centre le dessin aux cotes
  // réelles (casquettes glissées sur le plateau, postes) ; à droite le détail de la casquette
  // ou du poste sélectionné, sinon les cotes et les totaux du mobilier. Le matériel d'une
  // casquette se pose dans l'éditeur de racks (« Équiper »), comme dans une baie.
  import { untrack, tick } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Seg from '../../components/Seg.svelte';
  import Vu from '../../components/Vu.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import NumField from '../../components/NumField.svelte';
  import HeadActions from '../../components/HeadActions.svelte';
  import Desk3D from '../../components/Desk3D.svelte';
  import DeskPicker from '../../components/DeskPicker.svelte';
  import Racks from './Racks.svelte';
  import AddArticle from './AddArticle.svelte';
  import DeskDraw from './DeskDraw.svelte';
  import DeskPost from './DeskPost.svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { ask } from '../../lib/ask.svelte';
  import { canvasKeys } from '../../lib/canvaskeys';
  import { route, navigate } from '../../lib/router.svelte';
  import { familyColor, refById, loadRefs } from '../../lib/catalog.svelte';
  import { rackLines, rackText, setRackText, RACK_TEXT, type RackText } from '../../lib/racktext.svelte';
  import { nf, pl } from '../../lib/format';
  import { ALU, decor, accFace, visualOf } from '../../lib/faceplate';
  import { CONTAINER_KINDS, type Accessory, type Article, type Container, type DeskPosition, type ProjectView } from '../../lib/projects';

  let { view, reload }: { view: ProjectView; reload: () => Promise<void> } = $props();
  loadRefs();
  const pid = $derived(view.project.id);
  const writable = $derived(can('projets', 'ecriture'));
  const canDesign = $derived(can('conception', 'ecriture'));
  const desks = $derived(view.containers.filter((c) => c.kind === 'desk' || c.kind === 'furniture'));
  const cid = $derived(route.path.split('/')[4] ?? null);
  /** Casquette en cours d'équipement : /mobilier/{mobilier}/{casquette}. */
  const equip = $derived(route.path.split('/')[5] ?? null);
  $effect(() => { if (!cid && desks[0]) navigate(`/projets/${pid}/mobilier/${desks[0].id}`, true); });
  const desk = $derived(desks.find((d) => d.id === cid) ?? null);
  const L = $derived(desk?.length_mm ?? 2400), Hd = $derived(desk?.outer_height_mm ?? 750);
  const casq = $derived(desk ? view.containers.filter((c) => c.kind === 'casquette' && c.parent_id === desk.id).sort((a, b) => (a.offset_mm ?? 0) - (b.offset_mm ?? 0)) : []);
  const WC = 532.6; // largeur d'une casquette : 19″ et ses joues (mm)
  const deskHref = (d: string, c?: string) => `/projets/${pid}/mobilier/${d}${c ? `/${c}` : ''}`;

  // ----- Affichage (mémorisé) : vue, rendu, ajustement à la hauteur -----
  type DView = 'front' | 'side' | 'top' | 'casq';
  type Render = 'real' | 'schematic' | '3d';
  const stored = <T extends string>(k: string, ok: readonly T[], d: T): T => { try { const v = localStorage.getItem(k) as T; return ok.includes(v) ? v : d; } catch { return d; } };
  let face = $state<DView>(stored('mob-view', ['front', 'side', 'top', 'casq'] as const, 'front'));
  let render = $state<Render>(stored('mob-render', ['real', 'schematic', '3d'] as const, stored('mob-mode', ['plan', '3d'] as const, 'plan') === '3d' ? '3d' : 'real'));
  let fitH = $state(stored('mob-fit', ['0', '1'] as const, '1') === '1');
  $effect(() => { try { localStorage.setItem('mob-view', face); localStorage.setItem('mob-render', render); localStorage.setItem('mob-fit', fitH ? '1' : '0'); } catch { /* indisponible */ } });
  const show3d = $derived(render === '3d');
  const mode = $derived(render === 'schematic' ? 'schematic' : 'real');
  const VIEWS: { value: DView; label: string }[] = [{ value: 'front', label: 'Face' }, { value: 'side', label: 'Côté' }, { value: 'top', label: 'Dessus' }, { value: 'casq', label: 'Casquettes' }];
  const RENDERS = [{ value: 'real', label: 'Réel' }, { value: 'schematic', label: 'Schématique' }, { value: '3d', label: '3D', icon: 'box' }] as const;

  // ----- Sélection : une casquette ou un poste -----
  let sel = $state<string | null>(null);
  let selP = $state<string | null>(null);
  const selC = $derived(casq.find((c) => c.id === sel) ?? null);
  const posts = $derived<DeskPosition[]>(desk?.kind === 'desk' ? (desk.details?.positions ?? []) : []);
  const selPost = $derived(posts.find((p) => p.id === selP) ?? null);
  function pick(kind: 'c' | 'p', id: string) {
    if (armed || dragged) { deliver(dragged ?? armed!, kind, id); return; }
    if (kind === 'c') { sel = id; selP = null; } else { selP = id; sel = null; }
  }
  const clearSel = () => { sel = null; selP = null; };
  $effect(() => { void cid; untrack(() => { clearSel(); armed = null; }); });

  // ----- Contenu de chaque casquette et du mobilier (même source que l'éditeur de racks) -----
  interface El { placement: { id: string; face: string; u_start: number; width: string; col: number; shelf_id: string | null }; label: string; tag: string | null; custom?: string | null; brand?: string | null; nature: string; height_u: number; function: string | null; accessory: any; item_id: string | null; type_id?: string | null; visual?: string | null; depth_mm?: number | null; weight_kg?: number | null; power_w?: number | null }
  interface Pending { article_id: string; instance_id?: string; tag?: string; label: string; nature: string; height_u: number | null; width: string | null; rackable: boolean; function: string | null; accessory: Accessory | null; type_id?: string | null; visual?: string | null }
  interface Lite { elements: El[]; pending: Pending[] }
  let racks = $state<Record<string, Lite>>({});
  let loaded = $state(false);
  async function loadRacks() {
    if (!desk) return;
    const ids = [desk.id, ...casq.map((c) => c.id)];
    const out: Record<string, Lite> = {};
    await Promise.all(ids.map(async (id) => { try { out[id] = await api.get<Lite>(`/containers/${id}/rack`); } catch { out[id] = { elements: [], pending: [] }; } }));
    racks = out; loaded = true;
  }
  // Rechargé quand un élément change (révision) ou que son matériel change (totaux).
  $effect(() => { void [desk, ...casq].map((c) => `${c?.id}:${c?.rev}:${c?.totals.used_u}:${c?.totals.placed_u}:${c?.totals.weight_kg}`).join(); untrack(loadRacks); });

  // ----- À placer : matériel du mobilier sans poste, et matériel des casquettes pas encore posé -----
  type Item = Pending & { from: string };
  const owner = $derived(new Map(posts.flatMap((p) => p.instance_ids.map((i) => [i, p.id] as const))));
  const wanted = $derived<Item[]>(desk ? [
    ...(racks[desk.id]?.pending ?? []).filter((p) => p.instance_id && (desk.kind !== 'desk' || !owner.has(p.instance_id))).map((p) => ({ ...p, from: desk.id })),
    ...casq.flatMap((c) => (racks[c.id]?.pending ?? []).map((p) => ({ ...p, from: c.id }))),
  ] : []);
  const famColor = (f: string | null | undefined) => familyColor(f, { screen: true });
  // Rangé par nature, comme dans les racks : équipements par fonction, puis panels et accessoires.
  const catOf = (p: Pending): { key: string; label: string; color: string; order: number } => {
    if (p.accessory?.kind === 'panel') return { key: '~panel', label: 'Panels de brassage', color: 'var(--fg-3)', order: 1000 };
    if (p.nature === 'accessory') return { key: '~acc', label: 'Accessoires', color: 'var(--fg-3)', order: 1001 };
    const t = p.type_id ? refById('types', p.type_id) : null;
    const fk = p.function ?? t?.data?.function ?? (t?.parent_id ? refById('types', t.parent_id)?.data?.function : null) ?? null;
    const f = fk ? refById('functions', fk) : null;
    return f ? { key: f.id, label: f.name, color: famColor(f.id), order: f.position ?? 500 } : { key: '~none', label: 'Autres équipements', color: famColor(null), order: 999 };
  };
  const wantedGroups = $derived.by(() => {
    const m = new Map<string, { key: string; label: string; color: string; order: number; list: Item[] }>();
    for (const p of wanted) { const c = catOf(p); if (!m.has(c.key)) m.set(c.key, { ...c, list: [] }); m.get(c.key)!.list.push(p); }
    for (const g of m.values()) g.list.sort((a, b) => a.label.localeCompare(b.label, 'fr') || (a.tag ?? '').localeCompare(b.tag ?? ''));
    return [...m.values()].sort((a, b) => a.order - b.order || a.label.localeCompare(b.label, 'fr'));
  });
  const keyOf = (p: Item) => `${p.from}:${p.instance_id ?? p.article_id}`;
  const fromName = (p: Item) => (p.from === desk?.id ? (desk.kind === 'desk' ? 'Mobilier, sans poste' : 'Mobilier') : shortName(casq.find((c) => c.id === p.from)));
  const wTxt = (w: string | null) => (w && w !== 'full' ? ` · ${w === 'half' ? '½' : w === 'third' ? '⅓' : '¼'}` : '');

  // ----- Pose : choisir un élément « À placer », puis une casquette ou un poste (clic ou glisser) -----
  let armed = $state<Item | null>(null);
  let dragged = $state<Item | null>(null);
  let busy = $state(false);
  async function arm(p: Item) {
    armed = armed && keyOf(armed) === keyOf(p) ? null : p;
    if (armed && narrow) sheetOpen = false;
  }
  /** Prend la main sur l'élévation de la casquette le temps d'une écriture, puis la rend. */
  async function withLock<T>(id: string, run: () => Promise<T>): Promise<T> {
    const r = await api.post<{ mine: boolean; account_name: string }>(`/containers/${id}/rack/lock`);
    if (!r.mine) throw new Error(`${r.account_name} équipe cette casquette en ce moment : réessayez quand la main sera rendue.`);
    try { return await run(); } finally { try { await api.del(`/containers/${id}/rack/lock`); } catch { /* déjà libre */ } }
  }
  /** Déplace le matériel (ou un seul exemplaire) vers un autre élément ; renvoie l'article d'arrivée. */
  async function moveTo(p: Item, target: string): Promise<string> {
    const a = view.articles.find((x) => x.id === p.article_id);
    if (!a) throw new Error('Ce matériel n’existe plus : rechargez la page.');
    const r = await api.post<Article>(`/articles/${a.id}/move`, { rev: a.rev, container_id: target, location_id: null, instance_id: p.instance_id ?? null });
    return r.id;
  }
  async function deliver(p: Item, kind: 'c' | 'p', id: string, u?: number) {
    if (!desk || busy || !writable) return;
    armed = null; dragged = null;
    const name = p.tag ?? p.label;
    busy = true;
    try {
      if (kind === 'p') {
        const post = posts.find((x) => x.id === id);
        if (!post) return;
        if (!p.instance_id) { toast('Seul un équipement avec un ID se place sur un poste.', 'warn'); return; }
        if (p.from !== desk.id) { await moveTo(p, desk.id); await reload(); }
        const d = desks.find((x) => x.id === desk.id)!;
        const cur = d.details?.positions ?? [];
        if (await saveDetails(d, cur.map((x) => (x.id === id ? { ...x, instance_ids: [...x.instance_ids.filter((i) => i !== p.instance_id), p.instance_id!] } : x))))
          toast(`« ${name} » placé au poste ${post.role || 'sans rôle'}.`);
        selP = id; sel = null;
        return;
      }
      const c = casq.find((x) => x.id === id);
      if (!c) return;
      const rackable = p.rackable && !!p.height_u;
      // Place libre en face avant, au plus près du U visé (le haut de la casquette par défaut).
      const at = rackable ? freeU(c, Math.ceil(p.height_u!), u) : null;
      if (rackable && at === null) { toast(`Plus de place libre pour « ${name} » dans ${shortName(c)} : agrandissez la casquette ou retirez du matériel.`, 'warn'); return; }
      const art = p.from === id ? p.article_id : await moveTo(p, id);
      if (!rackable) { toast(`« ${name} » est dans ${shortName(c)} : hors format rack, il se pose sur une étagère avec « Équiper ».`, 'info'); return; }
      if (!canDesign) { toast(`« ${name} » est dans ${shortName(c)} : votre rôle ne permet pas de le poser dans l’élévation.`, 'info'); return; }
      const r = await withLock(id, () => api.post<{ adjusted: boolean; warning: string | null; placement: { u_start: number } }>(`/containers/${id}/placements`, { article_id: art, instance_id: p.instance_id ?? null, face: 'front', u_start: at, col: 0 }));
      toast(`« ${name} » posé dans ${shortName(c)} en U${r.placement.u_start}${r.adjusted ? ', la place libre la plus proche' : ''}.`);
      if (r.warning) toast(r.warning, 'warn');
      sel = id; selP = null;
    } catch (e) { fail(e); }
    finally { busy = false; await reload(); await loadRacks(); }
  }
  /** Première position libre (face avant) pour `h` U, au plus près de `want`. */
  function freeU(c: Container, h: number, want?: number): number | null {
    const units = c.height_u ?? 6, top = units - h + 1;
    if (top < 1) return null;
    const busy = new Set<number>();
    for (const e of frontEls(c.id)) for (let k = 0; k < Math.ceil(e.height_u); k++) busy.add(e.placement.u_start + k);
    const fits = (u0: number) => u0 >= 1 && u0 <= top && Array.from({ length: h }, (_, k) => u0 + k).every((x) => !busy.has(x));
    const w = Math.max(1, Math.min(want ?? top, top));
    for (let d = 0; d <= units; d++) for (const u0 of [w + d, w - d]) if (fits(u0)) return u0;
    return null;
  }
  /** U visé dans une casquette : position verticale du pointeur dans sa zone 19″. */
  function uAt(ev: MouseEvent, el: Element | null, units: number): number | undefined {
    if (!el) return undefined;
    const r = el.getBoundingClientRect();
    if (!r.height) return undefined;
    return Math.max(1, Math.min(units, Math.floor((r.bottom - ev.clientY) / (r.height / units)) + 1));
  }
  function dragStart(e: DragEvent, p: Item) {
    dragged = p;
    if (e.dataTransfer) { e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', p.label); }
  }
  let overT = $state<string | null>(null);
  /** Cible du glisser-déposer : casquette ou poste. */
  const dz = (kind: 'c' | 'p', id: string, inner?: string, units = 6) => ({
    ondragover: (e: DragEvent) => { if (!dragged) return; e.preventDefault(); overT = `${kind}:${id}`; },
    ondragleave: () => { if (overT === `${kind}:${id}`) overT = null; },
    ondrop: (e: DragEvent) => { e.preventDefault(); overT = null; const p = dragged; if (p) deliver(p, kind, id, inner ? uAt(e, (e.currentTarget as Element).querySelector(inner) ?? (e.currentTarget as Element), units) : undefined); },
  });
  /** Clic sur une casquette du dessin : pose de l'élément choisi au U visé, sinon sélection. */
  function hitCasq(ev: MouseEvent, c: Container, inner: string) {
    if (armed) { const t = ev.currentTarget as Element; deliver(armed, 'c', c.id, uAt(ev, t.matches(inner) ? t : t.querySelector(inner), c.height_u ?? 6)); return; }
    sel = c.id; selP = null;
  }

  // ----- Plan de face -----
  let stageW = $state(900), stageH = $state(600);
  let narrow = $state(false);
  $effect(() => {
    const mq = window.matchMedia('(max-width: 767px)');
    const on = () => (narrow = mq.matches); on();
    mq.addEventListener('change', on);
    return () => mq.removeEventListener('change', on);
  });
  let sheetOpen = $state(false);
  const topU = $derived(Math.max(6, ...casq.map((c) => c.height_u ?? 6)));
  // Marges du plan : la cote de hauteur à gauche (54 px), rien à droite.
  const padL = $derived(narrow ? 44 : 60), padR = $derived(narrow ? 12 : 24);
  const s = $derived.by(() => {
    const byW = (stageW - 32 - padL - padR) / L;
    const byH = fitH && !narrow ? (stageH - 78 - 134) / (Hd + topU * 44.45 + 120) : Infinity;
    return Math.max(0.08, Math.min(0.55, byW, byH));
  });
  const planH = $derived((Hd + topU * 44.45 + 120) * s);
  const typeOf = (e: El) => (e.nature === 'accessory' ? accFace(e as any) : visualOf(e));
  const cols = (w: string) => ({ full: 1, half: 2, third: 3, quarter: 4 } as Record<string, number>)[w] ?? 1;
  /** Textes : en schématique, les accessoires gardent leur nom (pas de façade dessinée). */
  const rl = (e: El): [string, string] => (mode === 'schematic' && e.nature === 'accessory' ? [e.label, ''] : rackLines(e));
  const skin = (e: El) => { const c = famColor(e.function); return `--c:${c}` + (mode === 'schematic' ? `;--fp-bg:linear-gradient(180deg,color-mix(in srgb,${c} 72%,#fff),${c});--fp-ink:#fff` : ''); };
  const frontEls = (id: string) => (racks[id]?.elements ?? []).filter((e) => e.placement.face === 'front' && !e.placement.shelf_id);

  let drag = $state<{ id: string; x0: number; off0: number; off: number } | null>(null);
  function startDrag(e: PointerEvent, c: Container) {
    if (armed) return;
    sel = c.id; selP = null;
    if (!writable) return;
    drag = { id: c.id, x0: e.clientX, off0: c.offset_mm ?? 0, off: c.offset_mm ?? 0 };
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
  }
  function moveDrag(e: PointerEvent) {
    if (!drag) return;
    drag = { ...drag, off: Math.round(Math.max(0, Math.min(L - WC, drag.off0 + (e.clientX - drag.x0) / s)) / 10) * 10 };
  }
  async function endDrag() {
    const d = drag; drag = null;
    if (!d || d.off === d.off0) return;
    const c = casq.find((x) => x.id === d.id); if (c) await patch(c, { offset_mm: d.off });
  }
  const offOf = (c: Container) => (drag?.id === c.id ? drag.off : (c.offset_mm ?? 0));

  // ----- Casquettes côte à côte : élévations à l'échelle des racks -----
  // Ajustée : la plus grande hauteur d'U qui fait tenir toutes les casquettes dans la zone
  // (sur une ou plusieurs rangées) ; sinon l'échelle des racks réduite, avec défilement.
  const UC = $derived.by(() => {
    if (!fitH || narrow || !casq.length) return narrow ? 17 : 26;
    let best = 12;
    for (let n = 1; n <= casq.length; n++) {
      const rows: Container[][] = [];
      for (let i = 0; i < casq.length; i += n) rows.push(casq.slice(i, i + n));
      const byW = ((stageW - 32 - (n - 1) * 36) / n - 36) / 10.86;
      const byH = (stageH - 78 - rows.length * 110 - (rows.length - 1) * 40) / rows.reduce((t, r) => t + Math.max(...r.map((c) => c.height_u ?? 6)), 0);
      best = Math.max(best, Math.min(byW, byH));
    }
    return Math.min(30, best);
  });
  const BWc = $derived(Math.round(UC * 10.86) - 22);

  // ----- Écritures -----
  async function patch(c: Container, changes: Partial<Container>) {
    const { id, project_id: _p, position: _o, rev, totals: _t, ...data } = c;
    try { await api.patch(`/containers/${id}`, { ...data, ...changes, rev }); await reload(); } catch (e) { fail(e); await reload(); }
  }
  /** Enregistre les postes du mobilier (caractéristiques propres de l'élément). */
  async function saveDetails(c: Container, positions: DeskPosition[]): Promise<boolean> {
    const { id, project_id: _p, position: _o, rev, totals: _t, ...data } = c;
    try { await api.patch(`/containers/${id}`, { ...data, details: { ...c.details, positions }, rev }); await reload(); return true; }
    catch (e) { fail(e); await reload(); return false; }
  }
  const savePosts = (ps: DeskPosition[]) => { const d = desks.find((x) => x.id === desk?.id); return d ? saveDetails(d, ps) : Promise.resolve(false); };
  async function addCasquette() {
    if (!desk) return;
    // Première place libre de gauche à droite.
    let off = 40;
    for (const c of casq) { if ((c.offset_mm ?? 0) - off >= WC) break; off = Math.max(off, Math.ceil(((c.offset_mm ?? 0) + WC + 20) / 10) * 10); }
    if (off + WC > L) off = Math.max(0, L - WC);
    off = Math.round(off / 10) * 10;
    try {
      const c = await api.post<Container>(`/projects/${pid}/containers`, { location_id: desk.location_id, name: `${desk.name} · casquette ${casq.length + 1}`, kind: 'casquette', rackable: true, height_u: 6, depth_mm: 350, parent_id: desk.id, offset_mm: off, tilt_deg: 15, cable_code: '', details: {} });
      toast('Casquette ajoutée : équipez-la avec « Équiper la casquette ».'); await reload(); sel = c.id; selP = null;
    } catch (e) { fail(e); }
  }
  async function addPost() {
    const n = posts.length;
    const p: DeskPosition = { id: `p${Date.now().toString(36)}`, role: `Poste ${n + 1}`, name: '', instance_ids: [] };
    if (await savePosts([...posts, p])) { selP = p.id; sel = null; toast(`Poste ${n + 1} ajouté : donnez-lui un rôle.`); }
  }
  /** Suppression d'un poste : son matériel reste dans le mobilier (à placer) ; Annuler le rétablit. */
  async function delPost(p: DeskPosition) {
    const before = posts, i = before.findIndex((x) => x.id === p.id);
    if (!(await savePosts(before.filter((x) => x.id !== p.id)))) return;
    selP = null;
    toast(`Poste « ${p.role || i + 1} » supprimé${p.instance_ids.length ? ` : ${p.instance_ids.length} exemplaire${pl(p.instance_ids.length)} à placer` : ''}.`, 'ok', { run: async () => {
      const now = desks.find((x) => x.id === desk?.id)?.details?.positions ?? [];
      return (await savePosts([...now.slice(0, i), p, ...now.slice(i)])) ? undefined : false;
    } });
  }
  // Nouveau pupitre : on demande l'emplacement et le nom avant de créer.
  let newDesk = $state<{ location_id: string; name: string; busy: boolean } | null>(null);
  function askDesk() {
    if (!view.locations.length) { toast('Créez d’abord un emplacement dans l’inventaire.', 'warn'); return; }
    newDesk = { location_id: view.locations[0]!.id, name: 'Pupitre', busy: false };
  }
  async function addDesk() {
    const d = newDesk; if (!d || !d.name.trim() || !d.location_id) return;
    // Erreur : remontée à la boîte (Dialog l'affiche et reste ouverte).
    const c = await api.post<Container>(`/projects/${pid}/containers`, { location_id: d.location_id, name: d.name.trim(), kind: 'desk', rackable: false, length_mm: 2400, width_mm: 900, outer_height_mm: 750, cable_code: '', details: { positions: [] } });
    const loc = view.locations.find((l) => l.id === d.location_id);
    newDesk = null;
    toast(`Mobilier « ${c.name} » ajouté${loc ? ` dans ${loc.name}` : ''}.`);
    await reload(); navigate(deskHref(c.id));
  }
  /** Suppression d'une casquette : son matériel part avec elle (cascade), donc confirmation. */
  async function delCasquette(c: Container) {
    const ok = await ask({
      title: `Supprimer « ${c.name} » ?`, eyebrow: 'Casquette', confirmLabel: 'Supprimer la casquette', danger: true,
      text: 'Le matériel qu’elle contient est supprimé de l’inventaire avec elle.',
      run: () => api.del(`/containers/${c.id}`),
    });
    if (!ok) return;
    toast(`« ${c.name} » supprimée.`); sel = null; await reload();
  }
  /** Vider une casquette : son matériel posé redevient « à placer » (l'inventaire ne change pas). */
  async function clearCasquette(c: Container) {
    const n = racks[c.id]?.elements.length ?? 0;
    await ask({
      title: 'Vider la casquette ?', eyebrow: c.name, confirmLabel: 'Vider la casquette', danger: true,
      text: `Tous les équipements posés (${n}) redeviennent « à placer ». L’inventaire ne change pas.`,
      run: async () => { const r = await withLock(c.id, () => api.post<{ removed: number }>(`/containers/${c.id}/placements/clear`)); toast(`${r.removed} équipement${pl(r.removed)} remis à placer.`); await loadRacks(); await reload(); },
    });
  }
  // Champs numériques du panneau : copie locale reprise à chaque changement de sélection ou rechargement.
  type CqKey = 'height_u' | 'depth_mm' | 'offset_mm' | 'tilt_deg';
  type DkKey = 'length_mm' | 'width_mm' | 'outer_height_mm';
  let cq = $state<Record<CqKey, number | null>>({ height_u: null, depth_mm: null, offset_mm: null, tilt_deg: null });
  let dk = $state<Record<DkKey, number | null>>({ length_mm: null, width_mm: null, outer_height_mm: null });
  $effect(() => { const c = selC; if (c) untrack(() => { cq = { height_u: c.height_u ?? 6, depth_mm: c.depth_mm ?? 350, offset_mm: Math.round(c.offset_mm ?? 0), tilt_deg: c.tilt_deg ?? 0 }; }); });
  $effect(() => { const d = desk; if (d) untrack(() => { dk = { length_mm: d.length_mm ?? null, width_mm: d.width_mm ?? null, outer_height_mm: d.outer_height_mm ?? null }; }); });
  /** Enregistre le champ quitté (événement change du champ, identifié par son id « cq-… » ou « dk-… »). */
  function saveField(e: Event) {
    const id = (e.target as HTMLElement).id;
    const k = id.slice(3);
    if (id.startsWith('cq-') && selC && k in cq) patch(selC, { [k]: cq[k as CqKey] });
    else if (id.startsWith('dk-') && desk && k in dk) patch(desk, { [k]: dk[k as DkKey] });
  }
  // Clavier : Échap remonte d'un niveau (élément choisi, puis sélection) ; Suppr supprime la casquette ou le poste sélectionné.
  $effect(() => canvasKeys({
    enabled: () => !equip && !!desk,
    onEscape: () => { if (armed) armed = null; else clearSel(); },
    onDelete: () => { if (!writable) return; if (selC) delCasquette(selC); else if (selPost) delPost(selPost); },
  }));
  const shortName = (c: Container | undefined) => (!c ? '' : desk ? c.name.replace(`${desk.name} · `, '') : c.name);
  const equipIt = (id: string) => { if (desk) navigate(deskHref(desk.id, id)); };
  // « Ajouter du matériel » : sur la casquette sélectionnée, sinon sur le mobilier lui-même.
  let adding = $state(false);
  const addTo = $derived(selC ?? desk);

  // ----- Jauges et totaux -----
  const equipped = $derived(casq.filter((c) => (racks[c.id]?.elements.length ?? 0) > 0).length);
  const totalU = $derived(casq.reduce((n, c) => n + (c.height_u ?? 0), 0));
  const posedU = $derived(casq.reduce((n, c) => n + (c.totals.placed_u ?? 0), 0));
  const weight = $derived(desk ? desk.totals.weight_kg + casq.reduce((n, c) => n + c.totals.weight_kg, 0) : 0);
  const power = $derived(desk ? desk.totals.power_w + casq.reduce((n, c) => n + c.totals.power_w, 0) : 0);
  /** Exemplaires du mobilier (pour les postes). */
  const gear = $derived.by(() => {
    if (!desk) return [];
    const own = new Map(view.articles.filter((a) => a.container_id === desk.id).map((a) => [a.id, a.label]));
    return view.instances.filter((i) => own.has(i.article_id)).map((i) => ({ id: i.id, tag: i.tag, label: own.get(i.article_id)! }));
  });
  const fmtU = (u: number) => nf(u, u % 1 ? 1 : 0);
  const selU = (c: Container) => c.totals.placed_u ?? 0;
</script>

{#if !desks.length}
  <EmptyState icon="layers" message="Aucun mobilier dans ce projet"
    hint={view.locations.length || !writable ? 'Un mobilier (pupitre, console, meuble) se découpe en postes et reçoit des casquettes rackables pour les écrans, les surfaces de commande et le matériel à portée de main.' : 'Un mobilier se range dans un emplacement : le projet n’en a pas encore.'}>
    {#snippet action()}
      {#if writable}
        {#if view.locations.length}<button class="btn primary" onclick={askDesk}><Icon name="plus" size="sm" />Nouveau mobilier</button>
        {:else}<a class="btn primary" href="/projets/{pid}/inventaire"><Icon name="plus" size="sm" />Créer un emplacement</a>{/if}
      {/if}
    {/snippet}
  </EmptyState>
{:else if desk}
  <!-- Action principale de la page, dans l'en-tête de projet. -->
  {#if writable}<HeadActions><button class="btn primary" onclick={askDesk}><Icon name="plus" size="sm" />Nouveau mobilier</button></HeadActions>{/if}
  {#if equip && casq.some((c) => c.id === equip)}
    <div class="mob card" style="padding:0;overflow:hidden">
      <div class="dtool">
        <DeskPicker {view} value={desk.id} onpick={(v) => navigate(deskHref(v))} />
        <nav class="seg mob-casq" aria-label="Casquette">{#each casq as c (c.id)}<a href={deskHref(desk.id, c.id)} class:on={equip === c.id} aria-current={equip === c.id ? 'page' : undefined}>{shortName(c)}</a>{/each}</nav>
        <span class="gap"></span>
        <a class="btn sm ghost" href={deskHref(desk.id)}><Icon name="layers" size="sm" />Retour au mobilier</a>
      </div>
      <!-- Même éditeur que les racks, limité à cette casquette. -->
      {#key equip}<div class="mob-equip"><Racks {view} fixed={equip} /></div>{/key}
    </div>
  {:else}
  <div class="rack-screen mob card" style="padding:0;overflow:hidden">
    <div class="dtool">
      <DeskPicker {view} value={desk.id} onpick={(v) => navigate(deskHref(v))} />
      <!-- En 3D, la vue et les textes n'ont pas d'effet : ils sont masqués (le cadrage est dans la vue 3D). -->
      {#if !show3d}<Seg label="Vue" bind:value={face} options={VIEWS} onchange={() => (armed = null)} />{/if}
      <Seg label="Rendu" bind:value={render} options={[...RENDERS]} onchange={(v) => { if (v === '3d') armed = null; }} />
      {#if !show3d}<label class="rtx" title="Textes affichés sur les équipements"><span class="flabel">Textes</span><select class="input sm" value={rackText.v} onchange={(e) => setRackText((e.target as HTMLSelectElement).value as RackText)}>{#each Object.entries(RACK_TEXT) as [k, l]}<option value={k}>{l}</option>{/each}</select></label>
        <button class="btn sm ghost" aria-pressed={fitH} title="Ajuster le dessin à la hauteur de l’écran" onclick={() => (fitH = !fitH)}><Icon name="fit" size="sm" />Ajuster à la hauteur</button>{/if}
      <span class="gap"></span>
      <span class="meter" title="Casquettes avec du matériel posé ; {fmtU(posedU)} U posés sur {totalU} U{wanted.length ? ` ; ${wanted.length} à placer` : ''}"><span>Casq.</span><Vu value={casq.length ? (equipped / casq.length) * 100 : 0} mode="prog" n={16} /><span class="mono">{equipped} / {casq.length}</span>{#if wanted.length}<span class="muted small mu-pend">+ {wanted.length} à placer</span>{/if}</span>
      <span class="meter" class:err={!!desk.max_weight_kg && weight > desk.max_weight_kg}><span>Poids</span><Vu value={desk.max_weight_kg ? (weight / desk.max_weight_kg) * 100 : 0} mode="load" n={16} /><span class="mono">{nf(weight, 1)}{desk.max_weight_kg ? ` / ${nf(desk.max_weight_kg)}` : ''} kg</span></span>
      <span class="meter" class:err={!!desk.max_power_w && power > desk.max_power_w}><span>Puiss.</span><Vu value={desk.max_power_w ? (power / desk.max_power_w) * 100 : 0} mode="load" n={16} /><span class="mono">{nf(power)}{desk.max_power_w ? ` / ${nf(desk.max_power_w)}` : ''} W</span></span>
    </div>

    <div class="design-body" class:ro={!writable}>
      <aside class="pane left" class:open={sheetOpen} aria-label="À placer">
        <button class="sheet-h" aria-expanded={sheetOpen} onclick={() => (sheetOpen = !sheetOpen)}>
          <span><b>À placer</b> <span class="muted">· {wanted.length}</span></span>
          {#if armed}<span class="sheet-armed">{armed.label} · touchez une casquette ou un poste</span>{/if}
          <Icon name="chevd" size="sm" />
        </button>
        <section>
          <h2 class="sheet-t">À placer <span class="muted">· {wanted.length}</span></h2>
          {#if writable && !show3d}<p class="small muted">Choisissez un équipement, puis une casquette ou un poste du dessin (clic ou glisser). Échap pour annuler.</p>{/if}
          {#if writable}<button class="btn sm add-mat" onclick={() => (adding = true)} title="Ajouter du matériel {selC ? `dans ${shortName(selC)}` : `à ${desk.name}`}"><Icon name="plus" size="sm" />Ajouter du matériel</button>{/if}
          {#each wantedGroups as g (g.key)}
            <div class="pcat eyebrow"><i style="--c:{g.color}"></i><span>{g.label}</span><b>{g.list.length}</b></div>
            {#each g.list as p (keyOf(p))}
              <button class="pend mini" class:on={!!armed && keyOf(armed) === keyOf(p)} aria-pressed={!!armed && keyOf(armed) === keyOf(p)} data-pend={keyOf(p)} disabled={!writable || show3d || busy}
                draggable={writable && !show3d && !busy} ondragstart={(e) => dragStart(e, p)} ondragend={() => { dragged = null; overT = null; }} onclick={() => arm(p)} title="{p.label}{p.tag ? ` · ${p.tag}` : ''} · {fromName(p)}">
                <i class="pm-c" style="--c:{famColor(p.function)}"></i><span class="pm-l">{p.label}<small>{fromName(p)}</small></span>{#if p.tag}<span class="pm-t">{p.tag}</span>{/if}<span class="pm-u">{p.height_u ? `${p.height_u} U${wTxt(p.width)}` : 'Hors rack'}</span>
              </button>
            {/each}
          {:else}<p class="small muted">{loaded ? 'Tout est placé.' : 'Chargement…'}</p>{/each}
        </section>
      </aside>

      <div class="rack-stage mob-stage" class:stage3d={show3d} class:placing={!!armed || !!dragged} bind:clientWidth={stageW} bind:clientHeight={stageH}>
        {#if show3d}
          <Desk3D {desk} casquettes={casq.map((c) => ({ c, elements: racks[c.id]?.elements ?? [] }))} />
        {:else if face === 'side' || face === 'top'}
          <div class="dd-wrap">
            <DeskDraw {desk} {casq} {posts} mode={face} {sel} {selP} w={stageW} h={narrow ? 4000 : stageH - 60} armed={!!armed || !!dragged}
              nameOf={shortName} gearOf={(p) => p.instance_ids.length} onhit={pick} onequip={equipIt} ondropon={(k, id) => { if (dragged) deliver(dragged, k, id); }} />
          </div>
        {:else if face === 'casq'}
          <div class="cq-row-wrap">
            {#each casq as c (c.id)}
              {@const units = c.height_u ?? 6}
              <div class="rack-wrap cq-el" class:sel={sel === c.id}>
                <button class="rack-title cq-t" onclick={() => { sel = c.id; selP = null; }} ondblclick={() => equipIt(c.id)} title="Sélectionner · double-clic pour équiper"><Icon name="server" />{shortName(c)}{c.tilt_deg ? ` · ${nf(c.tilt_deg)}°` : ''}</button>
                <div class="rack" class:over={overT === `c:${c.id}`} style="--units:{units};--u:{UC}px">
                  <div class="rack-scale">{#each Array(units) as _, i}<span>{units - i}</span>{/each}</div>
                  <div class="rack-body" style="width:{BWc}px" role="button" tabindex="0" aria-pressed={sel === c.id} aria-label="{shortName(c)} : Espace pour sélectionner, Entrée pour équiper"
                    onclick={(e) => hitCasq(e, c, '.rack-body')} ondblclick={() => equipIt(c.id)} onkeydown={(e) => { if (e.key === 'Enter' && !armed) { e.preventDefault(); equipIt(c.id); } else if (e.key === ' ' || e.key === 'Enter') { e.preventDefault(); pick('c', c.id); } }} {...dz('c', c.id, '.rack-body', units)}>
                    {#each frontEls(c.id) as e (e.placement.id)}
                      {@const n = cols(e.placement.width)}
                      <div class="ritem {e.nature === 'accessory' ? 'acc' : ''} {ALU.has(typeOf(e)) && mode === 'real' ? 'alu' : ''} {e.height_u <= 1 ? 'u1' : ''}" title="{e.custom ? `${e.custom} · ` : ''}{e.tag ? `${e.tag} · ` : ''}{e.label} · U{e.placement.u_start}"
                        style="{skin(e)};bottom:calc({e.placement.u_start - 1} * var(--u) + 1px);height:calc({e.height_u} * var(--u) - 2px);left:calc({(100 * e.placement.col) / n}% + 2px);width:calc({100 / n}% - 4px)">
                        {#if e.nature !== 'accessory'}<i class="led"></i>{/if}<span class="rl" class:two={!!rl(e)[1]}>{#if rl(e)[0]}<b class="rn" class:id={!!e.tag && rl(e)[0] === e.tag}>{rl(e)[0]}</b>{/if}{#if rl(e)[1]}<small>{rl(e)[1]}</small>{/if}</span>{#if mode === 'real'}<span class="dec">{@html decor(typeOf(e))}</span>{:else}<span class="dec"></span>{/if}
                      </div>
                    {/each}
                  </div>
                  <div class="rack-scale">{#each Array(units) as _, i}<span>{units - i}</span>{/each}</div>
                </div>
                {#if (racks[c.id]?.pending.length ?? 0) > 0}<span class="cq-pend2">{racks[c.id]!.pending.length} à placer</span>{/if}
              </div>
            {:else}<p class="muted small cq-none">Aucune casquette : « Ajouter une casquette » dans le panneau de droite.</p>{/each}
          </div>
        {:else}
          <div class="plan" style="height:{planH + 40}px;padding-left:{padL}px;padding-right:{padR}px" role="presentation" onpointermove={moveDrag} onpointerup={endDrag}>
            <div class="desk" style="width:{L * s}px;height:{Hd * s}px">
              <span class="top"></span><span class="pad"></span><span class="led"></span>
              {#if casq.length}<span class="deck" style="height:{Math.max(...casq.map((c) => c.height_u ?? 6)) * 44.45 * s * 0.55}px"><i></i></span>{/if}
              <span class="leg l"></span><span class="leg r"></span><span class="voile"></span><span class="tray"></span>
              <span class="floor"></span>
              <span class="dim mono"><i></i>{nf(L)} mm<i></i></span>
              <span class="dimh mono"><i></i><span>{nf(Hd)} mm</span><i></i></span>
              <!-- Postes : répartis sur la longueur, côté opérateur. -->
              {#if posts.length}
                <div class="posts" style="grid-template-columns:repeat({posts.length}, minmax(0, 1fr))">
                  {#each posts as p, i (p.id)}
                    <button class="post" class:sel={selP === p.id} class:over={overT === `p:${p.id}`} aria-pressed={selP === p.id} title="Poste {i + 1} · {p.role || 'Poste'}{p.name ? ` · ${p.name}` : ''} · {p.instance_ids.length} exemplaire{pl(p.instance_ids.length)}" onclick={() => pick('p', p.id)} {...dz('p', p.id)}>
                      <span class="post-n mono">{i + 1}</span><span class="post-r">{p.role || 'Poste'}</span><span class="post-c mono">{p.instance_ids.length}</span>
                    </button>
                  {/each}
                </div>
              {/if}
              {#each casq as c (c.id)}
                {@const u = (c.height_u ?? 6)}
                {@const up = 44.45 * s}
                <div class="cq" class:sel={sel === c.id} class:over={overT === `c:${c.id}`} class:dragging={drag?.id === c.id} style="left:{offOf(c) * s}px;width:{WC * s}px;height:{u * up + 50 * s}px;--u:{up}px"
                  role="button" tabindex="0" aria-pressed={sel === c.id} aria-label="{shortName(c)} : Espace pour sélectionner, Entrée pour équiper" title={armed ? `Poser ${armed.label} ici` : 'Glisser pour déplacer · Entrée ou double-clic pour équiper'}
                  ondblclick={() => equipIt(c.id)} onpointerdown={(e) => startDrag(e, c)} onclick={(e) => { if (armed) hitCasq(e, c, '.cq-in'); }}
                  onkeydown={(e) => { if (e.key === 'Enter' && !armed) { e.preventDefault(); equipIt(c.id); } else if (e.key === ' ' || e.key === 'Enter') { e.preventDefault(); pick('c', c.id); } }} {...dz('c', c.id, '.cq-in', u)}>
                  <span class="cq-name">{shortName(c)}{c.tilt_deg ? ` · ${nf(c.tilt_deg)}°` : ''}</span>
                  {#if (racks[c.id]?.pending.length ?? 0) > 0}<span class="cq-pend" title="Matériel de cette casquette pas encore posé : Entrée ou double-clic pour l’équiper">{racks[c.id]!.pending.length} à placer</span>{/if}
                  <div class="cq-in">
                    {#each frontEls(c.id) as e (e.placement.id)}
                      {@const n = cols(e.placement.width)}
                      {@const [l1, l2] = rl(e)}
                      <span class="ritem {e.nature === 'accessory' ? 'acc' : ''} {ALU.has(typeOf(e)) && mode === 'real' ? 'alu' : ''} {e.height_u <= 1 ? 'u1' : ''}" title="{e.custom ? `${e.custom} · ` : ''}{e.tag ? `${e.tag} · ` : ''}{e.label}"
                        style="{skin(e)};bottom:{(e.placement.u_start - 1) * up + 1}px;height:{e.height_u * up - 2}px;left:calc({(100 * e.placement.col) / n}% + 1px);width:calc({100 / n}% - 2px)">{#if up * e.height_u > 16}<span class="rl" class:two={!!l2 && up * e.height_u > 30}>{#if l1}<b class="rn" class:id={!!e.tag && l1 === e.tag}>{l1}</b>{/if}{#if l2 && up * e.height_u > 30}<small>{l2}</small>{/if}</span>{#if mode === 'real'}<span class="dec">{@html decor(typeOf(e))}</span>{/if}{/if}</span>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
            {#if !casq.length}<p class="plan-hint muted small">Ajoutez une casquette (panneau de droite) : elle se pose sur le plateau et se glisse à sa place.</p>{/if}
          </div>
        {/if}
      </div>

      <aside class="pane right">
        {#if selC}
          {@const c = selC}
          {@const els = racks[c.id]?.elements ?? []}
          <section>
            <h2 class="sel-h"><span>{shortName(c)}</span><button class="icon-btn sm" aria-label="Désélectionner (Échap)" title="Désélectionner (Échap)" onclick={clearSel}><Icon name="x" /></button></h2>
            <div class="field"><label for="cq-n">Nom</label><input class="input" id="cq-n" value={c.name} disabled={!writable} onchange={(e) => patch(c, { name: (e.target as HTMLInputElement).value })}></div>
            <fieldset class="mob-fs fg2" disabled={!writable} onchange={saveField}>
              <NumField id="cq-height_u" label="Hauteur" unit="U" min={1} max={24} step={1} bind:value={cq.height_u} />
              <NumField id="cq-depth_mm" label="Profondeur" unit="mm" min={100} max={900} step={10} bind:value={cq.depth_mm} />
              <NumField id="cq-offset_mm" label="Position" unit="mm" min={0} step={10} bind:value={cq.offset_mm} help="Depuis le bord gauche du plateau." />
              <NumField id="cq-tilt_deg" label="Inclinaison" unit="°" min={0} max={60} bind:value={cq.tilt_deg} help="Vers l’arrière, face vers l’opérateur." />
            </fieldset>
            <dl class="kv"><dt>Hauteur posée</dt><dd>{fmtU(selU(c))} / {c.height_u} U</dd><dt>Matériel posé</dt><dd>{els.length}</dd><dt>À placer</dt><dd>{racks[c.id]?.pending.length ?? 0}</dd><dt>Poids</dt><dd>{nf(c.totals.weight_kg, 1)} kg</dd><dt>Puissance</dt><dd>{nf(c.totals.power_w)} W</dd><dt>Code câble</dt><dd class="mono">{c.cable_code || '—'}</dd></dl>
            <a class="btn sm primary" style="align-self:flex-start" href={deskHref(desk.id, c.id)}><Icon name="server" size="sm" />Équiper la casquette</a>
          </section>
          <section>
            <h2>Matériel posé <span class="muted">· {els.length}</span></h2>
            {#each [...els].sort((a, b) => b.placement.u_start - a.placement.u_start) as e (e.placement.id)}
              <div class="el-row"><i class="pm-c" style="--c:{famColor(e.function)}"></i><span class="pm-l">{e.label}</span>{#if e.tag}<span class="pm-t">{e.tag}</span>{/if}<span class="pm-u">U{e.placement.u_start}</span></div>
            {:else}<p class="small muted">Rien n’est posé : « Équiper la casquette », ou glissez du matériel depuis « À placer ».</p>{/each}
          </section>
          {#if writable}
            <section class="rk-clear">
              {#if els.length && canDesign}<button class="btn sm ghost danger" title="Tous les équipements posés redeviennent « à placer » (après confirmation)" onclick={() => clearCasquette(c)}><Icon name="undo" size="sm" />Vider la casquette</button>{/if}
              <button class="btn sm ghost danger" title="Supprimer la casquette et son matériel (Suppr, après confirmation)" onclick={() => delCasquette(c)}><Icon name="trash" size="sm" />Supprimer la casquette</button>
            </section>
          {/if}
        {:else if selPost}
          <DeskPost post={selPost} index={posts.indexOf(selPost)} positions={posts} {gear} {writable} onsave={savePosts} onclose={clearSel} ondelete={() => delPost(selPost)} />
        {:else}
          <section>
            <h2>{CONTAINER_KINDS[desk.kind].label}</h2>
            <fieldset class="mob-fs fg2" disabled={!writable} onchange={saveField}>
              <NumField id="dk-length_mm" label="Longueur" unit="mm" min={600} max={3000} step={10} suggestions={[1800, 2400, 3000]} bind:value={dk.length_mm} />
              <NumField id="dk-width_mm" label="Profondeur" unit="mm" min={300} max={3000} step={10} suggestions={[900]} bind:value={dk.width_mm} />
              <NumField id="dk-outer_height_mm" label="Hauteur" unit="mm" min={400} max={1300} step={10} suggestions={[750]} bind:value={dk.outer_height_mm} />
            </fieldset>
          </section>
          <section>
            <h2>Casquettes <span class="muted">· {casq.length}</span></h2>
            {#each casq as c (c.id)}
              <button class="cq-row" onclick={() => pick('c', c.id)}><b>{shortName(c)}</b><span class="muted small mono">{c.height_u} U · {nf(c.offset_mm ?? 0)} mm{c.tilt_deg ? ` · ${nf(c.tilt_deg)}°` : ''}</span><span class="muted small">{racks[c.id]?.elements.length ?? 0} posé{pl(racks[c.id]?.elements.length ?? 0)}{(racks[c.id]?.pending.length ?? 0) ? ` · ${racks[c.id]!.pending.length} à placer` : ''}</span></button>
            {:else}<p class="muted small" style="margin:0">Aucune casquette.</p>{/each}
            {#if writable}<button class="btn sm" style="align-self:flex-start" onclick={addCasquette}><Icon name="plus" size="sm" />Ajouter une casquette</button>{/if}
          </section>
          {#if desk.kind === 'desk'}
            <section>
              <h2>Postes <span class="muted">· {posts.length}</span></h2>
              {#each posts as p, i (p.id)}
                <button class="cq-row" onclick={() => pick('p', p.id)}><b>{i + 1} · {p.role || 'Poste'}</b><span class="muted small mono">{p.instance_ids.length} ex.</span>{#if p.name}<span class="muted small">{p.name}</span>{/if}</button>
              {:else}<p class="muted small" style="margin:0">Aucun poste : un poste (rôle, opérateur) reçoit le matériel à portée de main.</p>{/each}
              {#if writable}<button class="btn sm" style="align-self:flex-start" onclick={addPost}><Icon name="plus" size="sm" />Ajouter un poste</button>{/if}
            </section>
          {/if}
          <section>
            <h2>Totaux</h2>
            <dl class="kv"><dt>Casquettes équipées</dt><dd>{equipped} / {casq.length}</dd><dt>Hauteur posée</dt><dd>{fmtU(posedU)} / {totalU} U</dd>{#if wanted.length}<dt>À placer</dt><dd>{wanted.length}</dd>{/if}{#if desk.kind === 'desk'}<dt>Postes</dt><dd>{posts.length} · {owner.size} / {gear.length} ex. affecté{pl(owner.size)}</dd>{/if}<dt>Poids</dt><dd>{nf(weight, 1)} kg</dd><dt>Puissance</dt><dd>{nf(power)} W</dd><dt>Dissipation</dt><dd>{nf(power * 3.412)} BTU/h</dd></dl>
          </section>
          <section class="rk-clear">
            <a class="btn sm ghost" style="align-self:flex-start" href="/projets/{pid}/inventaire" title="Le matériel, les exemplaires et les caractéristiques du mobilier dans l’inventaire"><Icon name="box" size="sm" />Voir dans l’inventaire</a>
          </section>
        {/if}
      </aside>
    </div>
  </div>
  {/if}
{/if}

{#if adding && addTo}<AddArticle {view} container={addTo} locationId={addTo.location_id} onclose={() => (adding = false)} ondone={async () => { await reload(); await loadRacks(); }} />{/if}

{#if newDesk}
  <Dialog title="Nouveau mobilier" eyebrow="Mobilier" onclose={() => (newDesk = null)} onsubmit={addDesk} submitLabel="Ajouter le mobilier" submitDisabled={!newDesk.name.trim()}>
    <div class="stack">
      <div class="field"><label for="nd-loc">Emplacement</label><select class="input" id="nd-loc" bind:value={newDesk.location_id}>{#each view.locations as l}<option value={l.id}>{l.name}{l.code && l.code !== l.name ? ` · ${l.code}` : ''}</option>{/each}</select></div>
      <div class="field"><label for="nd-name">Nom</label><input class="input" id="nd-name" bind:value={newDesk.name} required maxlength="120" placeholder="Pupitre réalisateur, console son"></div>
      <p class="muted small">Mobilier de 2 400 × 900 mm, 750 mm de haut : les cotes se règlent ensuite dans le panneau de droite.</p>
    </div>
  </Dialog>
{/if}

<style>
  /* ----- Écran : mêmes zones que les racks (barre, À placer, dessin, détail) ----- */
  .mob-equip :global(.rack-screen) { border-radius: 0; box-shadow: none; }
  .mob-casq a { display: inline-flex; align-items: center; height: var(--btn-h-sm, 30px); padding: 0 11px; border-radius: var(--btn-r-sm, 7px); font-size: var(--fs-ctl-sm, 13px); font-weight: 550; color: var(--fg-2); text-decoration: none; white-space: nowrap; }
  .mob-casq a:hover { color: var(--fg); }
  .mob-casq a.on { background: var(--key-bg); color: var(--fg); font-weight: 650; box-shadow: var(--key); }
  .mob-casq a:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  /* Hauteur fixe : le dessin défile seul, les panneaux latéraux restent visibles (comme les racks).
     L'écran remplit la hauteur sous l'en-tête de projet (~108 px) et la marge basse de la page
     (64 px) : pas de défilement de page, même quand la barre passe sur deux lignes. */
  .mob { display: flex; flex-direction: column; height: calc(100dvh - 226px); min-height: 600px; }
  .rack-screen .design-body { flex: 1; height: auto; min-height: 0; }
  .mob-equip { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .mob .mob-equip :global(.rack-screen) { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .mob .mob-equip :global(.rack-screen .design-body) { flex: 1; height: auto; min-height: 0; }
  .mob-fs { margin: 0; padding: 0; border: 0; min-width: 0; }
  .sel-h { justify-content: space-between; align-items: center; }
  .sel-h span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .add-mat { align-self: stretch; justify-content: center; margin: 2px 0 6px; }
  .pcat { display: flex; align-items: center; gap: 8px; margin: 10px 0 2px; padding: 0 2px; }
  .pcat i { width: 8px; height: 8px; border-radius: 50%; background: var(--c); }
  .pcat b { margin-left: auto; font: 600 11px var(--mono); padding: 1px 7px; border-radius: 99px; background: var(--surface-3); color: var(--fg-2); }
  .pend.mini { flex-direction: row; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 8px; min-height: 34px; }
  .pend.mini[draggable='true'] { cursor: grab; }
  .pm-c { flex: none; width: 4px; align-self: stretch; border-radius: 2px; background: var(--c); }
  .pm-l { flex: 1; min-width: 0; display: flex; flex-direction: column; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13.5px !important; font-weight: 600; color: var(--fg) !important; line-height: 1.2; }
  .pm-l small { font-size: 11.5px; font-weight: 500; color: var(--fg-3); overflow: hidden; text-overflow: ellipsis; }
  .pm-t { flex: none; font: 600 11px/1 var(--mono) !important; color: var(--fg-3); }
  .pm-u { flex: none; font: 600 11px/1 var(--mono) !important; padding: 3px 6px; border-radius: 5px; background: var(--sunken); color: var(--fg-2) !important; }
  .mu-pend { white-space: nowrap; }
  .rtx { display: inline-flex; align-items: center; gap: 8px; }
  .rtx .input { width: auto; }
  .rk-clear { padding-top: 14px; border-top: 1px solid var(--line); }
  .el-row { display: flex; align-items: center; gap: 8px; padding: 6px 8px; min-height: 34px; border-radius: 8px; background: var(--surface-2); }
  .cq-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 2px 8px; padding: 8px 10px; border: 0; border-radius: 10px; background: var(--surface-2); text-align: left; font: inherit; color: inherit; cursor: pointer; }
  .cq-row:hover { box-shadow: inset 0 0 0 1.5px var(--accent); }
  .cq-row:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .cq-row .muted.small:last-child:not(:nth-child(2)) { grid-column: 1 / -1; }
  .rack-stage.stage3d { padding: 0; display: block; align-self: stretch; }
  .mob-stage.placing { cursor: copy; }

  /* ----- Vue de côté et de dessus ----- */
  .dd-wrap { margin: auto; }

  /* ----- Casquettes côte à côte ----- */
  .cq-row-wrap { display: flex; flex-wrap: wrap; justify-content: center; align-items: flex-end; gap: 40px 36px; padding-top: 6px; }
  .cq-el { position: relative; }
  .cq-t { border: 0; cursor: pointer; font: 700 13px/1 var(--mono); }
  .cq-t:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  .cq-el .rack { grid-template-columns: 24px auto 24px; padding-top: 16px; padding-bottom: 18px; }
  .cq-el.sel .rack, .cq-el .rack.over { box-shadow: var(--rack-rim), 0 0 0 3px var(--accent), 0 30px 50px -24px rgba(0, 0, 0, .6); }
  .cq-el .rack-body { cursor: pointer; }
  .cq-el .rack-body:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  .placing .cq-el .rack-body { cursor: copy; }
  .cq-pend2 { font: 600 11px/1.4 var(--mono); padding: 1px 6px; border-radius: 4px; background: var(--warn); color: var(--primary-fg); }
  .cq-none { margin: 60px auto 0; }

  /* ----- Plan de face : plateau aux cotes réelles, casquettes et postes ----- */
  .cq:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  .cq-in .rl { flex-direction: column; }
  .plan { position: relative; display: flex; align-items: flex-end; justify-content: center; padding-top: 44px; padding-bottom: 50px; box-sizing: content-box; min-height: 300px; touch-action: none; margin: auto; }
  .desk { position: relative; flex: none; }
  /* Plateau : noir mat, bandeau noir laqué côté opérateur, repose-poignets cuir, filet LED. */
  .desk .top { position: absolute; left: -2px; right: -2px; top: 0; height: 14px; border-radius: 5px 5px 3px 3px; background: linear-gradient(180deg, #3a3d42 0%, #16171a 40%, #0b0c0e 100%); box-shadow: inset 0 1px 0 rgba(255,255,255,.25), 0 2px 0 #1a1b1e, 0 14px 26px -14px rgba(0,0,0,.65); z-index: 2; }
  .desk .deck { position: absolute; left: 0; right: 0; bottom: 100%; border-radius: 6px 6px 0 0; background: linear-gradient(180deg, #2a2c31, #111215); box-shadow: inset 0 1px 0 rgba(255,255,255,.08); z-index: 1; }
  .desk .deck i { position: absolute; left: 3%; right: 3%; top: 3px; height: 2px; border-radius: 2px; background: #9cc8ff; box-shadow: 0 0 6px 1px rgba(90,168,255,.8); }
  .desk .pad { position: absolute; left: 8%; right: 8%; top: -6px; height: 8px; border-radius: 6px; background: linear-gradient(180deg, #34373d, #141517); box-shadow: 0 1px 2px rgba(0,0,0,.5); z-index: 3; }
  .desk .led { position: absolute; left: 4%; right: 4%; top: 15px; height: 2px; border-radius: 2px; background: #9cc8ff; box-shadow: 0 0 6px 1px rgba(90,168,255,.85), 0 0 18px 4px rgba(90,168,255,.35); z-index: 2; }
  .desk .leg { position: absolute; top: 12px; bottom: 0; width: 20px; border-radius: 3px; background: linear-gradient(90deg, #8c939b, #e3e6ea 35%, #b9bec5 60%, #7f868e); box-shadow: inset 0 0 0 1px rgba(0,0,0,.25); z-index: 1; }
  .desk .leg.l { left: 0; } .desk .leg.r { right: 0; }
  .desk .voile { position: absolute; left: 28px; right: 28px; top: 14px; height: 42%; border-radius: 0 0 4px 4px; background: linear-gradient(180deg, #2a2d33, #202328); box-shadow: inset 0 -1px 0 rgba(255,255,255,.06); }
  .desk .tray { position: absolute; left: 30px; right: 30px; bottom: 16%; height: 5px; border-radius: 2px; background: linear-gradient(180deg, #c9ccd1, #8b9097); opacity: .8; }
  .desk .floor { position: absolute; left: -4%; right: -4%; bottom: -10px; height: 14px; border-radius: 50%; background: radial-gradient(50% 50% at 50% 50%, rgba(0,0,0,.28), transparent 70%); z-index: 0; }
  .desk .dim { position: absolute; left: 0; right: 0; bottom: -30px; display: flex; align-items: center; gap: 8px; font-size: 11px; color: var(--fg-3); }
  .desk .dim i { flex: 1; height: 1px; background: var(--line-2); position: relative; }
  .desk .dim i:first-child::before, .desk .dim i:last-child::after { content: ''; position: absolute; top: -4px; width: 1px; height: 9px; background: var(--fg-3); }
  .desk .dim i:first-child::before { left: 0; } .desk .dim i:last-child::after { right: 0; }
  /* Cote verticale : trait de cote avec repères, comme la cote horizontale. */
  .desk .dimh { position: absolute; left: -54px; top: 0; bottom: 0; width: 44px; display: flex; flex-direction: column; align-items: center; gap: 6px; font-size: 11px; color: var(--fg-3); }
  .desk .dimh span { writing-mode: vertical-rl; transform: rotate(180deg); white-space: nowrap; }
  .desk .dimh i { flex: 1; width: 1px; background: var(--line-2); position: relative; }
  .desk .dimh i:first-child::before, .desk .dimh i:last-child::after { content: ''; position: absolute; left: -4px; width: 9px; height: 1px; background: var(--fg-3); }
  .desk .dimh i:first-child::before { top: 0; } .desk .dimh i:last-child::after { bottom: 0; }
  /* Postes : pastilles posées sur le voile, côté opérateur. */
  .posts { position: absolute; left: 32px; right: 32px; top: 22px; display: grid; gap: 6px; z-index: 3; }
  .post { display: flex; align-items: center; gap: 6px; min-width: 0; height: 26px; padding: 0 8px 0 4px; border: 0; border-radius: 13px; background: rgba(255,255,255,.1); box-shadow: inset 0 0 0 1px rgba(255,255,255,.18); color: #E8EBF0; font: 600 12px var(--font); cursor: pointer; text-align: left; }
  .post:hover { background: rgba(255,255,255,.16); }
  .post.sel, .post.over { background: color-mix(in srgb, var(--btn-pri) 70%, transparent); box-shadow: inset 0 0 0 1.5px #fff; color: #fff; }
  .post:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .post-n { flex: none; width: 18px; height: 18px; border-radius: 50%; display: grid; place-items: center; font-size: 11px; background: #E8EBF0; color: #16181C; }
  .post-r { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .post-c { flex: none; font-size: 11px; opacity: .8; }
  .placing .post { box-shadow: inset 0 0 0 1.5px var(--btn-pri); }
  /* Casquette encastrée : joues noir laqué, dessus noir, liseré alu ; gradin continu derrière. */
  .cq { position: absolute; bottom: 100%; margin-bottom: 0; padding: 0 calc(25 * var(--u) / 44.45) calc(25 * var(--u) / 44.45); box-sizing: border-box; border-radius: 7px 7px 2px 2px; background: linear-gradient(90deg, #050506, #2a2c31 5%, #141518 6.5%, #141518 93.5%, #2a2c31 95%, #050506); box-shadow: inset 0 3px 0 #1b1d21, inset 0 4px 0 rgba(220,224,230,.6), 0 0 0 1px #0d0e10, 0 14px 22px -14px rgba(0,0,0,.75); cursor: pointer; touch-action: none; z-index: 2; }
  .cq.sel, .cq.over { box-shadow: inset 0 3px 0 #1b1d21, inset 0 4px 0 rgba(220,224,230,.6), 0 0 0 1.5px var(--accent), 0 14px 22px -14px rgba(0,0,0,.75); }
  .placing .cq { cursor: copy; box-shadow: inset 0 3px 0 #1b1d21, inset 0 4px 0 rgba(220,224,230,.6), 0 0 0 1.5px var(--accent), 0 0 0 5px color-mix(in srgb, var(--accent) 25%, transparent); }
  .cq.dragging { cursor: grabbing; opacity: .9; }
  .cq-name { position: absolute; left: 0; right: 0; bottom: 100%; margin-bottom: 4px; text-align: center; font: 600 11px var(--font); color: var(--fg-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .cq-in { position: absolute; left: calc(25 * var(--u) / 44.45 + 10px); right: calc(25 * var(--u) / 44.45 + 10px); top: calc(25 * var(--u) / 44.45); bottom: calc(25 * var(--u) / 44.45); background: repeating-linear-gradient(to top, #1E2228 0 1px, #08090B 1px var(--u)); overflow: hidden; }
  .cq-in .ritem { position: absolute; font-size: 11px; padding: 0 5px; }
  .cq-pend { position: absolute; left: 50%; bottom: 100%; margin-bottom: 22px; transform: translateX(-50%); white-space: nowrap; font: 600 11px/1.4 var(--mono); padding: 1px 6px; border-radius: 4px; background: var(--warn); color: var(--primary-fg); z-index: 3; }
  .plan-hint { position: absolute; left: 0; right: 0; top: 30px; text-align: center; }

  /* ----- Tablette et téléphone : même comportement que les racks ----- */
  @media (max-width: 1100px) {
    .mob { height: auto; min-height: 0; }
    .rack-screen .design-body, .mob .mob-equip :global(.rack-screen .design-body) { grid-template-columns: minmax(0, 1fr); flex: none; }
    .rack-stage.stage3d { height: 560px; }
    .plan { min-height: 0; }
  }
  .sheet-h { display: none; }
  @media (max-width: 767px) {
    .rack-stage:not(.stage3d) { overflow-x: hidden; }
    .cq-row-wrap { gap: 28px 16px; }
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
</style>
