<script lang="ts">
  // Inventaire (section 8) : emplacements, éléments, articles, exemplaires.
  import Icon from '../../components/Icon.svelte';
  import Vu from '../../components/Vu.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import HeadActions from '../../components/HeadActions.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Loading from '../../components/Loading.svelte';
  import Menu from '../../components/Menu.svelte';
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
  import { nf, pl } from '../../lib/format';
  import { route } from '../../lib/router.svelte';
  import { tick, untrack } from 'svelte';
  import { CONTAINER_KINDS, type Article, type ElementDetails, type Container, type Instance, type Location, type ProjectView } from '../../lib/projects';

  let { view = $bindable(), reload }: { view: ProjectView; reload: () => Promise<void> } = $props();
  const writable = $derived(can('projets', 'ecriture'));

  let selLoc = $state<string | null>(null);
  let panelOf = $state<Article | null>(null);
  let selCont = $state<string | null>(null); // id d’élément, ou « free » pour les équipements libres
  $effect(() => {
    if (!view.locations.some((l) => l.id === selLoc)) selLoc = view.locations[0]?.id ?? null;
    const conts = view.containers.filter((c) => c.location_id === selLoc);
    if (selCont !== 'free' && !conts.some((c) => c.id === selCont)) selCont = conts[0]?.id ?? (selLoc ? 'free' : null);
  });
  // État de l'écran dans l'adresse : ?emplacement=<id>&element=<id|libre>, partageable et
  // rechargeable. ?i=<exemplaire> (palette de recherche) ouvre son emplacement et son élément,
  // fait défiler jusqu'à sa ligne et la met en évidence.
  let flash = $state<string | null>(null);
  let flashT: ReturnType<typeof setTimeout> | undefined;
  function applyQuery(q: URLSearchParams) {
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
    if (inst) {
      flash = inst.id;
      clearTimeout(flashT); flashT = setTimeout(() => (flash = null), 6000);
      tick().then(() => requestAnimationFrame(() => document.querySelector(`[data-inst="${CSS.escape(inst.id)}"]`)?.scrollIntoView({ block: 'center', behavior: 'smooth' })));
    }
  }
  let seenQ = untrack(() => route.query);
  untrack(() => applyQuery(seenQ));
  $effect(() => { const q = route.query; if (q === seenQ) return; seenQ = q; untrack(() => applyQuery(q)); });
  // L'adresse suit l'emplacement et l'élément choisis, sans entrée d'historique ni retour en
  // haut de page (navigate() ramènerait la vue en haut à chaque clic sur une carte).
  $effect(() => {
    if (!selLoc) return;
    const p = new URLSearchParams({ emplacement: selLoc, ...(selCont ? { element: selCont === 'free' ? 'libre' : selCont } : {}) });
    if (location.search === `?${p}`) return;
    history.replaceState(history.state, '', `${location.pathname}?${p}`);
    untrack(() => { seenQ = p; route.query = p; });
  });
  const loc = $derived(view.locations.find((l) => l.id === selLoc));
  // Un emplacement ne se supprime que vide (ni élément ni matériel) : le bouton le dit avant.
  const busyLoc = $derived(!!loc && (loc.totals.containers > 0 || view.articles.some((a) => a.location_id === loc.id)));
  const conts = $derived(view.containers.filter((c) => c.location_id === selLoc));
  const cont = $derived(view.containers.find((c) => c.id === selCont) ?? null);
  // Les modules glissés dans un châssis s'affichent sous leur châssis, pas comme des lignes à part.
  const arts = $derived(view.articles.filter((a) => !a.host && (selCont === 'free' ? a.location_id === selLoc && !a.container_id : a.container_id === selCont)));
  const modulesOf = (a: Article) => view.articles.filter((m) => m.host?.article_id === a.id).sort((x, y) => x.host!.slot - y.host!.slot);
  let chassisOf = $state<Article | null>(null);

  // Statut de pose : pour le rack, flight-case ou casquette ouvert, où chaque exemplaire
  // (ou accessoire) est posé dans l'élévation, d'après l'éditeur de racks. Les jauges Hauteur
  // des cartes lisent les totaux du serveur (placed_u, pending_u) sans relire chaque élévation.
  interface Pl { article_id: string; instance_id: string | null; u_start: number; face: string; shelf_id: string | null }
  type RackEl = { placement: Pl; height_u: number; accessory?: { kind: string } | null };
  type RackView = { elements: RackEl[]; pending?: { height_u: number | null; rackable: boolean }[] };
  let racks = $state<Record<string, RackView>>({});
  $effect(() => {
    void view.articles; void view.instances;
    const c = cont;
    if (c?.rackable) api.get<RackView>(`/containers/${c.id}/rack`).then((r) => { racks[c.id] = r; }, () => {});
  });
  const rackEls = $derived(cont && racks[cont.id] ? racks[cont.id]!.elements : null);
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
  /** Statut d'un exemplaire, ou d'un article sans exemplaire (accessoire, panel, adaptateur). */
  function statusOf(a: Article, instId: string | null): { kind: 'ok' | 'todo' | 'info' | 'load'; text: string } {
    if (a.host) return { kind: 'info', text: `Dans le châssis · S${a.host.slot}` };
    if (!cont) return { kind: 'info', text: 'Libre dans l’emplacement' };
    if (!cont.rackable) return { kind: 'info', text: `Dans ${CONTAINER_KINDS[cont.kind].label.toLowerCase()}` };
    const rackable = a.accessory || item(a)?.rackable;
    if (!rackable) return { kind: 'info', text: 'Hors rack' };
    if (!rackEls) return { kind: 'load', text: 'Lecture de la pose' };
    const e = rackEls.find((x) => x.placement.article_id === a.id && (instId ? x.placement.instance_id === instId : !x.placement.instance_id));
    return e ? { kind: 'ok', text: `Posé · ${posText(e)}` } : { kind: 'todo', text: 'À placer' };
  }
  /** Pastille de statut de pose (pastilles communes). */
  const ST: Record<string, string> = { ok: 'chip ok', todo: 'chip warn', info: 'chip' };
  /** Page de conception de l'élément : éditeur de racks ou de mobilier. */
  const designHref = (c: Container) => (c.kind === 'desk' || c.kind === 'furniture' ? `/projets/${view.project.id}/mobilier/${c.id}` : c.kind === 'casquette' && c.parent_id ? `/projets/${view.project.id}/mobilier/${c.parent_id}/${c.id}` : c.rackable ? `/projets/${view.project.id}/racks/${c.id}` : null);
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
  // Barre d'emplacements (défile sur téléphone) : fondu au bord tant qu'il reste à voir.
  let locBar = $state<HTMLElement>();
  let moreR = $state(false), moreL = $state(false);
  function locScroll() {
    const b = locBar; if (!b) return;
    moreL = b.scrollLeft > 4;
    moreR = b.scrollLeft + b.clientWidth < b.scrollWidth - 4;
  }
  $effect(() => {
    void view.locations.length;
    const b = locBar; if (!b) return;
    locScroll();
    const ro = new ResizeObserver(locScroll); ro.observe(b);
    return () => ro.disconnect();
  });
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
  let moving = $state<{ a: Article; target: string; instance: string | null } | null>(null);
  let confirmDel = $state<{ kind: 'container' | 'article' | 'location'; id: string; name: string } | null>(null);

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
  // Matériel rangé par nature ; les accessoires et panels identiques (une pièce par article,
  // chacune posée à sa place) sont regroupés sur une ligne avec un compteur.
  type Row = { key: string; a: Article; list: Article[] };
  const SECTIONS: [string, string][] = [['equipment', 'Équipements'], ['panel', 'Panels de brassage'], ['accessory', 'Accessoires'], ['adapter', 'Adaptateurs']];
  const sectionOf = (a: Article) => (a.nature === 'accessory' ? (a.accessory?.kind === 'panel' ? 'panel' : 'accessory') : a.nature);
  const grouped = $derived.by(() => {
    const rows = new Map<string, Row>();
    for (const a of arts) {
      // Accessoires identiques regroupés ; un châssis garde sa ligne (ses modules lui sont propres).
      const unit = a.nature === 'accessory';
      const key = unit ? `${sectionOf(a)}|${a.item_id ?? ''}|${JSON.stringify(a.accessory ?? null)}` : a.id;
      const r = rows.get(key);
      if (r) r.list.push(a); else rows.set(key, { key, a, list: [a] });
    }
    return SECTIONS.map(([k, label]) => ({ k, label, rows: [...rows.values()].filter((r) => sectionOf(r.a) === k) })).filter((g) => g.rows.length);
  });
  const qtyOf = (r: Row) => r.list.reduce((n, a) => n + a.quantity, 0);
  /** + / − sur une ligne : quantité de l'article, ou une pièce de plus ou de moins. */
  async function step(r: Row, d: 1 | -1) {
    const unit = r.list.length > 1 || r.a.nature === 'accessory' || item(r.a)?.modular;
    if (!unit) return setQty(r.a, r.a.quantity + d);
    try {
      if (d > 0) {
        const a = r.list[r.list.length - 1]!;
        await api.post(`/projects/${view.project.id}/articles`, { container_id: a.container_id, location_id: a.container_id ? null : a.location_id, nature: a.nature, item_id: a.item_id, accessory: a.accessory, quantity: 1 });
      } else {
        const a = r.list[r.list.length - 1]!;
        if (r.list.length === 1) { confirmDel = { kind: 'article', id: a.id, name: a.label }; return; }
        await api.del(`/articles/${a.id}`);
        toast(`Un exemplaire de « ${a.label} » retiré.`, 'ok', { run: async () => { try { await api.post(`/projects/${view.project.id}/articles`, { container_id: a.container_id, location_id: a.container_id ? null : a.location_id, nature: a.nature, item_id: a.item_id, accessory: a.accessory, quantity: 1 }); await reload(); } catch (e) { fail(e); } } });
      }
      await reload();
    } catch (e) { fail(e); await reload(); }
  }
  async function setQty(a: Article, q: number) {
    if (q < 1) { confirmDel = { kind: 'article', id: a.id, name: a.label }; return; }
    try { await api.patch(`/articles/${a.id}`, { rev: a.rev, quantity: q }); await reload(); } catch (e) { fail(e); await reload(); }
  }
  // Confirmation et déplacement : la boîte suit l'envoi ; en cas d'échec, l'erreur s'affiche et elle reste ouverte.
  async function del() {
    const d = confirmDel!;
    await api.del(d.kind === 'container' ? `/containers/${d.id}` : d.kind === 'article' ? `/articles/${d.id}` : `/locations/${d.id}`);
    toast(d.kind === 'location' ? `Emplacement « ${d.name} » supprimé.` : d.kind === 'container' ? `Élément « ${d.name} » supprimé, avec son contenu.` : `« ${d.name} » retiré de l’inventaire.`); confirmDel = null; await reload();
  }
  async function doMove() {
    const m = moving!;
    const [kind, target] = m.target.split(':');
    await api.post(`/articles/${m.a.id}/move`, { rev: m.a.rev, container_id: kind === 'c' ? target : null, location_id: kind === 'l' ? target : null, instance_id: m.instance });
    toast(m.a.quantity > 1 && m.a.nature === 'equipment' ? 'Un exemplaire a été déplacé.' : 'Matériel déplacé.'); moving = null; await reload();
  }
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
</script>

