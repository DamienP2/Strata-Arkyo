<script lang="ts">
  // Inventaire (section 8) : emplacements, éléments, articles, exemplaires.
  // Deux vues : « Par élément » (liste des éléments à gauche, élément ouvert à droite) et
  // « Nomenclature » (un seul tableau groupé par élément). Les lignes de matériel se glissent
  // d'un élément à l'autre (glisser-déposer HTML5) ou passent par « Déplacer vers… ».
  import Icon from '../../components/Icon.svelte';
  import Vu from '../../components/Vu.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import HeadActions from '../../components/HeadActions.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Loading from '../../components/Loading.svelte';
  import Menu from '../../components/Menu.svelte';
  import Seg from '../../components/Seg.svelte';
  import ContainerDialog from './ContainerDialog.svelte';
  import AddArticle from './AddArticle.svelte';
  import TemplatePick from './TemplatePick.svelte';
  import ProjectPanel from './ProjectPanel.svelte';
  import ChassisDialog from './ChassisDialog.svelte';
  import WallView from './WallView.svelte';
  import DeskView from './DeskView.svelte';
  import KindIll from '../../components/KindIll.svelte';
  import { api, ApiError, download } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { ask } from '../../lib/ask.svelte';
  import { shortcutBlocked } from '../../lib/canvaskeys';
  import { modalOpen } from '../../lib/modal';
  import { nf, pl } from '../../lib/format';
  import { route } from '../../lib/router.svelte';
  import { tick, untrack } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import { CONTAINER_KINDS, type Article, type ElementDetails, type Container, type Instance, type Location, type ProjectView } from '../../lib/projects';

  let { view = $bindable(), reload }: { view: ProjectView; reload: () => Promise<void> } = $props();
  const writable = $derived(can('projets', 'ecriture'));

  // Vue : « Par élément » (a) ou « Nomenclature » (b), mémorisée dans le navigateur et dans l'adresse.
  type Mode = 'a' | 'b';
  const MODE_KEY = 'strata.inventaire.vue';
  let mode = $state<Mode>(untrack(() => { try { return localStorage.getItem(MODE_KEY) === 'b' ? 'b' : 'a'; } catch { return 'a'; } }));
  function setMode(m: Mode) {
    mode = m; sel.clear();
    try { localStorage.setItem(MODE_KEY, m); } catch { /* stockage indisponible : la vue reste celle de la session */ }
  }

  let selLoc = $state<string | null>(null);
  let panelOf = $state<Article | null>(null);
  let selCont = $state<string | null>(null); // id d’élément, ou « free » pour les équipements libres
  $effect(() => {
    if (!view.locations.some((l) => l.id === selLoc)) selLoc = view.locations[0]?.id ?? null;
    const conts = view.containers.filter((c) => c.location_id === selLoc);
    if (selCont !== 'free' && !conts.some((c) => c.id === selCont)) selCont = conts[0]?.id ?? (selLoc ? 'free' : null);
  });
  // État de l'écran dans l'adresse : ?emplacement=<id>&element=<id|libre>[&vue=nomenclature],
  // partageable et rechargeable. ?i=<exemplaire> (palette de recherche) ouvre son emplacement et
  // son élément, déplie son groupe au besoin, fait défiler sa sous-ligne et la met en évidence.
  let flash = $state<string | null>(null);
  let flashT: ReturnType<typeof setTimeout> | undefined;
  function applyQuery(q: URLSearchParams) {
    if (q.has('vue')) mode = q.get('vue') === 'nomenclature' ? 'b' : 'a';
    const iid = q.get('i');
    const inst = iid ? view.instances.find((x) => x.id === iid) : undefined;
    let art = inst ? view.articles.find((a) => a.id === inst.article_id) : undefined;
    if (art?.host) art = view.articles.find((a) => a.id === art!.host!.article_id) ?? art;
    const l = art?.location_id ?? q.get('emplacement');
    const e = art ? (art.container_id ?? 'libre') : q.get('element');
    if (l && view.locations.some((x) => x.id === l)) {
      selLoc = l;
      selCont = e === 'libre' ? 'free' : e && view.containers.some((c) => c.id === e && c.location_id === l) ? e : null;
    }
    if (inst && art) {
      collapsed.delete(scopeOfArt(art));
      flash = inst.id;
      clearTimeout(flashT); flashT = setTimeout(() => (flash = null), 6000);
      tick().then(() => requestAnimationFrame(() => document.querySelector(`[data-inst="${CSS.escape(inst.id)}"]`)?.scrollIntoView({ block: 'center', behavior: 'smooth' })));
    }
  }
  const loc = $derived(view.locations.find((l) => l.id === selLoc));
  // Un emplacement ne se supprime que vide (ni élément ni matériel) : le menu le dit avant.
  const busyLoc = $derived(!!loc && (loc.totals.containers > 0 || view.articles.some((a) => a.location_id === loc.id)));
  const conts = $derived(view.containers.filter((c) => c.location_id === selLoc));
  const cont = $derived(view.containers.find((c) => c.id === selCont) ?? null);
  const modulesOf = (a: Article) => view.articles.filter((m) => m.host?.article_id === a.id).sort((x, y) => x.host!.slot - y.host!.slot);
  let chassisOf = $state<Article | null>(null);
  const contOf = (a: Article) => (a.container_id ? view.containers.find((c) => c.id === a.container_id) ?? null : null);
  /** Portée d'un article dans l'écran : l'id de son élément, ou « free » (équipements libres). */
  const scopeOfArt = (a: Article) => a.container_id ?? 'free';
  /** Matériel de premier niveau d'une portée (les modules d'un châssis s'affichent sous leur châssis). */
  const scopeArts = (s: string) => view.articles.filter((a) => !a.host && (s === 'free' ? a.location_id === selLoc && !a.container_id : a.container_id === s));
  const piecesOf = (s: string) => scopeArts(s).reduce((n, a) => n + a.quantity, 0);

  // Statut de pose : pour chaque élément rackable affiché, où chaque exemplaire (ou accessoire)
  // est posé dans l'élévation, d'après l'éditeur de racks. Les jauges lisent les totaux du
  // serveur (placed_u, pending_u) sans relire chaque élévation.
  interface Pl { article_id: string; instance_id: string | null; u_start: number; face: string; shelf_id: string | null }
  type RackEl = { placement: Pl; height_u: number; accessory?: { kind: string } | null };
  type RackView = { elements: RackEl[]; pending?: { height_u: number | null; rackable: boolean }[] };
  let racks = $state<Record<string, RackView>>({});
  $effect(() => {
    void view.articles; void view.instances;
    const list = (mode === 'b' ? conts : cont ? [cont] : []).filter((c) => c.rackable);
    for (const c of list) api.get<RackView>(`/containers/${c.id}/rack`).then((r) => { racks[c.id] = r; }, () => {});
  });
  /** U posés dans l'élévation (hors matériel posé sur étagère et PDU verticaux) et U restant à placer. */
  function rackUse(c: Container): { placed: number; todo: number } | null {
    const t = c.totals;
    return t.placed_u == null ? null : { placed: t.placed_u, todo: t.pending_u ?? 0 };
  }
  const fu = (u: number) => nf(u, u % 1 ? 1 : 0);
  const FACE: Record<string, string> = { front: 'avant', rear: 'arrière', side: 'côté' };
  const posText = (e: { placement: Pl; height_u: number }) => {
    if (e.placement.shelf_id) return 'Posé sur étagère';
    const h = Math.max(1, Math.ceil(e.height_u)), u = e.placement.u_start;
    return `U${u}${h > 1 ? `–${u + h - 1}` : ''} · ${nf(e.height_u, e.height_u % 1 ? 1 : 0)} U · ${FACE[e.placement.face] ?? e.placement.face}`;
  };
  const posShort = (e: { placement: Pl; height_u: number }) => {
    if (e.placement.shelf_id) return 'étagère';
    const h = Math.max(1, Math.ceil(e.height_u)), u = e.placement.u_start;
    return `U${u}${h > 1 ? `–${u + h - 1}` : ''}`;
  };
  type St = { kind: 'ok' | 'todo' | 'info' | 'load'; text: string; title?: string };
  /** Statut d'un exemplaire, ou d'un article sans exemplaire (accessoire, panel, adaptateur). */
  function statusOf(a: Article, instId: string | null): St {
    if (a.host) return { kind: 'info', text: `Dans le châssis · S${a.host.slot}` };
    const c = contOf(a);
    if (!c) return { kind: 'info', text: 'Libre dans l’emplacement' };
    if (!c.rackable) return { kind: 'info', text: `Dans ${CONTAINER_KINDS[c.kind].label.toLowerCase()}` };
    const rackable = a.accessory || item(a)?.rackable;
    if (!rackable) return { kind: 'info', text: 'Hors rack' };
    const els = racks[c.id]?.elements;
    if (!els) return { kind: 'load', text: 'Lecture de la pose' };
    const e = els.find((x) => x.placement.article_id === a.id && (instId ? x.placement.instance_id === instId : !x.placement.instance_id));
    return e ? { kind: 'ok', text: `Posé · ${posShort(e)}`, title: posText(e) } : { kind: 'todo', text: 'À placer' };
  }
  /** Statut de chaque pièce d'une ligne : ses exemplaires, ou ses articles sans exemplaire. */
  const unitsOf = (r: Row) => r.list.flatMap((a) => {
    const inst = instOf(a);
    return inst.length ? inst.map((i) => statusOf(a, i.id)) : [statusOf(a, null)];
  });
  /** Statut d'une ligne : un exemplaire, ou le compte des pièces posées. */
  function rowStatus(r: Row): St {
    const units = unitsOf(r);
    const first = units[0] ?? statusOf(r.a, null);
    if (units.length <= 1 || first.kind === 'info' || units.some((u) => u.kind === 'load')) return first;
    const p = units.filter((u) => u.kind === 'ok').length;
    return { kind: p === units.length ? 'ok' : 'todo', text: p ? `${p} / ${units.length} posés` : 'À placer' };
  }
  /** Pastille de statut de pose (pastilles communes). */
  const ST: Record<string, string> = { ok: 'chip ok', todo: 'chip warn', info: 'chip' };
  /** Page de conception de l'élément : éditeur de racks ou de mobilier. */
  const designHref = (c: Container) => (c.kind === 'desk' || c.kind === 'furniture' ? `/projets/${view.project.id}/mobilier/${c.id}` : c.kind === 'casquette' && c.parent_id ? `/projets/${view.project.id}/mobilier/${c.parent_id}/${c.id}` : c.rackable ? `/projets/${view.project.id}/racks/${c.id}` : null);
  const designLabel = (c: Container) => (c.kind === 'desk' || c.kind === 'furniture' || (c.kind === 'casquette' && c.parent_id) ? 'Ouvrir dans Mobilier' : 'Ouvrir dans Racks');
  const instOf = (a: Article) => view.instances.filter((i) => i.article_id === a.id);
  const item = (a: Article) => (a.item_id ? view.items[a.item_id] : undefined);
  const heightOf = (a: Article) => (a.accessory ? (a.accessory.kind === 'panel' ? a.accessory.panel.height_u : a.accessory.kind === 'vertical_pdu' ? null : a.accessory.height_u) : item(a)?.rackable ? item(a)!.height_u : null);

  // Dialogues
  let locEdit = $state<{ id: string | null; rev: number; name: string; code: string } | null>(null);
  let locErr = $state<Record<string, string>>({});
  let contEdit = $state<{ c: Container | null } | null>(null);
  let adding = $state(false);
  let fromTemplate = $state(false);
  /** Exemplaires du matériel d'un élément, pour les tuiles du mur et les postes du pupitre. */
  const gearOf = (c: Container) => {
    const own = new Map(view.articles.filter((a) => a.container_id === c.id).map((a) => [a.id, a.label]));
    return view.instances.filter((i) => own.has(i.article_id)).map((i) => ({ id: i.id, tag: i.tag, label: own.get(i.article_id)! }));
  };
  /** Enregistre les caractéristiques propres d'un élément (mur, postes). */
  async function saveDetails(c: Container, details: ElementDetails): Promise<boolean> {
    const { id, project_id: _p, position: _o, rev, totals: _t, ...data } = c;
    try { await api.patch(`/containers/${id}`, { ...data, details, rev }); await reload(); return true; }
    catch (e) { fail(e); await reload(); return false; }
  }
  /** Ligne de résumé d'un élément selon son type. */
  function specLine(c: Container): string {
    const d = c.details ?? {};
    if (c.rackable) return `${c.height_u} U · ${c.depth_mm ?? '—'} mm`;
    switch (c.kind) {
      case 'monitor_wall': return `${d.wall?.rows ?? 0} × ${d.wall?.cols ?? 0} · ${d.wall?.tiles.length ?? 0} tuile${pl(d.wall?.tiles.length ?? 0)}`;
      case 'desk': case 'operator_position': return `${d.positions?.length ?? 0} poste${pl(d.positions?.length ?? 0)}${d.positions?.[0]?.role && c.kind === 'operator_position' ? ` · ${d.positions[0].role}` : ''}`;
      case 'zone': return [d.surface, d.area_m2 ? `${nf(d.area_m2, 1)} m²` : ''].filter(Boolean).join(' · ') || 'Emplacement libre';
      case 'camera_position': return [d.camera_number ? `CAM ${d.camera_number}` : '', d.mount, d.link].filter(Boolean).join(' · ') || 'Caméra';
      case 'rf_mast': return d.mast_height_m ? `${nf(d.mast_height_m, 1)} m` : 'Mât';
      case 'power': return [d.power_kva ? `${nf(d.power_kva)} kVA` : '', d.phases === 3 ? 'tri' : d.phases === 1 ? 'mono' : '', d.outlet].filter(Boolean).join(' · ') || 'Énergie';
      case 'external_feed': return [d.feed, d.provider].filter(Boolean).join(' · ') || 'Liaison';
      case 'site_point': case 'field_box': return d.point_kind || CONTAINER_KINDS[c.kind].label;
      case 'display': return [d.display_kind, d.diagonal_in ? `${d.diagonal_in}″` : '', d.resolution].filter(Boolean).join(' · ') || 'Écran';
      case 'cable_drum': return [d.drum_count ? `${d.drum_count} ×` : '', d.cable_length_m ? `${nf(d.cable_length_m)} m` : ''].filter(Boolean).join(' ') || 'Touret';
      default: return `${c.length_mm ?? '—'} × ${c.width_mm ?? '—'} mm`;
    }
  }

  async function saveAsTemplate(c: Container) {
    try { const r = await api.post<{ name: string }>(`/containers/${c.id}/save-as-template`, { name: c.name }); toast(`Modèle « ${r.name} » enregistré en brouillon. Activez-le dans Catalogue › Modèles.`); }
    catch (e) { fail(e); }
  }
  // Modèles de régie, au niveau du lieu : tout le contenu du lieu en un modèle, ou un modèle
  // recopié dans le lieu (ses éléments et leur matériel).
  let regiePick = $state<{ lib: { id: string; name: string; members: { name: string }[] }[]; id: string } | null>(null);
  async function saveLocAsRegie() {
    if (!loc || !conts.length) { toast('Cet emplacement ne contient aucun élément.', 'warn'); return; }
    try {
      const r = await api.post<{ id: string }>(`/projects/${view.project.id}/regies`, { name: loc.name, kind: view.project.regie, service_type: view.project.service_type, notes: null, members: conts.map((c) => c.id) });
      const m = await api.post<{ name: string; members: unknown[] }>(`/regies/${r.id}/save-as-template`, {});
      await api.del(`/regies/${r.id}`).catch(() => {});
      toast(`Modèle de régie « ${m.name} » créé avec ${m.members.length} élément${m.members.length > 1 ? 's' : ''} (en brouillon, à activer dans Catalogue › Modèles › Régies).`);
    } catch (e) { fail(e); }
  }
  async function openRegiePick() {
    try { const l = (await api.get<{ regies: any[] }>('/regies')).regies; regiePick = { lib: l, id: l[0]?.id ?? '' }; } catch (e) { fail(e); }
  }
  // La boîte suit l'envoi (bouton bloqué, indicateur) ; une erreur la laisse ouverte.
  async function addRegie() {
    if (!regiePick?.id || !selLoc) return;
    const r = await api.post<{ regie: { members: unknown[] }; skipped_items: number }>(`/projects/${view.project.id}/regies/from-template`, { regie_id: regiePick.id, location_id: selLoc });
    toast(`${r.regie.members.length} élément${r.regie.members.length > 1 ? 's' : ''} ajouté${r.regie.members.length > 1 ? 's' : ''}${r.skipped_items ? ` · ${r.skipped_items} fiche${pl(r.skipped_items)} introuvable${pl(r.skipped_items)} dans le catalogue` : ''}.`);
    regiePick = null; await reload();
  }

  const newLoc = () => { locErr = {}; locEdit = { id: null, rev: 0, name: '', code: '' }; };
  const editLoc = (l: Location) => { locErr = {}; locEdit = { id: l.id, rev: l.rev, name: l.name, code: l.code }; };
  async function saveLoc() {
    locErr = {};
    const l = locEdit!;
    try {
      const r = l.id ? await api.patch<Location>(`/locations/${l.id}`, { rev: l.rev, name: l.name, code: l.code }) : await api.post<Location>(`/projects/${view.project.id}/locations`, { name: l.name, code: l.code });
      toast(l.id ? 'Emplacement enregistré.' : 'Emplacement ajouté.'); locEdit = null; selLoc = r.id; await reload();
    } catch (x) { if (x instanceof ApiError) locErr = Object.fromEntries(x.fields.map((f) => [f.field, f.message])); if (!(x instanceof ApiError && x.fields.length)) fail(x); }
  }
  function pickLoc(id: string) { if (id === selLoc) return; selLoc = id; selCont = null; sel.clear(); }
  function pickCont(id: string) { if (id === selCont) return; selCont = id; sel.clear(); q = ''; }

  // Matériel rangé par nature, une ligne par matériel : les exemplaires d'une même fiche dans un
  // même élément et les accessoires identiques (une pièce par article, chacune posée à sa place)
  // sont regroupés sur une ligne avec un compteur. Un châssis garde sa ligne (ses modules lui
  // sont propres).
  type Row = { key: string; scope: string; a: Article; list: Article[] };
  type Sec = { k: string; label: string; rows: Row[] };
  const SECTIONS: [string, string][] = [['equipment', 'Équipements'], ['panel', 'Panels de brassage'], ['accessory', 'Accessoires'], ['adapter', 'Adaptateurs']];
  const sectionOf = (a: Article) => (a.nature === 'accessory' ? (a.accessory?.kind === 'panel' ? 'panel' : 'accessory') : a.nature);
  function keyOf(a: Article): string {
    const s = a.container_id ?? `free:${a.location_id}`;
    if (a.nature === 'accessory') return `${s}|acc|${a.item_id ?? ''}|${JSON.stringify(a.accessory ?? null)}`;
    if (!a.item_id || item(a)?.modular) return `${s}|${a.id}`;
    return `${s}|${a.nature}|${a.item_id}`;
  }
  // Recherche dans l'élément (ou la nomenclature) et filtre de pose.
  let q = $state('');
  let filter = $state<'all' | 'todo' | 'ok'>('all');
  const instsOf = (r: Row) => r.list.flatMap(instOf);
  function match(r: Row, needle: string): boolean {
    if (!needle) return true;
    const it = item(r.a);
    const mods = r.list.flatMap(modulesOf);
    const text = [r.a.label, it?.brand, it?.reference, ...instsOf(r).flatMap((i) => [i.tag, i.label]), ...mods.flatMap((m) => [m.label, ...instOf(m).map((i) => i.tag)])].filter(Boolean).join(' ').toLowerCase();
    return text.includes(needle);
  }
  /** Lignes d'une portée, regroupées par nature ; `all` ignore la recherche et le filtre. Le
   *  filtre de pose garde un matériel dès qu'une de ses pièces est dans l'état choisi. */
  function build(scope: string, all = false): Sec[] {
    const rows = new Map<string, Row>();
    for (const a of scopeArts(scope)) {
      const k = keyOf(a);
      const r = rows.get(k);
      if (r) r.list.push(a); else rows.set(k, { key: k, scope, a, list: [a] });
    }
    const needle = all ? '' : q.trim().toLowerCase();
    const keep = [...rows.values()].filter((r) => match(r, needle) && (all || filter === 'all' || unitsOf(r).some((u) => u.kind === filter)));
    return SECTIONS.map(([k, label]) => ({ k, label, rows: keep.filter((r) => sectionOf(r.a) === k) })).filter((g) => g.rows.length);
  }
  const scopes = $derived(mode === 'b' ? [...conts.map((c) => c.id), 'free'] : selCont ? [selCont] : []);
  const secsOf = $derived(new Map(scopes.map((s) => [s, build(s)])));
  const qtyOf = (r: Row) => r.list.reduce((n, a) => n + a.quantity, 0);

  // Sélection multiple (cases) et groupes repliés de la nomenclature.
  const sel = new SvelteSet<string>();
  const collapsed = new SvelteSet<string>();
  const shownRows = $derived(scopes.flatMap((s) => (mode === 'b' && collapsed.has(s) ? [] : (secsOf.get(s) ?? []).flatMap((g) => g.rows))));
  const selRows = $derived(scopes.flatMap((s) => (secsOf.get(s) ?? []).flatMap((g) => g.rows)).filter((r) => sel.has(r.key)));
  const allSel = $derived(shownRows.length > 0 && shownRows.every((r) => sel.has(r.key)));
  function toggleAll() {
    if (allSel) for (const r of shownRows) sel.delete(r.key);
    else for (const r of shownRows) sel.add(r.key);
  }
  function toggleRow(r: Row) { if (sel.has(r.key)) sel.delete(r.key); else sel.add(r.key); }
  // Échap désélectionne (sauf dans un champ, une boîte ou un menu ouverts).
  $effect(() => {
    const on = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || !sel.size) return;
      const ck = e.target instanceof HTMLInputElement && e.target.type === 'checkbox';
      if (ck ? modalOpen() || e.defaultPrevented : shortcutBlocked(e)) return;
      sel.clear(); e.preventDefault();
    };
    addEventListener('keydown', on);
    return () => removeEventListener('keydown', on);
  });

  /** + / − sur une ligne : une pièce de plus ou de moins. */
  async function step(r: Row, d: 1 | -1) {
    const last = r.list[r.list.length - 1]!;
    const unit = last.nature === 'accessory' || item(last)?.modular;
    if (d < 0 && qtyOf(r) <= 1) return delRow(r);
    if (!unit && (d > 0 || last.quantity > 1)) return setQty(last, last.quantity + d);
    try {
      if (d > 0) {
        await api.post(`/projects/${view.project.id}/articles`, { container_id: last.container_id, location_id: last.container_id ? null : last.location_id, nature: last.nature, item_id: last.item_id, accessory: last.accessory, quantity: 1 });
      } else {
        await api.del(`/articles/${last.id}`);
        toast(`Un exemplaire de « ${last.label} » retiré.`, 'ok', { run: async () => { try { await api.post(`/projects/${view.project.id}/articles`, { container_id: last.container_id, location_id: last.container_id ? null : last.location_id, nature: last.nature, item_id: last.item_id, accessory: last.accessory, quantity: 1 }); await reload(); } catch (e) { fail(e); } } });
      }
      await reload();
    } catch (e) { fail(e); await reload(); }
  }
  async function setQty(a: Article, n: number) {
    try { await api.patch(`/articles/${a.id}`, { rev: a.rev, quantity: n }); await reload(); } catch (e) { fail(e); await reload(); }
  }
  // Suppressions : confirmation dans la boîte commune, la requête part depuis la boîte.
  async function delArts(arts: Article[], title: string, done: string) {
    const ok = await ask({ title, eyebrow: 'Matériel', text: arts.length > 1 ? 'Leurs exemplaires et leurs ID partent avec eux.' : 'Ses exemplaires et leurs ID partent avec lui.', confirmLabel: 'Supprimer', danger: true,
      run: async () => { for (const a of arts) await api.del(`/articles/${a.id}`); } });
    if (ok) { for (const a of arts) sel.delete(keyOf(a)); toast(done); }
    await reload();
  }
  const delRow = (r: Row) => delArts(r.list, `Supprimer « ${r.a.label} »${qtyOf(r) > 1 ? ` (${qtyOf(r)} pièces)` : ''} ?`, `« ${r.a.label} » retiré de l’inventaire.`);
  const delSel = () => delArts(selRows.flatMap((r) => r.list), `Supprimer ${selRows.length} matériel${selRows.length > 1 ? 's' : ''} ?`, `${selRows.length} matériel${selRows.length > 1 ? 's' : ''} retiré${selRows.length > 1 ? 's' : ''} de l’inventaire.`);
  async function delModule(mo: Article) {
    const ok = await ask({ title: `Retirer « ${mo.label} » du châssis ?`, eyebrow: 'Module', text: 'Le module quitte l’inventaire, avec son ID.', confirmLabel: 'Retirer', danger: true, run: () => api.del(`/articles/${mo.id}`) });
    if (ok) toast(`« ${mo.label} » retiré du châssis.`);
    await reload();
  }
  async function delCont(c: Container) {
    const ok = await ask({ title: `Supprimer « ${c.name} » ?`, eyebrow: 'Élément', text: 'Son matériel et ses exemplaires partent avec lui.', confirmLabel: 'Supprimer', danger: true, run: () => api.del(`/containers/${c.id}`) });
    if (ok) { toast(`Élément « ${c.name} » supprimé, avec son contenu.`); await reload(); }
  }
  async function delLoc(l: Location) {
    const ok = await ask({ title: `Supprimer « ${l.name} » ?`, eyebrow: 'Emplacement', text: 'L’emplacement doit être vide : sans élément ni équipement libre.', confirmLabel: 'Supprimer', danger: true, run: () => api.del(`/locations/${l.id}`) });
    if (ok) { toast(`Emplacement « ${l.name} » supprimé.`); await reload(); }
  }

  // Déplacement. Une destination s'écrit « c:<élément> » ou « l:<emplacement> » (équipements
  // libres). Un équipement en quantité N se déplace exemplaire par exemplaire (le serveur sort un
  // exemplaire à la fois, avec son ID) ; le dernier emporte l'article lui-même.
  let moveBusy = $state(false);
  const targetName = (t: string) => {
    const [k, id] = t.split(':');
    return k === 'c' ? `« ${view.containers.find((c) => c.id === id)?.name ?? 'élément'} »` : `Équipements libres (${view.locations.find((l) => l.id === id)?.name ?? 'emplacement'})`;
  };
  const isAt = (a: Article, t: string) => { const [k, id] = t.split(':'); return k === 'c' ? a.container_id === id : !a.container_id && a.location_id === id; };
  /** La destination accepte-t-elle ce matériel ? (un accessoire reste dans un élément rackable) */
  function accepts(arts: Article[], t: string): boolean {
    if (!writable || !arts.length || arts.every((a) => isAt(a, t))) return false;
    const [k, id] = t.split(':');
    if (!arts.some((a) => a.nature === 'accessory')) return true;
    return k === 'c' && !!view.containers.find((c) => c.id === id)?.rackable;
  }
  async function moveArts(arts: Article[], t: string, only: string | null, what: string) {
    const [k, id] = t.split(':');
    const body = { container_id: k === 'c' ? id : null, location_id: k === 'l' ? id : null };
    const back: { id: string; to: string }[] = [];
    moveBusy = true;
    try {
      for (const a of arts) {
        if (isAt(a, t)) continue;
        const from = a.container_id ? `c:${a.container_id}` : `l:${a.location_id}`;
        if (a.nature === 'equipment' && a.quantity > 1) {
          const ids = only ? [only] : instOf(a).slice(0, a.quantity - 1).map((i) => i.id);
          let rev = a.rev;
          for (const iid of ids) {
            const r = await api.post<Article>(`/articles/${a.id}/move`, { rev, ...body, instance_id: iid });
            back.push({ id: r.id, to: from }); rev += 1;
          }
          if (!only) { const r = await api.post<Article>(`/articles/${a.id}/move`, { rev, ...body, instance_id: null }); back.push({ id: r.id, to: from }); }
        } else {
          const r = await api.post<Article>(`/articles/${a.id}/move`, { rev: a.rev, ...body, instance_id: null });
          back.push({ id: r.id, to: from });
        }
      }
    } catch (e) { fail(e); }
    finally { moveBusy = false; sel.clear(); await reload(); }
    if (back.length) toast(`${what} vers ${targetName(t)}.`, 'ok', { run: () => undoMove(back) });
  }
  async function undoMove(back: { id: string; to: string }[]) {
    moveBusy = true;
    try {
      await reload();
      for (const b of back) {
        const a = view.articles.find((x) => x.id === b.id);
        if (!a) continue;
        const [k, id] = b.to.split(':');
        await api.post(`/articles/${a.id}/move`, { rev: a.rev, container_id: k === 'c' ? id : null, location_id: k === 'l' ? id : null, instance_id: null });
      }
    } finally { moveBusy = false; await reload(); }
  }
  const movedText = (rows: Row[]) => (rows.length === 1 ? `« ${rows[0]!.a.label} » déplacé` : `${rows.length} matériels déplacés`);

  // Boîte « Déplacer vers… » : une ligne, un exemplaire ou toute la sélection.
  let moving = $state<{ rows: Row[]; inst: string; target: string } | null>(null);
  const movingArts = $derived(moving ? moving.rows.flatMap((r) => r.list) : []);
  const moveOne = $derived(movingArts.length === 1 && movingArts[0]!.nature === 'equipment' && movingArts[0]!.quantity > 1 ? movingArts[0]! : null);
  function doMove() {
    const m = moving!, arts = movingArts;
    const only = moveOne && m.inst ? m.inst : null;
    const tag = only ? view.instances.find((i) => i.id === only)?.tag : null;
    moving = null;
    return moveArts(arts, m.target, only, tag ? `Exemplaire ${tag} déplacé` : movedText(m.rows));
  }
  /** Déplace un seul exemplaire (sous-ligne) : la boîte s'ouvre avec lui. */
  function moveInst(r: Row, i: Instance) {
    const a = r.list.find((x) => x.id === i.article_id)!;
    moving = { rows: [{ key: r.key, scope: r.scope, a, list: [a] }], inst: i.id, target: '' };
  }

  // Glisser-déposer : une ligne, ou toute la sélection si la ligne glissée en fait partie.
  // Cibles : lignes d'éléments (vue par élément), lignes de groupe et lignes du groupe
  // (nomenclature), équipements libres. Le geste ne part pas d'un champ ni d'un bouton.
  let drag = $state<{ rows: Row[]; keys: Set<string> } | null>(null);
  let dropOn = $state<string | null>(null);
  let downIn = false;
  const dropTarget = (s: string) => (s === 'free' ? `l:${selLoc}` : `c:${s}`);
  function dragStart(e: DragEvent, r: Row) {
    if (!writable || moveBusy || downIn) { e.preventDefault(); return; }
    const rows = sel.has(r.key) ? selRows : [r];
    drag = { rows, keys: new Set(rows.map((x) => x.key)) };
    if (e.dataTransfer) { e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', rows.map((x) => x.a.label).join(', ')); }
  }
  function dragEnd() { drag = null; dropOn = null; }
  $effect(() => {
    const over = (e: DragEvent) => {
      if (!drag) return;
      const t = e.target instanceof Element ? (e.target.closest('[data-drop]') as HTMLElement | null) : null;
      const s = t?.dataset.drop ?? null;
      if (s && accepts(drag.rows.flatMap((r) => r.list), dropTarget(s))) {
        e.preventDefault();
        if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
        dropOn = s;
      } else dropOn = null;
    };
    const drop = (e: DragEvent) => {
      if (!drag || !dropOn) return;
      e.preventDefault();
      const rows = drag.rows, t = dropTarget(dropOn);
      dragEnd();
      void moveArts(rows.flatMap((r) => r.list), t, null, movedText(rows));
    };
    document.addEventListener('dragover', over);
    document.addEventListener('drop', drop);
    return () => { document.removeEventListener('dragover', over); document.removeEventListener('drop', drop); };
  });

  // Les saisies sur un même exemplaire s'enchaînent : chacune part avec la révision
  // renvoyée par la précédente, sans attendre un rechargement complet.
  const queue = new Map<string, Promise<void>>();
  function saveInstance(i: Instance, field: 'label' | 'serial' | 'mac' | 'inventory_tag', value: string) {
    const prev = queue.get(i.id) ?? Promise.resolve();
    const next = prev.then(async () => {
      const cur = view.instances.find((x) => x.id === i.id) ?? i;
      if ((cur[field] ?? '') === value.trim()) return;
      try {
        const r = await api.patch<Instance>(`/instances/${i.id}`, { rev: cur.rev, label: cur.label, serial: cur.serial, mac: cur.mac, inventory_tag: cur.inventory_tag, [field]: value });
        const k = view.instances.findIndex((x) => x.id === i.id);
        if (k >= 0) view.instances[k] = r;
      } catch (e) { fail(e); await reload(); }
    });
    queue.set(i.id, next);
  }
  const pct = (v: number, max: number | null | undefined) => (max ? (v / max) * 100 : 0);
  /** Blocs de la mini-baie : chaque article rackable, de bas en haut, à l'échelle. */
  function blocks(c: Container) {
    const total = c.height_u ?? 1;
    const out: { h: number; color: string }[] = [];
    for (const a of view.articles.filter((x) => x.container_id === c.id)) {
      const h = heightOf(a);
      if (h) for (let i = 0; i < a.quantity; i++) out.push({ h: (h / total) * 100, color: a.accessory ? 'var(--fg-3)' : 'var(--sig-v)' });
    }
    return out;
  }
  const gaugeTitle = (c: Container) => {
    const use = rackUse(c), H = c.height_u ?? 0, t = c.totals;
    if (H > 0 && t.used_u > H) return `Dépasse de ${fu(t.used_u - H)} U : ${fu(t.used_u)} U affectés pour ${H} U`;
    return `${use ? fu(use.placed) : '0'} U posés${use?.todo ? `, ${fu(use.todo)} U à placer` : ''}, ${H} U disponibles`;
  };
  const allCollapsed = $derived(scopes.length > 0 && scopes.every((s) => collapsed.has(s)));
  const MODES = [{ value: 'a' as Mode, label: 'Par élément' }, { value: 'b' as Mode, label: 'Nomenclature' }];
  const FILTERS = [{ value: 'all' as const, label: 'Tout' }, { value: 'todo' as const, label: 'À placer' }, { value: 'ok' as const, label: 'Posé' }];
  const exportXlsx = () => download(`/projects/${view.project.id}/inventory/export?format=xlsx&group=container`);
  // Lecture initiale de l'adresse, une fois toutes les déclarations faites (applyQuery lit
  // les lignes dépliées et les groupes repliés).
  let seenQ = untrack(() => route.query);
  untrack(() => applyQuery(seenQ));
  $effect(() => { const q = route.query; if (q === seenQ) return; seenQ = q; untrack(() => applyQuery(q)); });
  // L'adresse suit l'emplacement, l'élément et la vue choisis, sans entrée d'historique ni retour
  // en haut de page (navigate() ramènerait la vue en haut à chaque clic).
  $effect(() => {
    if (!selLoc) return;
    const p = new URLSearchParams({ emplacement: selLoc, ...(selCont ? { element: selCont === 'free' ? 'libre' : selCont } : {}), ...(mode === 'b' ? { vue: 'nomenclature' } : {}) });
    if (location.search === `?${p}`) return;
    history.replaceState(history.state, '', `${location.pathname}?${p}`);
    untrack(() => { seenQ = p; route.query = p; });
  });
</script>

{#snippet newLocAction()}<button class="btn primary" onclick={newLoc}><Icon name="plus" size="sm" />Nouvel emplacement</button>{/snippet}

<!-- Vignette d'élément : la mini-baie ou le pictogramme actuels, en réduction. -->
{#snippet thumb(c: Container | null, big: boolean)}
  <span class="thumb" class:big aria-hidden="true">
    {#if !c}<Icon name="box" size="sm" />
    {:else if c.rackable}<span class="mini-rack" class:case={c.kind !== 'rack'}>{#each blocks(c) as b}<i style="height:{b.h}%;--c:{b.color}"></i>{/each}</span>
    {:else}<span class="kind-vis"><KindIll kind={c.kind} size={big ? 44 : 30} /></span>{/if}
  </span>
{/snippet}

<!-- Jauge de hauteur posée : U posés dans l'élévation ; le matériel affecté mais pas encore posé
     est annoncé à part, et un dépassement de la hauteur colore toute la jauge. -->
{#snippet gauge(c: Container, n: number, long: boolean)}
  {@const use = rackUse(c)}
  {@const H = c.height_u ?? 0}
  {@const over = H > 0 && c.totals.used_u > H}
  <span class="gauge" class:err={over} title={gaugeTitle(c)}>
    {#if over}<Vu value={100} mode="prog" cls="err" {n} />{:else}<Vu value={use ? pct(use.placed, H) : 0} mode="load" {n} />{/if}
    <span class="mono g-val">{use ? fu(use.placed) : '—'} / {H} U</span>
    {#if over}<span class="g-note over">{long ? gaugeTitle(c) : `+${fu(c.totals.used_u - H)} U de trop`}</span>
    {:else if use?.todo}<span class="g-note">+{fu(use.todo)} U à placer</span>{/if}
  </span>
{/snippet}

<!-- Sélecteur d'emplacement et actions de l'emplacement (en tête de la liste ou de la barre). -->
{#snippet locPicker()}
  {#if loc}
    <div class="loc-pick">
      <span class="eyebrow">Emplacement</span>
      {#if view.locations.length > 1}
        <Menu label="Changer d’emplacement" btnClass="btn ghost sm loc-btn" align="left" menuLabel="Emplacements">
          {#snippet trigger()}<b>{loc.name}</b><span class="mono muted">{loc.code}</span><Icon name="chevd" size="sm" />{/snippet}
          {#snippet children()}
            {#each view.locations as l (l.id)}
              <button role="menuitemradio" aria-checked={l.id === selLoc} onclick={() => pickLoc(l.id)}>
                <span class="mono loc-code">{l.code || '—'}</span>
                <span class="mi"><b>{l.name}</b><small class="pm-sub">{l.totals.containers} élément{l.totals.containers > 1 ? 's' : ''}{l.totals.free_items ? ` · ${l.totals.free_items} libre${l.totals.free_items > 1 ? 's' : ''}` : ''}</small></span>
                {#if l.id === selLoc}<Icon name="check" size="sm" />{/if}
              </button>
            {/each}
          {/snippet}
        </Menu>
      {:else}
        <span class="loc-one"><b>{loc.name}</b><span class="mono muted">{loc.code}</span></span>
      {/if}
    </div>
    <Menu label="Actions de l’emplacement" iconOnly icon="more" btnClass="icon-btn sm">
      {#snippet children()}
        {#if writable}
          <button role="menuitem" onclick={() => editLoc(loc)}><Icon name="pen" size="sm" /><span class="mi"><b>Renommer l’emplacement</b><small class="pm-sub">Nom et code câble</small></span></button>
          <button role="menuitem" onclick={newLoc}><Icon name="plus" size="sm" /><span class="mi"><b>Nouvel emplacement</b></span></button>
          <hr aria-hidden="true">
          <button role="menuitem" onclick={() => (fromTemplate = true)}><Icon name="case" size="sm" /><span class="mi"><b>Ajouter un élément depuis un modèle</b><small class="pm-sub">Un rack, un flight-case, avec son contenu</small></span></button>
          <button role="menuitem" onclick={openRegiePick}><Icon name="layers" size="sm" /><span class="mi"><b>Ajouter une régie depuis un modèle</b><small class="pm-sub">Plusieurs éléments d’un coup, dans cet emplacement</small></span></button>
          {#if can('catalogue', 'ecriture')}<button role="menuitem" onclick={saveLocAsRegie}><Icon name="copy" size="sm" /><span class="mi"><b>Enregistrer comme modèle de régie</b><small class="pm-sub">Tous ses éléments et leur matériel</small></span></button>{/if}
          <hr aria-hidden="true">
        {/if}
        <button role="menuitem" onclick={exportXlsx}><Icon name="download" size="sm" /><span class="mi"><b>Exporter la nomenclature (Excel)</b><small class="pm-sub">Tout le matériel du projet, élément par élément</small></span></button>
        {#if writable}
          <hr aria-hidden="true">
          <button role="menuitem" class="danger" disabled={busyLoc} onclick={() => delLoc(loc)}><Icon name="trash" size="sm" /><span class="mi"><b>Supprimer l’emplacement</b>{#if busyLoc}<small class="pm-sub">Videz-le d’abord : déplacez ou supprimez son contenu</small>{/if}</span></button>
        {/if}
      {/snippet}
    </Menu>
  {/if}
{/snippet}

<!-- Actions d'un élément (en-tête de l'élément ouvert, ligne de groupe de la nomenclature). -->
{#snippet elMenu(c: Container, inB: boolean)}
  {#if writable || inB}
    <Menu label="Actions de « {c.name} »" iconOnly icon="more" btnClass="icon-btn sm">
      {#snippet children()}
        {#if inB}<button role="menuitem" onclick={() => { selCont = c.id; setMode('a'); }}><Icon name="list" size="sm" /><span class="mi"><b>Ouvrir dans la vue par élément</b></span></button>{/if}
        {#if writable}
          <button role="menuitem" onclick={() => (contEdit = { c })}><Icon name="pen" size="sm" /><span class="mi"><b>Modifier l’élément</b><small class="pm-sub">Nom, type, dimensions, limites</small></span></button>
          {#if can('catalogue', 'ecriture')}<button role="menuitem" onclick={() => saveAsTemplate(c)}><Icon name="copy" size="sm" /><span class="mi"><b>Enregistrer comme modèle</b><small class="pm-sub">Copie l’élément et son contenu dans la bibliothèque</small></span></button>{/if}
          <hr aria-hidden="true">
          <button role="menuitem" class="danger" onclick={() => delCont(c)}><Icon name="trash" size="sm" /><span class="mi"><b>Supprimer l’élément</b></span></button>
        {/if}
      {/snippet}
    </Menu>
  {/if}
{/snippet}

<!-- Une ligne de matériel, puis une sous-ligne par exemplaire (ID, libellé éditable, pose) et par
     module de châssis, toujours affichées ; identique dans les deux vues. -->
{#snippet rowLines(r: Row, inB: boolean)}
  {@const a = r.a}
  {@const it = item(a)}
  {@const inst = instsOf(r)}
  {@const mods = it?.modular ? modulesOf(a) : []}
  {@const subs = [...inst.map((i) => ({ i, mo: null as Article | null })), ...mods.flatMap((mo) => instOf(mo).map((i) => ({ i, mo })))]}
  {@const st = rowStatus(r)}
  {@const h = heightOf(a)}
  {@const c = contOf(a)}
  {@const rackHref = c && c.rackable && (a.accessory || it?.rackable) ? designHref(c) : null}
  <tr class="row" class:has-sub={subs.length > 0} class:sel={sel.has(r.key)} class:drag-src={drag?.keys.has(r.key)} data-drop={inB ? r.scope : undefined}
    draggable={writable && !moveBusy} onpointerdown={(e) => (downIn = !!(e.target as Element).closest('input, button, a, select, textarea, .stepper, .menu-wrap'))}
    ondragstart={(e) => dragStart(e, r)} ondragend={dragEnd}>
    {#if writable}
      <td class="ck"><input type="checkbox" checked={sel.has(r.key)} onchange={() => toggleRow(r)} aria-label="Sélectionner {a.label}"></td>
      <td class="c-grip"><span class="grip" title="Glisser vers un autre élément"><Icon name="grip" size="sm" /></span></td>
    {/if}
    <td class="c-name">
      <span class="nm-txt" title="{a.label}{it?.weight_kg ? ` · ${nf(it.weight_kg * qtyOf(r), 1)} kg` : ''}{it?.power_w ? ` · ${nf(it.power_w * qtyOf(r))} W` : ''}">
        <b class="nm">{a.label}</b>
        <span class="br">{a.accessory?.kind === 'panel' ? `Panel · ${a.accessory.panel.slots.length} connecteur${a.accessory.panel.slots.length > 1 ? 's' : ''}` : a.nature === 'adapter' ? 'Adaptateur' : ''}{it?.brand ? `${a.accessory?.kind === 'panel' || a.nature === 'adapter' ? ' · ' : ''}${it.brand}` : ''}{it && !it.active ? ' · fiche inactive' : ''}</span>
        {#if it?.modular}<span class="chip"><Icon name="grid" size="sm" />{mods.length} module{mods.length > 1 ? 's' : ''}</span>{/if}
        {#if !inst.length}<span class="no-id">Sans ID</span>{/if}
      </span>
    </td>
    <td class="c-u num mono">{h ? `${fu(h)} U` : '—'}</td>
    <td class="c-qty">{#if writable}<span class="stepper" title="Quantité"><button onclick={() => step(r, -1)} aria-label="Retirer un {a.label}" disabled={moveBusy}>−</button><span>{qtyOf(r)}</span><button onclick={() => step(r, 1)} aria-label="Ajouter un {a.label}" disabled={moveBusy}>+</button></span>{:else}<span class="mono">× {qtyOf(r)}</span>{/if}</td>
    <!-- Exemplaire unique : son état est sur sa sous-ligne, pas en double ici. -->
    <td class="c-st">{#if inst.length === 1 && !it?.modular}{:else if st.kind === 'load'}<Loading inline label={st.text} />{:else}<span class={ST[st.kind]} title={st.title}>{st.text}</span>{/if}</td>
    <td class="acts">
      <Menu label="Actions pour {a.label}" iconOnly icon="more" btnClass="icon-btn sm">
        {#snippet children()}
          {#if writable}<button role="menuitem" onclick={() => (moving = { rows: [r], inst: '', target: '' })}><Icon name="arrowr" size="sm" /><span class="mi"><b>Déplacer vers…</b><small class="pm-sub">Un autre élément ou les équipements libres</small></span></button>{/if}
          {#if rackHref}<a role="menuitem" href={rackHref}><Icon name="server" size="sm" /><span class="mi"><b>Poser dans le rack</b><small class="pm-sub">{designLabel(c!)}</small></span></a>{/if}
          {#if it?.modular}<button role="menuitem" onclick={() => (chassisOf = a)}><Icon name="grid" size="sm" /><span class="mi"><b>Modules du châssis</b><small class="pm-sub">{mods.length} module{mods.length > 1 ? 's' : ''} glissé{mods.length > 1 ? 's' : ''}</small></span></button>{/if}
          {#if a.accessory?.kind === 'panel'}<button role="menuitem" onclick={() => (panelOf = a)}><Icon name="panel" size="sm" /><span class="mi"><b>{writable ? 'Modifier le panel' : 'Voir le panel'}</b></span></button>{/if}
          {#if writable}<hr aria-hidden="true"><button role="menuitem" class="danger" onclick={() => delRow(r)}><Icon name="trash" size="sm" /><span class="mi"><b>Supprimer de l’inventaire</b></span></button>{/if}
        {/snippet}
      </Menu>
    </td>
  </tr>
  {#each subs as { i, mo }, n (i.id)}
    {@const owner = mo ?? r.list.find((x) => x.id === i.article_id) ?? a}
    {@const ist = statusOf(owner, i.id)}
    {@const mi = mo ? item(mo) : undefined}
    <tr class="sub" class:mod={!!mo} class:last={n === subs.length - 1} class:drag-src={drag?.keys.has(r.key)} class:focus={flash === i.id} data-inst={i.id} data-drop={inB ? r.scope : undefined}>
      {#if writable}<td class="ck"></td><td class="c-grip"></td>{/if}
      <td class="sub-main" colspan="3">
        <span class="sub-line">
          <span class="tag mono">{i.tag}</span>
          {#if mo}<span class="tag slot mono">S{mo.host!.slot}</span><span class="mod-name">{mo.label}{mi?.brand ? ` · ${mi.brand}` : ''}</span>
          {:else}<input class="input cell" value={i.label ?? ''} readonly={!writable} placeholder="Libellé libre" maxlength="80" onchange={(e) => saveInstance(i, 'label', (e.target as HTMLInputElement).value)} aria-label="Libellé de {i.tag}">{/if}
        </span>
      </td>
      <td class="c-st">{#if ist.kind === 'load'}<Loading inline label={ist.text} />{:else}<span class={ST[ist.kind]} title={ist.title}>{ist.text}</span>{/if}</td>
      <td class="acts">
        {#if writable && mo}<button class="icon-btn sm danger" title="Retirer du châssis" aria-label="Retirer {mo.label} du châssis" onclick={() => delModule(mo)}><Icon name="x" size="sm" /></button>
        {:else if writable && owner.nature === 'equipment'}<button class="icon-btn sm" title="Déplacer cet exemplaire" aria-label="Déplacer l’exemplaire {i.tag}" onclick={() => moveInst(r, i)}><Icon name="arrowr" size="sm" /></button>{/if}
      </td>
    </tr>
  {/each}
{/snippet}

{#snippet thead(label: string)}
  <thead><tr>
    {#if writable}<th class="ck"><input type="checkbox" checked={allSel} disabled={!shownRows.length} onchange={toggleAll} aria-label="Tout sélectionner"></th><th class="c-grip"><span class="sr-only">Glisser</span></th>{/if}
    <th class="c-name">{label}</th><th class="c-u num">U</th><th class="c-qty">Qté</th><th class="c-st">État</th><th class="acts"><span class="sr-only">Actions</span></th>
  </tr></thead>
{/snippet}

{#snippet selBar()}
  {#if writable && selRows.length}
    <div class="selbar" role="region" aria-label="Sélection">
      <b class="sel-n">{selRows.length} sélectionné{selRows.length > 1 ? 's' : ''}</b>
      <button class="btn sm" disabled={moveBusy} onclick={() => (moving = { rows: [...selRows], inst: '', target: '' })}><Icon name="arrowr" size="sm" />Déplacer vers…</button>
      <button class="btn sm ghost danger" disabled={moveBusy} onclick={delSel}><Icon name="trash" size="sm" />Supprimer</button>
      <span class="grow"></span>
      <button class="btn sm ghost" onclick={() => sel.clear()} title="Désélectionner (Échap)">Désélectionner</button>
    </div>
  {/if}
{/snippet}

<div class="inv">
  {#if !view.locations.length}
    <EmptyState icon="pin" message="Aucun emplacement" hint={writable ? 'Commencez par en créer un : la régie, le bord du terrain, la cabine.' : 'Aucun emplacement n’a encore été créé dans ce projet.'} action={writable ? newLocAction : undefined} />
  {/if}

  {#if loc}
    <HeadActions>
      <button class="btn" onclick={exportXlsx} title="Fichier Excel de tout le matériel du projet, élément par élément : liste de colisage, chiffrage, logistique"><Icon name="download" size="sm" />Exporter</button>
      {#if writable}<button class="btn primary" onclick={() => (adding = true)} title={cont ? `Dans « ${cont.name} »` : 'Dans les équipements libres'}><Icon name="plus" size="sm" />Ajouter du matériel</button>{/if}
    </HeadActions>

    {#if mode === 'a'}
      <div class="a-grid">
        <aside class="card a-side" aria-label="Éléments">
          <div class="side-head">{@render locPicker()}</div>
          <div class="side-list">
            {#each conts as c (c.id)}
              <button class="el" class:drop-ok={dropOn === c.id} class:child={!!c.parent_id && conts.some((p) => p.id === c.parent_id)} aria-current={c.id === selCont ? 'true' : undefined} data-drop={c.id} onclick={() => pickCont(c.id)}>
                {@render thumb(c, false)}
                <span class="el-t">
                  <span class="el-n">{c.name}</span>
                  {#if c.rackable}{@render gauge(c, 14, false)}{:else}<span class="el-k">{CONTAINER_KINDS[c.kind].label}</span>{/if}
                </span>
                <span class="el-cnt mono" title="Pièces">{piecesOf(c.id)}</span>
              </button>
            {/each}
            <button class="el free" class:drop-ok={dropOn === 'free'} aria-current={selCont === 'free' ? 'true' : undefined} data-drop="free" onclick={() => pickCont('free')}>
              {@render thumb(null, false)}
              <span class="el-t"><span class="el-n">Équipements libres</span><span class="el-k">Sans élément</span></span>
              <span class="el-cnt mono" title="Pièces">{loc.totals.free_items}</span>
            </button>
            {#if writable}<button class="btn ghost sm el-add" onclick={() => (contEdit = { c: null })}><Icon name="plus" size="sm" />Nouvel élément</button>{/if}
          </div>
        </aside>

        <div class="a-main">
          <section class="card elhead" aria-label={cont ? cont.name : 'Équipements libres'}>
            <div class="who">
              {@render thumb(cont, true)}
              <div class="who-t">
                <span class="who-chips">{#if cont}<span class="chip">{CONTAINER_KINDS[cont.kind].label}</span>{#if cont.wheels}<span class="chip">Roulettes</span>{/if}{:else}<span class="chip">Sans élément</span>{/if}</span>
                <h2 class="el-title">{cont ? cont.name : 'Équipements libres'}</h2>
                <span class="mono small muted">{#if cont}{cont.cable_code} · {specLine(cont)}{cont.totals.volume_m3 ? ` · ${nf(cont.totals.volume_m3, 2)} m³` : ''}{:else}Comptés dans la nomenclature et les totaux · {loc.code}{/if}</span>
              </div>
            </div>
            <div class="stats">
              {#if cont?.rackable}<span class="kv kv-g"><span>Hauteur posée</span>{@render gauge(cont, 24, true)}</span>{/if}
              <span class="kv"><span>Pièces</span><b class="mono">{selCont ? piecesOf(selCont) : 0}</b></span>
              <span class="kv" class:err={cont?.totals.over_weight}><span>Poids</span><b class="mono">{nf(cont ? cont.totals.weight_kg : loc.totals.weight_kg, 1)}{cont?.max_weight_kg ? ` / ${nf(cont.max_weight_kg)}` : ''} kg</b></span>
              <span class="kv" class:err={cont?.totals.over_power}><span>Conso</span><b class="mono">{nf(cont ? cont.totals.power_w : loc.totals.power_w)}{cont?.max_power_w ? ` / ${nf(cont.max_power_w)}` : ''} W</b></span>
            </div>
            {#if cont}
              <div class="el-acts">
                {#if designHref(cont)}<a class="btn" href={designHref(cont)}><Icon name={cont.rackable && !(cont.kind === 'casquette' && cont.parent_id) ? 'server' : 'layers'} size="sm" />{designLabel(cont)}</a>{/if}
                {@render elMenu(cont, false)}
              </div>
            {/if}
          </section>

          {#if cont?.kind === 'monitor_wall' && cont.details?.wall}
            <section class="card"><WallView wall={cont.details.wall} screens={gearOf(cont)} {writable} onsave={(w) => saveDetails(cont, { ...cont.details, wall: w })} /></section>
          {:else if (cont?.kind === 'desk' || cont?.kind === 'operator_position') && cont.details?.positions?.length}
            <section class="card"><DeskView positions={cont.details.positions} gear={gearOf(cont)} {writable} onsave={(ps) => saveDetails(cont, { ...cont.details, positions: ps })} /></section>
          {/if}

          <div class="toolbar">
            <label class="search"><Icon name="search" /><input class="input" type="search" bind:value={q} placeholder="Rechercher dans {cont ? cont.name : 'les équipements libres'}" aria-label="Rechercher dans l’élément"></label>
            {#if cont?.rackable}<Seg options={FILTERS} bind:value={filter} label="Filtrer par pose" />{/if}
            <span class="grow"></span>
            <Seg options={MODES} value={mode} label="Vue" onchange={setMode} />
            {#if writable}<button class="btn" onclick={() => (adding = true)}><Icon name="plus" size="sm" />Ajouter ici</button>{/if}
          </div>

          {@render selBar()}

          <div class="card tbl-card dense">
            <table class="tbl dense inv-tbl">
              {@render thead('Matériel et exemplaires')}
              <tbody>
                {#each secsOf.get(selCont ?? '') ?? [] as g (g.k)}
                  <tr class="grp"><th colspan={writable ? 7 : 5} scope="colgroup"><span class="eyebrow">{g.label}</span><span class="grp-n mono">{g.rows.reduce((n, r) => n + qtyOf(r), 0)}</span></th></tr>
                  {#each g.rows as r (r.key)}{@render rowLines(r, false)}{/each}
                {:else}
                  <tr><td colspan={writable ? 7 : 5}>
                    {#if q.trim() || filter !== 'all'}<EmptyState card={false} icon="search" message="Aucun matériel ne correspond" hint="Changez la recherche ou le filtre de pose." />
                    {:else}<EmptyState card={false} icon="box" message="Aucun matériel ici" hint={writable ? 'Ajoutez-en avec « Ajouter du matériel », ou glissez une ligne depuis un autre élément.' : ''} />{/if}
                  </td></tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    {:else}
      <div class="toolbar b-bar">
        <div class="b-loc">{@render locPicker()}</div>
        <label class="search"><Icon name="search" /><input class="input" type="search" bind:value={q} placeholder="Matériel, marque, ID, libellé" aria-label="Rechercher dans la nomenclature"></label>
        <Seg options={FILTERS} bind:value={filter} label="Filtrer par pose" />
        <span class="grow"></span>
        <button class="btn ghost" onclick={() => { if (allCollapsed) collapsed.clear(); else for (const s of scopes) collapsed.add(s); }}><Icon name={allCollapsed ? 'chevd' : 'chevr'} size="sm" />{allCollapsed ? 'Tout déplier' : 'Tout replier'}</button>
        <Seg options={MODES} value={mode} label="Vue" onchange={setMode} />
        {#if writable}<button class="btn" onclick={() => (contEdit = { c: null })}><Icon name="plus" size="sm" />Nouvel élément</button>{/if}
      </div>

      {@render selBar()}

      <div class="card tbl-card dense">
        <table class="tbl dense inv-tbl b-tbl">
          {@render thead('Matériel et exemplaires')}
          {#each scopes as s (s)}
            {@const c = s === 'free' ? null : view.containers.find((x) => x.id === s) ?? null}
            {@const rows = (secsOf.get(s) ?? []).flatMap((g) => g.rows)}
            {@const shut = collapsed.has(s)}
            <tbody class="b-group">
              <tr class="b-grp" class:drop-ok={dropOn === s} data-drop={s}>
                <td class="b-g-main" colspan={writable ? 6 : 4}>
                  <span class="gtitle">
                    <button class="icon-btn xs caret" aria-expanded={!shut} aria-label="{shut ? 'Déplier' : 'Replier'} {c ? c.name : 'Équipements libres'}" title={shut ? 'Déplier' : 'Replier'} onclick={() => (shut ? collapsed.delete(s) : collapsed.add(s))}><Icon name="chevr" size="sm" /></button>
                    {@render thumb(c, false)}
                    <b class="g-name">{c ? c.name : 'Équipements libres'}</b>
                    <span class="chip">{c ? CONTAINER_KINDS[c.kind].label : 'Sans élément'}</span>
                    <span class="mono small muted">{s === 'free' ? loc.totals.free_items : piecesOf(s)} pièce{(s === 'free' ? loc.totals.free_items : piecesOf(s)) > 1 ? 's' : ''}</span>
                    {#if c?.rackable}<span class="g-gauge">{@render gauge(c, 14, false)}</span>{/if}
                  </span>
                </td>
                <td class="acts">
                  <span class="g-acts">
                    {#if c && designHref(c)}<a class="icon-btn sm" href={designHref(c)} title={designLabel(c)} aria-label="{designLabel(c)} : {c.name}"><Icon name={c.rackable && !(c.kind === 'casquette' && c.parent_id) ? 'server' : 'layers'} size="sm" /></a>{/if}
                    {#if writable}<button class="icon-btn sm" title="Ajouter ici" aria-label="Ajouter du matériel dans {c ? c.name : 'les équipements libres'}" onclick={() => { selCont = s; adding = true; }}><Icon name="plus" size="sm" /></button>{/if}
                    {#if c}{@render elMenu(c, true)}{/if}
                  </span>
                </td>
              </tr>
              {#if !shut}
                {#each rows as r (r.key)}{@render rowLines(r, true)}
                {:else}
                  <tr class="b-drop" class:drop-ok={dropOn === s} data-drop={s}><td colspan={writable ? 7 : 5}>{#if scopeArts(s).length}Aucun matériel ne correspond{:else}Vide{writable ? ' · glissez du matériel ici' : ''}{/if}</td></tr>
                {/each}
              {/if}
            </tbody>
          {/each}
        </table>
      </div>
    {/if}
  {/if}
</div>

{#if locEdit}
  <Dialog title={locEdit.id ? 'Modifier l’emplacement' : 'Nouvel emplacement'} eyebrow={view.project.name} onclose={() => (locEdit = null)} onsubmit={saveLoc} submitLabel={locEdit.id ? 'Enregistrer' : 'Créer'}>
    <div class="stack">
      <div class="field"><label for="l-name">Nom</label><input class="input" class:err={!!locErr.name} id="l-name" bind:value={locEdit.name} required maxlength="120" placeholder="Ex. Bord terrain" aria-invalid={!!locErr.name} aria-describedby={locErr.name ? 'l-name-err' : undefined}><FieldError for="l-name" message={locErr.name} /></div>
      <div class="field"><label for="l-code">Code câble</label><input class="input mono" class:err={!!locErr.code} id="l-code" bind:value={locEdit.code} maxlength="5" placeholder="Tiré du nom" aria-invalid={!!locErr.code} aria-describedby={locErr.code ? 'l-code-help l-code-err' : 'l-code-help'}><span class="help" id="l-code-help">5 caractères au plus ; il entre dans la numérotation des câbles.</span><FieldError for="l-code" message={locErr.code} /></div>
    </div>
  </Dialog>
{/if}
{#if contEdit && selLoc}<ContainerDialog {view} container={contEdit.c} locationId={selLoc} onclose={() => (contEdit = null)} ondone={async (id) => { contEdit = null; await reload(); const c = view.containers.find((x) => x.id === id); if (c) { selLoc = c.location_id; selCont = id; } }} />{/if}
{#if fromTemplate && selLoc}<TemplatePick {view} locationId={selLoc} onclose={() => (fromTemplate = false)} ondone={async (id) => { fromTemplate = false; await reload(); selCont = id; }} />{/if}
{#if adding && selLoc}<AddArticle {view} container={cont} locationId={selLoc} onclose={() => (adding = false)} ondone={async () => { await reload(); }} />{/if}
{#if regiePick}
  <Dialog title="Ajouter une régie depuis un modèle" eyebrow={loc?.name ?? ''} onclose={() => (regiePick = null)} onsubmit={addRegie} submitLabel="Ajouter dans {loc?.name ?? 'l’emplacement'}" submitDisabled={!regiePick.id}>
    {#if !regiePick.lib.length}<p class="muted">Aucun modèle de régie : enregistrez un emplacement comme modèle, ou créez-en un dans <a class="ilink" href="/catalogue/regies">Catalogue › Modèles › Régies</a>.</p>
    {:else}
      <div class="field"><label for="rp-m">Modèle</label><select class="input" id="rp-m" bind:value={regiePick.id} aria-describedby="rp-m-help">{#each regiePick.lib as r}<option value={r.id}>{r.name} · {r.members.length} élément{r.members.length > 1 ? 's' : ''}</option>{/each}</select>
        <span class="help" id="rp-m-help">{regiePick.lib.find((r) => r.id === regiePick!.id)?.members.map((m) => m.name).join(', ') || 'Modèle sans élément'}</span></div>
    {/if}
  </Dialog>
{/if}
{#if moving}
  {@const hasAcc = movingArts.some((a) => a.nature === 'accessory')}
  <Dialog title={moving.rows.length === 1 ? `Déplacer « ${moving.rows[0]!.a.label} »` : `Déplacer ${moving.rows.length} matériels`} eyebrow={view.project.name} onclose={() => (moving = null)} onsubmit={doMove} submitLabel="Déplacer" submitDisabled={!moving.target}>
    <div class="stack">
      {#if moveOne}
        <div class="field"><label for="mv-i">Exemplaires à déplacer</label><select class="input" id="mv-i" bind:value={moving.inst} aria-describedby="mv-i-help"><option value="">Tous les exemplaires ({moveOne.quantity})</option>{#each instOf(moveOne) as i}<option value={i.id}>{i.tag}{i.label ? ` · ${i.label}` : ''}{i.serial ? ` · ${i.serial}` : ''}</option>{/each}</select><span class="help" id="mv-i-help">Chaque exemplaire part avec son ID.</span></div>
      {/if}
      <div class="field"><label for="mv-t">Vers</label>
        <select class="input" id="mv-t" bind:value={moving.target} aria-describedby={hasAcc ? 'mv-t-help' : undefined}>
          <option value="" disabled>Choisir une destination</option>
          {#each view.locations as l}
            <optgroup label={l.name}>
              {#each view.containers.filter((c) => c.location_id === l.id) as c}{#if !movingArts.every((a) => a.container_id === c.id)}<option value="c:{c.id}" disabled={hasAcc && !c.rackable}>{c.name}</option>{/if}{/each}
              {#if !movingArts.every((a) => !a.container_id && a.location_id === l.id)}<option value="l:{l.id}" disabled={hasAcc}>Équipements libres de {l.name}</option>{/if}
            </optgroup>
          {/each}
        </select>
        {#if hasAcc}<span class="help" id="mv-t-help">Un accessoire se pose dans un élément rackable : les autres destinations sont grisées.</span>{/if}
      </div>
    </div>
  </Dialog>
{/if}

{#if chassisOf}<ChassisDialog {view} host={chassisOf} {writable} onclose={() => (chassisOf = null)} ondone={reload} />{/if}
{#if panelOf}<ProjectPanel article={panelOf} readonly={!writable} eyebrow="Panel du projet" onclose={() => (panelOf = null)} ondone={reload} />{/if}

<style>
  .inv { display: flex; flex-direction: column; gap: 14px; min-width: 0; }
  [role^='menuitem'] > .mi { display: flex; flex-direction: column; gap: 1px; min-width: 0; flex: 1; }
  .loc-code { min-width: 44px; font-size: var(--fs-sm); color: var(--fg-3); }

  /* ---------- Vue par élément : deux colonnes ---------- */
  .a-grid { display: grid; grid-template-columns: 300px minmax(0, 1fr); gap: 14px; align-items: start; }
  .a-side { padding: 8px; display: flex; flex-direction: column; gap: 2px; position: sticky; top: 12px; min-width: 0; }
  .side-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 6px; padding: 4px 4px 8px 8px; }
  .loc-pick { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; min-width: 0; }
  .loc-one { display: inline-flex; align-items: baseline; gap: 8px; font: 700 var(--fs-lg)/1.25 var(--display); }
  .loc-one .mono, :global(.loc-btn) .mono { font: 600 var(--fs-xs) var(--mono); }
  :global(.btn.loc-btn) { margin-left: -11px; font-family: var(--display); }
  .side-list { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .el { display: flex; gap: 10px; align-items: center; width: 100%; padding: 8px; border: 0; border-radius: var(--r-sm); background: transparent; color: inherit; text-align: left; font: inherit; cursor: pointer; transition: background .12s; }
  .el:hover { background: var(--surface-2); }
  .el[aria-current='true'] { background: var(--accent-soft); }
  .el:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .el.child { padding-left: 20px; }
  .el-t { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 3px; }
  .el-n { font-weight: 650; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .el[aria-current='true'] .el-n { color: var(--accent-2); }
  .el-k { font-size: var(--fs-xs); color: var(--fg-3); }
  .el-cnt { font-size: var(--fs-xs); font-weight: 600; color: var(--fg-3); }
  .el-add { align-self: flex-start; margin-top: 6px; }
  .drop-ok, .tbl tr.drop-ok td { box-shadow: inset 0 0 0 2px var(--accent); background: var(--accent-soft); }
  .tbl tr.drop-ok td { box-shadow: inset 0 2px 0 var(--accent), inset 0 -2px 0 var(--accent); }

  /* Vignettes : les illustrations des éléments en réduction. */
  .thumb { flex: none; display: grid; place-items: center; width: 30px; height: 38px; border-radius: 6px; background: var(--surface-2); box-shadow: inset 0 0 0 1px var(--line); color: var(--fg-3); overflow: hidden; }
  .thumb.big { width: 44px; height: 56px; border-radius: var(--r-sm); }
  .thumb .mini-rack { width: 18px; height: 30px; padding: 2px 3px 3px; gap: 0; border-radius: 2px; box-shadow: inset 0 0 0 1px rgba(0, 0, 0, .6); }
  .thumb.big .mini-rack { width: 26px; height: 46px; padding: 3px 4px 4px; }
  .thumb .mini-rack i::before, .thumb .mini-rack.case::after { display: none; }
  .thumb .mini-rack.case { box-shadow: inset 0 0 0 2px var(--ill-alu); }
  .kind-vis { display: grid; place-items: center; --ki: var(--sig-v); }

  /* Jauge de hauteur (liste, en-tête, groupes de la nomenclature). */
  .gauge { display: inline-flex; flex-wrap: wrap; align-items: center; gap: 2px 8px; min-width: 0; font-size: var(--fs-xs); color: var(--fg-3); }
  .gauge :global(.vu) { width: 64px; min-width: 0; height: 7px; }
  .g-val { font: 600 var(--fs-xs) var(--mono); color: var(--fg-2); white-space: nowrap; }
  .gauge.err .g-val { color: var(--err); font-weight: 700; }
  .g-note { white-space: nowrap; }
  .g-note.over { color: var(--err); font-weight: 600; white-space: normal; }

  .a-main { display: flex; flex-direction: column; gap: 12px; min-width: 0; }
  .elhead { padding: 14px 16px; display: flex; flex-wrap: wrap; gap: 12px 22px; align-items: center; }
  .who { display: flex; gap: 12px; align-items: center; min-width: 0; flex: 1 1 260px; }
  .who-t { display: flex; flex-direction: column; align-items: flex-start; gap: 3px; min-width: 0; }
  .who-chips { display: flex; gap: 6px; }
  .el-title { margin: 0; font: 700 var(--fs-xl)/1.2 var(--display); letter-spacing: -.015em; overflow-wrap: anywhere; }
  .stats { display: flex; gap: 10px 20px; align-items: flex-start; flex-wrap: wrap; }
  .kv { display: flex; flex-direction: column; gap: 3px; font-size: var(--fs-xs); color: var(--fg-3); }
  .kv b { font-size: var(--fs-ctl-sm); font-weight: 600; color: var(--fg); white-space: nowrap; }
  .kv.err b { color: var(--err); font-weight: 700; }
  .kv-g { max-width: 260px; }
  .kv-g .gauge :global(.vu) { width: 120px; }
  .el-acts { display: flex; gap: 8px; align-items: center; margin-left: auto; }

  .toolbar { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .toolbar .grow { flex: 1; }
  .toolbar .search { flex: 1 1 220px; max-width: 340px; }
  .b-bar { gap: 8px 10px; }
  .b-loc { display: flex; align-items: center; gap: 4px; margin-right: 6px; }
  .b-loc .loc-pick { flex-direction: row; align-items: center; gap: 10px; }

  .selbar { position: sticky; top: 8px; z-index: 5; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 6px 8px 6px 14px; border-radius: var(--r-md); background: var(--accent-soft); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 45%, transparent); color: var(--accent-2); }
  .sel-n { margin-right: 6px; font-weight: 650; white-space: nowrap; }
  .selbar .grow { flex: 1; }

  /* ---------- Tableau du matériel (les deux vues) ---------- */
  .inv-tbl { table-layout: auto; }
  .inv-tbl th.c-grip, .inv-tbl td.c-grip { width: 22px; padding-right: 4px; }
  .inv-tbl .c-u { width: 44px; }
  .inv-tbl .c-qty { width: 1%; }
  .inv-tbl .c-st { width: 1%; white-space: nowrap; }
  .inv-tbl td.acts { width: 1%; padding-right: 0; }
  .inv-tbl th.acts { padding-right: 0; }
  .inv-tbl input[type='checkbox'] { display: block; }
  .tbl tr.grp th { padding: 14px 8px 6px 0; }
  .grp .eyebrow { display: inline; }
  .grp-n { margin-left: 8px; font-size: var(--fs-xs); font-weight: 600; color: var(--fg-3); }
  .row[draggable='true'] .grip { cursor: grab; }
  .row[draggable='true']:active .grip { cursor: grabbing; }
  .grip { display: grid; place-items: center; width: 18px; height: 24px; color: var(--fg-3); }
  .row.drag-src, .sub.drag-src { opacity: .4; }
  .caret :global(.ic) { transition: transform .15s; }
  .caret[aria-expanded='true'] :global(.ic) { transform: rotate(90deg); }
  .nm-txt { display: flex; flex-wrap: wrap; align-items: center; gap: 2px 8px; min-width: 0; }
  .nm { font-weight: 650; overflow-wrap: anywhere; }
  .br { font-size: var(--fs-sm); color: var(--fg-3); }
  .br:empty { display: none; }
  .tag { display: inline-flex; align-items: center; height: 22px; padding: 0 6px; border-radius: 6px; background: var(--surface); box-shadow: inset 0 0 0 1px var(--line-2); font-size: var(--fs-2xs); font-weight: 600; color: var(--fg); white-space: nowrap; }
  /* Sous-lignes d'exemplaire : compactes, alignées sous la colonne Matériel, toujours affichées. */
  .tbl.dense tr.row.has-sub td { border-bottom: 0; }
  .tbl.dense tr.sub td { height: 32px; padding-top: 1px; padding-bottom: 1px; border-bottom: 0; }
  .tbl.dense tr.sub.last td { height: 36px; padding-bottom: 5px; border-bottom: 1px solid var(--line); }
  .tbl.dense tr.sub .chip { font-weight: 600; }
  .sub-line { display: flex; align-items: center; gap: 8px; min-width: 0; padding-left: 16px; }
  .tag.slot { background: var(--surface-3); box-shadow: none; color: var(--fg-2); }
  .no-id { font-size: var(--fs-xs); color: var(--fg-3); }
  .sub-line .cell { flex: 1 1 160px; max-width: 320px; }
  .mod-name { min-width: 0; font-size: var(--fs-sm); color: var(--fg-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  /* ---------- Nomenclature ---------- */
  .b-tbl tbody.b-group + tbody.b-group tr.b-grp td { border-top: 8px solid var(--bg); }
  .tbl.dense tr.b-grp td { height: 50px; background: var(--surface-2); }
  .gtitle { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 10px; min-width: 0; }
  .g-name { font: 700 var(--fs-lg)/1.25 var(--display); }
  .g-gauge { margin-left: auto; }
  .g-acts { display: inline-flex; gap: 4px; align-items: center; }
  .b-drop td { text-align: center; font-size: var(--fs-sm); color: var(--fg-3); }

  @media (max-width: 900px) {
    .a-grid { grid-template-columns: minmax(0, 1fr); }
    .a-side { position: static; padding: 6px; }
    .side-head { padding: 4px 4px 6px 8px; }
    .side-list { flex-direction: row; overflow-x: auto; scrollbar-width: thin; padding-bottom: 4px; }
    .side-list .el { flex: none; width: 220px; }
    .el.child { padding-left: 8px; }
    .el-add { flex: none; align-self: center; margin: 0 4px; }
    .el-acts { margin-left: 0; }
  }
  /* Téléphone : la ligne de matériel garde nom, quantité et menu ; la pose passe dessous ; une
     sous-ligne d'exemplaire tient sur une rangée (ID, libellé, pose, action). */
  @media (max-width: 640px) {
    .inv-tbl, .inv-tbl tbody { display: block; }
    .inv-tbl thead, .inv-tbl .c-grip, .inv-tbl .c-u { display: none; }
    .inv-tbl tr.row { display: grid; grid-template-columns: 28px minmax(0, 1fr) auto 34px; grid-template-areas: 'ck name qty acts' '. st st st'; gap: 4px 8px; align-items: center; padding: 8px 0; border-bottom: 1px solid var(--line); }
    .inv-tbl tr.row td.c-st:empty { display: none; }
    .inv-tbl tr.row.has-sub { border-bottom: 0; padding-bottom: 4px; }
    .inv-tbl:not(:has(td.ck)) tr.row { grid-template-columns: 0 minmax(0, 1fr) auto 34px; }
    .inv-tbl tr.row td { height: auto; padding: 0; border: 0; }
    .inv-tbl tr.row td.ck { grid-area: ck; }
    .inv-tbl tr.row td.c-name { grid-area: name; }
    .inv-tbl tr.row td.acts { grid-area: acts; }
    .inv-tbl tr.row td.c-st { grid-area: st; width: auto; }
    .inv-tbl tr.row td.c-qty { grid-area: qty; width: auto; justify-self: end; }
    .inv-tbl tr.sub { display: grid; grid-template-columns: 28px minmax(0, 1fr) auto 34px; grid-template-areas: '. main st acts'; gap: 2px 6px; align-items: center; padding: 1px 0; }
    .inv-tbl:not(:has(td.ck)) tr.sub { grid-template-columns: 0 minmax(0, 1fr) auto 34px; }
    .sub-line { padding-left: 0; gap: 6px; }
    .sub-line .cell { flex-basis: 60px; }
    .inv-tbl tr.sub.last { padding-bottom: 8px; border-bottom: 1px solid var(--line); }
    .tbl.dense.inv-tbl tr.sub td { height: auto; padding: 0; border: 0; }
    .inv-tbl tr.sub td.ck, .inv-tbl tr.sub td.c-grip { display: none; }
    .inv-tbl tr.sub td.sub-main { grid-area: main; }
    .inv-tbl tr.sub td.c-st { grid-area: st; width: auto; }
    .inv-tbl tr.sub td.acts { grid-area: acts; }
    .inv-tbl tr.grp, .inv-tbl tr.grp th { display: block; }
    .inv-tbl tr.b-grp { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 4px; border-radius: var(--r-sm); background: var(--surface-2); }
    .tbl.dense.inv-tbl tr.b-grp td { background: none; border-top: 0; }
    .inv-tbl tbody.b-group + tbody.b-group tr.b-grp { margin-top: 10px; }
    .inv-tbl tr.b-grp td.acts { width: auto; padding-right: 4px; }
    .tbl.dense.inv-tbl tr.b-grp td { display: block; height: auto; border-bottom: 0; }
    .inv-tbl tr.b-grp td.b-g-main { flex: 1; min-width: 0; padding: 8px 0 8px 8px; }
    .inv-tbl tr.b-drop, .inv-tbl tr.b-drop td { display: block; }
    .g-gauge { margin-left: 0; }
    .toolbar .search { max-width: none; flex-basis: 100%; }
    .toolbar .grow { display: none; }
    .b-loc { flex-basis: 100%; justify-content: space-between; }
    .stats { width: 100%; }
  }
</style>