{#snippet newLocAction()}<button class="btn primary" onclick={newLoc}><Icon name="plus" size="sm" />Nouvel emplacement</button>{/snippet}
{#snippet menuTrigger()}<Icon name="copy" size="sm" /><span class="d-only">Modèles</span><span class="m-only">Actions</span><Icon name="chevd" size="sm" />{/snippet}

<div class="inv">
  {#if !view.locations.length}
    <EmptyState icon="pin" message="Aucun emplacement" hint={writable ? 'Commencez par en créer un : la régie, le bord du terrain, la cabine.' : 'Aucun emplacement n’a encore été créé dans ce projet.'} action={writable ? newLocAction : undefined} />
  {:else}
    <div class="card loc-list" bind:this={locBar} onscroll={locScroll} class:more-r={moreR} class:more-l={moreL}>
      {#each view.locations as l (l.id)}
        <button class="loc" class:on={l.id === selLoc} aria-pressed={l.id === selLoc} onclick={() => { selLoc = l.id; selCont = null; }}>
          <span class="code mono">{l.code || '—'}</span><span class="ln"><b>{l.name}</b><span class="small muted">{l.totals.containers} élément{l.totals.containers > 1 ? 's' : ''}{l.totals.free_items ? ` · ${l.totals.free_items} libre${l.totals.free_items > 1 ? 's' : ''}` : ''}</span></span>
        </button>
      {/each}
      {#if writable}<button class="btn ghost" onclick={newLoc}><Icon name="plus" size="sm" />Nouvel emplacement</button>{/if}
      <span class="grow d-only"></span>
      <button class="btn ghost d-only" title="Fichier Excel de tout le matériel du projet, élément par élément : liste de colisage, chiffrage, logistique" onclick={() => download(`/projects/${view.project.id}/inventory/export?format=xlsx&group=container`)}><Icon name="download" size="sm" />Exporter la nomenclature (Excel)</button>
    </div>
  {/if}

  {#if loc}
    {#if writable}<HeadActions><button class="btn" class:primary={!conts.length} onclick={() => (contEdit = { c: null })}><Icon name="plus" size="sm" />Nouvel élément</button></HeadActions>{/if}
    <div class="inv-main">
      <div class="page-head loc-head">
        <div><p class="eyebrow">Emplacement · code câble {loc.code}</p><h2 class="loc-title">{loc.name}</h2><p class="muted small">{nf(loc.totals.weight_kg, 1)} kg · {nf(loc.totals.power_w)} W</p></div>
        <div class="head-actions">
          {#if writable}
            <span class="d-only" title={busyLoc ? `Impossible de supprimer « ${loc.name} » : il contient encore ${loc.totals.containers ? `${loc.totals.containers} élément${loc.totals.containers > 1 ? 's' : ''}` : 'du matériel'}. Déplacez ou supprimez son contenu d’abord.` : 'Supprimer l’emplacement'}><button class="icon-btn danger" disabled={busyLoc} aria-label="Supprimer l’emplacement" onclick={() => (confirmDel = { kind: 'location', id: loc.id, name: loc.name })}><Icon name="trash" /></button></span>
            <button class="btn ghost d-only" onclick={() => editLoc(loc)}><Icon name="pen" size="sm" />Renommer</button>
          {/if}
          <!-- Sur téléphone, le menu « Actions » reprend aussi Renommer, Supprimer et l'export. -->
          <div class:m-only={!writable}>
            <Menu label="Actions de l’emplacement" trigger={menuTrigger}>
              {#if writable}
                <button role="menuitem" class="m-only" onclick={() => editLoc(loc)}><Icon name="pen" size="sm" /><span><b>Renommer l’emplacement</b></span></button>
                <button role="menuitem" class="m-only danger" disabled={busyLoc} onclick={() => (confirmDel = { kind: 'location', id: loc.id, name: loc.name })}><Icon name="trash" size="sm" /><span><b>Supprimer l’emplacement</b>{#if busyLoc}<small class="pm-sub">Videz-le d’abord</small>{/if}</span></button>
                <hr class="m-only" aria-hidden="true">
                <button role="menuitem" onclick={() => (fromTemplate = true)}><Icon name="case" size="sm" /><span><b>Ajouter un élément depuis un modèle</b><small class="pm-sub">Un rack, un flight-case, avec son contenu</small></span></button>
                <button role="menuitem" onclick={openRegiePick}><Icon name="layers" size="sm" /><span><b>Ajouter une régie depuis un modèle</b><small class="pm-sub">Plusieurs éléments d’un coup, dans cet emplacement</small></span></button>
                {#if can('catalogue', 'ecriture')}<button role="menuitem" onclick={saveLocAsRegie}><Icon name="download" size="sm" /><span><b>Enregistrer cet emplacement comme modèle de régie</b><small class="pm-sub">Tous ses éléments et leur matériel</small></span></button>{/if}
                <hr class="m-only" aria-hidden="true">
              {/if}
              <button role="menuitem" class="m-only" onclick={() => download(`/projects/${view.project.id}/inventory/export?format=xlsx&group=container`)}><Icon name="download" size="sm" /><span><b>Exporter la nomenclature (Excel)</b><small class="pm-sub">Tout le matériel du projet, élément par élément</small></span></button>
            </Menu>
          </div>
        </div>
      </div>

      <div class="cont-grid">
        {#each conts as c (c.id)}
          {@const t = c.totals}
          <button class="cont" class:on={c.id === selCont} aria-pressed={c.id === selCont} onclick={() => (selCont = c.id)}>
            <span class="cont-vis">
              {#if c.rackable}<span class="mini-rack" class:case={c.kind !== 'rack'}>{#each blocks(c) as b}<i style="height:{b.h}%;--c:{b.color}"></i>{/each}</span>{:else}<span class="kind-vis"><KindIll kind={c.kind} size={78} /></span>{/if}
            </span>
            <span class="cont-info">
              <span class="cont-top"><span class="chip">{CONTAINER_KINDS[c.kind].label}</span>{#if c.wheels}<span class="chip">Roulettes</span>{/if}</span>
              <b class="cont-name">{c.name}</b>
              <span class="mono small muted">{c.cable_code} · {specLine(c)}</span>
              <span class="meters">
                {#if c.rackable}
                  <!-- Jauge = U posés dans l'élévation ; le matériel affecté mais pas encore posé est
                       annoncé à part, et un dépassement de la hauteur colore toute la jauge. -->
                  {@const use = rackUse(c)}
                  {@const H = c.height_u ?? 0}
                  {@const over = H > 0 && t.used_u > H}
                  <span class="meter" class:err={over}>
                    <span>Hauteur posée</span>
                    <span class="mono">{use ? fu(use.placed) : '—'} / {H} U</span>
                    {#if over}<Vu value={100} mode="prog" cls="err" n={24} />{:else}<Vu value={use ? pct(use.placed, H) : 0} mode="load" n={24} />{/if}
                    {#if over}<span class="meter-note over">Dépasse de {fu(t.used_u - H)} U : {fu(t.used_u)} U affectés pour {H} U</span>
                    {:else if use?.todo}<span class="meter-note">+ {fu(use.todo)} U à placer</span>{/if}
                  </span>
                {/if}
                <span class="meter" class:err={t.over_weight}><span>Poids</span><span class="mono">{nf(t.weight_kg, 1)}{c.max_weight_kg ? ` / ${nf(c.max_weight_kg)}` : ''} kg</span>{#if c.max_weight_kg}<Vu value={pct(t.weight_kg, c.max_weight_kg)} mode="load" n={24} />{/if}</span>
                <span class="meter" class:err={t.over_power}><span>Conso</span><span class="mono">{nf(t.power_w)}{c.max_power_w ? ` / ${nf(c.max_power_w)}` : ''} W</span>{#if c.max_power_w}<Vu value={pct(t.power_w, c.max_power_w)} mode="load" n={24} />{/if}</span>
              </span>
            </span>
          </button>
        {/each}
        <button class="cont free" class:on={selCont === 'free'} aria-pressed={selCont === 'free'} onclick={() => (selCont = 'free')}>
          <span class="cont-info"><span class="cont-top"><span class="chip">Sans élément</span></span><b class="cont-name">Équipements libres</b><span class="small muted">{loc.totals.free_items} pièce{loc.totals.free_items > 1 ? 's' : ''}, comptées dans la nomenclature et les totaux.</span></span>
        </button>
      </div>

      <section class="card">
        <div class="card-head">
          <div><h2>{cont ? cont.name : 'Équipements libres'}</h2><span class="small muted">{arts.reduce((s, a) => s + a.quantity, 0)} pièce{arts.reduce((s, a) => s + a.quantity, 0) > 1 ? 's' : ''}{cont ? ` · code câble ${cont.cable_code}` : ''}{cont?.totals.volume_m3 ? ` · ${nf(cont.totals.volume_m3, 2)} m³` : ''}</span></div>
          <div class="head-actions el-acts">
            {#if writable && cont}<button class="icon-btn danger act-danger el-del" title="Supprimer l’élément" aria-label="Supprimer l’élément" onclick={() => (confirmDel = { kind: 'container', id: cont.id, name: cont.name })}><Icon name="trash" /></button>{/if}
            {#if writable && cont && can('catalogue', 'ecriture')}<button class="btn ghost" onclick={() => saveAsTemplate(cont)} title="Copie l’élément et son contenu dans la bibliothèque"><Icon name="copy" size="sm" />Enregistrer comme modèle</button>{/if}
            {#if writable && cont}<button class="btn ghost" onclick={() => (contEdit = { c: cont })}><Icon name="pen" size="sm" />Modifier</button>{/if}
            {#if cont && designHref(cont)}<a class="btn" href={designHref(cont)}><Icon name={cont.rackable ? 'server' : 'layers'} size="sm" />Conception</a>{/if}
            {#if writable}<button class="btn primary" onclick={() => (adding = true)}><Icon name="plus" size="sm" />Ajouter du matériel</button>{/if}
          </div>
        </div>
        {#if cont?.kind === 'monitor_wall' && cont.details?.wall}
          <WallView wall={cont.details.wall} screens={gearOf(cont)} {writable} onsave={(w) => saveDetails(cont, { ...cont.details, wall: w })} />
        {:else if (cont?.kind === 'desk' || cont?.kind === 'operator_position') && cont.details?.positions?.length}
          <DeskView positions={cont.details.positions} gear={gearOf(cont)} {writable} onsave={(ps) => saveDetails(cont, { ...cont.details, positions: ps })} />
        {/if}
        <div class="tbl-wrap"><table class="tbl inv-tbl">
          <!-- Les lignes d'article couvrent tout le tableau : l'en-tête n'est lu que par les lecteurs d'écran. -->
          <thead class="sr-only"><tr><th class="c-id">ID</th><th>Libellé</th><th>Statut</th><th><span class="sr-only">Actions</span></th></tr></thead>
          <tbody>
            {#each grouped as g (g.k)}
            <tr class="grp"><th colspan="4" scope="colgroup">{g.label}<span class="grp-n mono">{g.rows.reduce((n, r) => n + qtyOf(r), 0)}</span></th></tr>
            {#each g.rows as r (r.key)}
              {@const a = r.a}
              {@const it = item(a)}
              {@const inst = instOf(a)}
              <tr class="art-row">
                <td colspan="4"><div class="art-line">
                  <span class="art-name"><b>{a.label}</b><span class="small muted">{it?.modular ? `Châssis · ${modulesOf(a).length} module${modulesOf(a).length > 1 ? 's' : ''}` : a.nature === 'equipment' ? 'Équipement' : a.nature === 'adapter' ? 'Adaptateur' : a.accessory?.kind === 'panel' ? `Panel · ${a.accessory.panel.slots.length} connecteur${a.accessory.panel.slots.length > 1 ? 's' : ''}` : 'Accessoire'}{it?.brand ? ` · ${it.brand}` : ''}{heightOf(a) ? ` · ${nf(heightOf(a)!, 1).replace(',0', '')} U` : ''}{it?.weight_kg ? ` · ${nf(it.weight_kg * qtyOf(r), 1)} kg` : ''}{it?.power_w ? ` · ${nf(it.power_w * qtyOf(r))} W` : ''}{it && !it.active ? ' · fiche inactive' : ''}</span></span>
                  {#if !inst.length}{@const st = statusOf(a, null)}{#if st.kind === 'load'}<Loading inline label={st.text} />{:else}<span class={ST[st.kind]}>{r.list.length > 1 ? `${r.list.filter((x) => statusOf(x, null).kind === 'ok').length} / ${r.list.length} posés` : st.text}</span>{/if}{/if}
                  <span class="art-tools">
                    {#if writable}<span class="stepper" title="Quantité"><button onclick={() => step(r, -1)} aria-label="Retirer un">−</button><span>{qtyOf(r)}</span><button onclick={() => step(r, 1)} aria-label="Ajouter un">+</button></span>{:else}<span class="mono">× {qtyOf(r)}</span>{/if}
                    {#if a.accessory?.kind === 'panel'}<button class="btn sm ghost" onclick={() => (panelOf = a)}><Icon name="grid" size="sm" />{writable ? 'Modifier' : 'Voir'}</button>{/if}
                    {#if it?.modular}<button class="btn sm" onclick={() => (chassisOf = a)}><Icon name="grid" size="sm" />Modules</button>{/if}
                    {#if writable}<button class="icon-btn sm" title="Déplacer" aria-label="Déplacer" onclick={() => (moving = { a, target: '', instance: inst.at(-1)?.id ?? null })}><Icon name="arrowr" size="sm" /></button><button class="icon-btn sm danger" title="Supprimer" aria-label="Supprimer" onclick={() => (confirmDel = { kind: 'article', id: a.id, name: a.label })}><Icon name="trash" size="sm" /></button>{/if}
                  </span>
                </div></td>
              </tr>
              {#each inst as i (i.id)}
                {@const st = statusOf(a, i.id)}
                <tr class="inst-row" class:focus={flash === i.id} data-inst={i.id}>
                  <td class="c-id"><span class="mono id">{i.tag}</span></td>
                  <td><input class="input cell" value={i.label ?? ''} disabled={!writable} placeholder="Libellé libre" maxlength="80" onchange={(e) => saveInstance(i, 'label', (e.target as HTMLInputElement).value)} aria-label="Libellé de {i.tag}"></td>
                  <td>{#if st.kind === 'load'}<Loading inline label={st.text} />{:else}<span class={ST[st.kind]}>{st.text}</span>{/if}</td>
                  <td class="acts"></td>
                </tr>
              {/each}
              {#if it?.modular}
                {#each modulesOf(a) as mo (mo.id)}
                  {@const mi = item(mo)}
                  {#each view.instances.filter((x) => x.article_id === mo.id) as i (i.id)}
                    <tr class="inst-row mod" class:focus={flash === i.id} data-inst={i.id}>
                      <td class="c-id"><span class="mono id">{i.tag}</span></td>
                      <td><span class="mod-name"><span class="slotn mono">S{mo.host!.slot}</span>{mo.label}{mi?.brand ? ` · ${mi.brand}` : ''}</span></td>
                      <td><span class="chip">Dans le châssis · S{mo.host!.slot}</span></td>
                      <td class="acts">{#if writable}<button class="icon-btn sm danger" title="Retirer du châssis" aria-label="Retirer du châssis" onclick={() => (confirmDel = { kind: 'article', id: mo.id, name: mo.label })}><Icon name="x" size="sm" /></button>{/if}</td>
                    </tr>
                  {/each}
                {/each}
              {/if}
            {/each}
            {:else}<tr><td colspan="4"><EmptyState card={false} icon="box" message="Aucun matériel ici" hint={writable ? 'Ajoutez-en avec « Ajouter du matériel ».' : ''} /></td></tr>{/each}
          </tbody>
        </table></div>
      </section>
    </div>
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
  <Dialog title="Déplacer « {moving.a.label} »" eyebrow={view.project.name} onclose={() => (moving = null)} onsubmit={doMove} submitLabel="Déplacer" submitDisabled={!moving.target}>
    {#if moving.a.nature === 'equipment' && moving.a.quantity > 1}
      <div class="field"><label for="mv-i">Exemplaire à déplacer</label><select class="input" id="mv-i" bind:value={moving.instance} aria-describedby="mv-i-help">{#each instOf(moving.a) as i}<option value={i.id}>{i.tag}{i.serial ? ` · ${i.serial}` : ''}</option>{/each}</select><span class="help" id="mv-i-help">Ce matériel compte {moving.a.quantity} exemplaires : un seul part, avec son ID.</span></div>
    {/if}
    <div class="field"><label for="mv-t">Vers</label>
      <select class="input" id="mv-t" bind:value={moving.target}>
        <option value="" disabled>Choisir une destination</option>
        {#each view.locations as l}
          <optgroup label={l.name}>
            {#each view.containers.filter((c) => c.location_id === l.id && c.id !== moving!.a.container_id) as c}<option value="c:{c.id}" disabled={moving.a.nature === 'accessory' && !c.rackable}>{c.name}</option>{/each}
            {#if moving.a.nature !== 'accessory' && !(moving.a.container_id === null && moving.a.location_id === l.id)}<option value="l:{l.id}">Équipements libres de {l.name}</option>{/if}
          </optgroup>
        {/each}
      </select>
    </div>
  </Dialog>
{/if}
{#if confirmDel}
  <Confirm title="Supprimer « {confirmDel.name} » ?" eyebrow={confirmDel.kind === 'container' ? 'Élément' : confirmDel.kind === 'location' ? 'Emplacement' : 'Matériel'} confirmLabel="Supprimer" danger onconfirm={del} onclose={() => (confirmDel = null)}>
    <p>{confirmDel.kind === 'container' ? 'Son matériel et ses exemplaires partent avec lui.' : confirmDel.kind === 'location' ? 'L’emplacement doit être vide : sans élément ni équipement libre.' : 'Ses exemplaires et leurs ID partent avec lui.'}</p>
  </Confirm>
{/if}

{#if chassisOf}<ChassisDialog {view} host={chassisOf} {writable} onclose={() => (chassisOf = null)} ondone={reload} />{/if}
{#if panelOf}<ProjectPanel article={panelOf} readonly={!writable} eyebrow="Panel du projet" onclose={() => (panelOf = null)} ondone={reload} />{/if}

<style>
  /* Onglet d’emplacement actif : le texte secondaire suit la couleur du bouton (contraste lisible). */
  .loc-list .loc.on .ln b, .loc-list .loc.on .ln .small { color: inherit; }
  .loc-list .loc.on .ln .small { opacity: .8; }
  .loc-list .grow { flex: 1; }
  .loc-head { align-items: center; }
  .loc-title { margin: 0; font: 700 var(--fs-xl)/1.2 var(--display); letter-spacing: -.015em; }
  [role='menuitem'] > span { display: flex; flex-direction: column; gap: 1px; }
  .m-only { display: none; }
  @media (max-width: 640px) {
    .d-only { display: none; }
    .m-only { display: revert; }
    [role='menuitem'].m-only { display: flex; }
    .loc-list.more-r { -webkit-mask-image: linear-gradient(90deg, #000 calc(100% - 48px), transparent); mask-image: linear-gradient(90deg, #000 calc(100% - 48px), transparent); }
    .loc-list.more-l.more-r { -webkit-mask-image: linear-gradient(90deg, transparent, #000 32px, #000 calc(100% - 48px), transparent); mask-image: linear-gradient(90deg, transparent, #000 32px, #000 calc(100% - 48px), transparent); }
    .loc-list.more-l:not(.more-r) { -webkit-mask-image: linear-gradient(90deg, transparent, #000 32px); mask-image: linear-gradient(90deg, transparent, #000 32px); }
  }
  /* Jauge Hauteur : matériel affecté mais pas posé, ou dépassement de la hauteur. */
  .meter-note { grid-column: 1 / -1; font-size: var(--fs-xs); color: var(--fg-3); }
  .meter-note.over { color: var(--err); font-weight: 600; }
  .cont.free { grid-template-columns: 1fr; }
  /* Actions de l’élément : la suppression à gauche, à l’écart des autres. */
  .el-acts { flex: 1; justify-content: flex-end; align-items: center; }
  .el-del { margin-right: auto; }
  .slotn { display: inline-block; margin-right: 8px; padding: 1px 6px; border-radius: 5px; background: var(--surface-2); box-shadow: inset 0 0 0 1px var(--line); font-size: var(--fs-2xs); color: var(--fg-2); }
  .grp-n { margin-left: 8px; font-size: var(--fs-sm); font-weight: 600; color: var(--fg-3); }
  .inv-tbl { table-layout: auto; }
  .c-id { width: 96px; }
  .art-row td { padding: 14px 8px 6px 0; border-bottom: 0; border-top: 1px solid var(--line); }
  .grp + .art-row td { border-top: 0; }
  .art-line { display: flex; align-items: center; gap: 14px; flex-wrap: wrap; }
  .art-name { flex: 1; min-width: 240px; display: flex; flex-direction: column; align-items: flex-start; gap: 2px; }
  .art-name b { font-size: 15px; }
  .art-tools { display: inline-flex; align-items: center; gap: 4px; }
  .inst-row td { padding: 3px 8px 3px 0; border-bottom: 0; }
  .inst-row:last-of-type td { padding-bottom: 8px; }
  .inst-row .id { display: inline-block; padding: 3px 7px; border-radius: 6px; background: var(--surface-2); box-shadow: inset 0 0 0 1px var(--line); font-size: var(--fs-xs); font-weight: 600; }
  .mod-name { font-size: var(--fs-md); }
  .kind-vis { display: grid; place-items: center; width: 84px; height: 148px; --ki: var(--sig-v); }
</style>
