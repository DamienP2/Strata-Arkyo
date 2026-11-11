<script lang="ts">
  // Inventaire (section 8) : emplacements, éléments, articles, exemplaires.
  import Icon from '../../components/Icon.svelte';
  import Vu from '../../components/Vu.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import ContainerDialog from './ContainerDialog.svelte';
  import AddArticle from './AddArticle.svelte';
  import TemplatePick from './TemplatePick.svelte';
  import ProjectPanel from './ProjectPanel.svelte';
  import WallView from './WallView.svelte';
  import DeskView from './DeskView.svelte';
  import KindIll from '../../components/KindIll.svelte';
  import { api, ApiError, download } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { nf } from '../../lib/format';
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
  const loc = $derived(view.locations.find((l) => l.id === selLoc));
  const conts = $derived(view.containers.filter((c) => c.location_id === selLoc));
  const cont = $derived(view.containers.find((c) => c.id === selCont) ?? null);
  const arts = $derived(view.articles.filter((a) => (selCont === 'free' ? a.location_id === selLoc && !a.container_id : a.container_id === selCont)));
  const instOf = (a: Article) => view.instances.filter((i) => i.article_id === a.id);
  const item = (a: Article) => (a.item_id ? view.items[a.item_id] : undefined);
  const heightOf = (a: Article) => (a.accessory ? (a.accessory.kind === 'panel' ? a.accessory.panel.height_u : a.accessory.height_u) : item(a)?.rackable ? item(a)!.height_u : null);

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
      case 'monitor_wall': return `${d.wall?.rows ?? 0} × ${d.wall?.cols ?? 0} · ${d.wall?.tiles.length ?? 0} tuile(s)`;
      case 'desk': case 'operator_position': return `${d.positions?.length ?? 0} poste(s)${d.positions?.[0]?.role && c.kind === 'operator_position' ? ` · ${d.positions[0].role}` : ''}`;
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
  let moving = $state<{ a: Article; target: string; instance: string | null } | null>(null);
  let confirmDel = $state<{ kind: 'container' | 'article' | 'location'; id: string; name: string } | null>(null);
  let open = $state<Set<string>>(new Set());

  async function saveLoc(e: SubmitEvent) {
    e.preventDefault(); locErr = {};
    const l = locEdit!;
    try {
      const r = l.id ? await api.patch<Location>(`/locations/${l.id}`, { rev: l.rev, name: l.name, code: l.code }) : await api.post<Location>(`/projects/${view.project.id}/locations`, { name: l.name, code: l.code });
      toast(l.id ? 'Emplacement enregistré.' : 'Emplacement ajouté.'); locEdit = null; selLoc = r.id; await reload();
    } catch (x) { if (x instanceof ApiError) locErr = Object.fromEntries(x.fields.map((f) => [f.field, f.message])); if (!(x instanceof ApiError && x.fields.length)) fail(x); }
  }
  async function setQty(a: Article, q: number) {
    if (q < 1) { confirmDel = { kind: 'article', id: a.id, name: a.label }; return; }
    try { await api.patch(`/articles/${a.id}`, { rev: a.rev, quantity: q }); await reload(); } catch (e) { fail(e); await reload(); }
  }
  async function del() {
    const d = confirmDel!;
    try {
      await api.del(d.kind === 'container' ? `/containers/${d.id}` : d.kind === 'article' ? `/articles/${d.id}` : `/locations/${d.id}`);
      toast('Supprimé.'); confirmDel = null; await reload();
    } catch (e) { fail(e); confirmDel = null; }
  }
  async function doMove() {
    const m = moving!;
    const [kind, target] = m.target.split(':');
    try {
      await api.post(`/articles/${m.a.id}/move`, { rev: m.a.rev, container_id: kind === 'c' ? target : null, location_id: kind === 'l' ? target : null, instance_id: m.instance });
      toast(m.a.quantity > 1 && m.a.nature === 'equipment' ? 'Un exemplaire a été déplacé.' : 'Article déplacé.'); moving = null; await reload();
    } catch (e) { fail(e); }
  }
  // Les saisies sur un même exemplaire s'enchaînent : chacune part avec la révision
  // renvoyée par la précédente, sans attendre un rechargement complet.
  const queue = new Map<string, Promise<void>>();
  function saveInstance(i: Instance, field: 'serial' | 'mac' | 'inventory_tag', value: string) {
    const prev = queue.get(i.id) ?? Promise.resolve();
    const next = prev.then(async () => {
      const cur = view.instances.find((x) => x.id === i.id) ?? i;
      if ((cur[field] ?? '') === value.trim()) return;
      try {
        const r = await api.patch<Instance>(`/instances/${i.id}`, { rev: cur.rev, serial: cur.serial, mac: cur.mac, inventory_tag: cur.inventory_tag, [field]: value });
        const k = view.instances.findIndex((x) => x.id === i.id);
        if (k >= 0) view.instances[k] = r;
      } catch (e) { fail(e); await reload(); }
    });
    queue.set(i.id, next);
  }
  const toggle = (id: string) => { const s = new Set(open); s.has(id) ? s.delete(id) : s.add(id); open = s; };
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

<div class="inv">
  <div class="card loc-list" style="align-items:center">
    {#each view.locations as l (l.id)}
      <button class="loc" class:on={l.id === selLoc} onclick={() => { selLoc = l.id; selCont = null; }}>
        <span class="code mono">{l.code || '—'}</span><span class="ln"><b>{l.name}</b><span class="small muted">{l.totals.containers} élément{l.totals.containers > 1 ? 's' : ''}{l.totals.free_items ? ` · ${l.totals.free_items} libre${l.totals.free_items > 1 ? 's' : ''}` : ''}</span></span>
      </button>
    {/each}
    {#if writable}<button class="btn sm" onclick={() => (locEdit = { id: null, rev: 0, name: '', code: '' })}><Icon name="plus" size="sm" />Emplacement</button>{/if}
    <span style="flex:1"></span>
    <button class="btn sm ghost" onclick={() => download(`/projects/${view.project.id}/inventory/export?format=xlsx&group=container`)}><Icon name="download" size="sm" />Nomenclature</button>
  </div>

  {#if !view.locations.length}
    <div class="empty">Aucun emplacement. {#if writable}Commencez par en créer un : la régie, le bord du terrain, la cabine…{/if}</div>
  {:else if loc}
    <div class="inv-main">
      <div class="page-head" style="align-items:center">
        <div><p class="eyebrow">Emplacement · code câble {loc.code}</p><h2 style="font:720 30px/1.1 var(--display);letter-spacing:-.03em">{loc.name}</h2><p class="muted small">{nf(loc.totals.weight_kg, 1)} kg · {nf(loc.totals.power_w)} W</p></div>
        {#if writable}<div class="head-actions">
          <button class="btn sm" onclick={() => (locEdit = { id: loc.id, rev: loc.rev, name: loc.name, code: loc.code })}><Icon name="pen" size="sm" />Renommer</button>
          <button class="btn sm ghost danger" onclick={() => (confirmDel = { kind: 'location', id: loc.id, name: loc.name })}><Icon name="trash" size="sm" /></button>
          <button class="btn sm" onclick={() => (fromTemplate = true)}><Icon name="copy" size="sm" />Depuis un modèle</button>
          <button class="btn sm primary" onclick={() => (contEdit = { c: null })}><Icon name="plus" size="sm" />Ajouter un élément</button>
        </div>{/if}
      </div>

      <div class="cont-grid">
        {#each conts as c (c.id)}
          {@const t = c.totals}
          <button class="cont" class:on={c.id === selCont} onclick={() => (selCont = c.id)}>
            <span class="cont-vis">
              {#if c.rackable}<span class="mini-rack" class:case={c.kind !== 'rack'}>{#each blocks(c) as b}<i style="height:{b.h}%;--c:{b.color}"></i>{/each}</span>{:else}<span class="kind-vis"><KindIll kind={c.kind} size={78} /></span>{/if}
            </span>
            <span class="cont-info">
              <span class="cont-top"><span class="chip">{CONTAINER_KINDS[c.kind].label}</span>{#if c.wheels}<span class="chip">Roulettes</span>{/if}{#if t.over_weight || t.over_power || t.over_u}<span class="chip err"><Icon name="alert" />Alerte</span>{/if}</span>
              <b class="cont-name">{c.name}</b>
              <span class="mono small muted">{c.cable_code} · {specLine(c)}</span>
              <span class="meters">
                {#if c.rackable}<span class="meter" class:err={t.over_u}><span>Hauteur</span><span class="mono">{nf(t.used_u, t.used_u % 1 ? 1 : 0)} / {c.height_u} U</span><Vu value={pct(t.used_u, t.total_u)} mode="load" n={24} /></span>{/if}
                <span class="meter" class:err={t.over_weight}><span>Poids</span><span class="mono">{nf(t.weight_kg, 1)}{c.max_weight_kg ? ` / ${nf(c.max_weight_kg)}` : ''} kg</span>{#if c.max_weight_kg}<Vu value={pct(t.weight_kg, c.max_weight_kg)} mode="load" n={24} />{/if}</span>
                <span class="meter" class:err={t.over_power}><span>Conso</span><span class="mono">{nf(t.power_w)}{c.max_power_w ? ` / ${nf(c.max_power_w)}` : ''} W</span>{#if c.max_power_w}<Vu value={pct(t.power_w, c.max_power_w)} mode="load" n={24} />{/if}</span>
              </span>
            </span>
          </button>
        {/each}
        <button class="cont" class:on={selCont === 'free'} onclick={() => (selCont = 'free')} style="grid-template-columns:1fr">
          <span class="cont-info"><span class="cont-top"><span class="chip">Sans élément</span></span><b class="cont-name">Équipements libres</b><span class="small muted">{loc.totals.free_items} article{loc.totals.free_items > 1 ? 's' : ''}, comptés dans la nomenclature et les totaux.</span></span>
        </button>
      </div>

      <section class="card">
        <div class="card-head">
          <div><h2>{cont ? cont.name : 'Équipements libres'}</h2><span class="small muted">{arts.reduce((s, a) => s + a.quantity, 0)} article{arts.reduce((s, a) => s + a.quantity, 0) > 1 ? 's' : ''}{cont ? ` · code câble ${cont.cable_code}` : ''}{cont?.totals.volume_m3 ? ` · ${nf(cont.totals.volume_m3, 2)} m³` : ''}</span></div>
          {#if writable}<div class="head-actions">
            {#if cont && can('catalogue', 'ecriture')}<button class="btn sm ghost" onclick={() => saveAsTemplate(cont)} title="Copie l’élément et son contenu dans la bibliothèque"><Icon name="copy" size="sm" />Enregistrer comme modèle</button>{/if}
            {#if cont}<button class="btn sm" onclick={() => (contEdit = { c: cont })}><Icon name="pen" size="sm" />Modifier</button><button class="btn sm ghost danger" onclick={() => (confirmDel = { kind: 'container', id: cont.id, name: cont.name })}><Icon name="trash" size="sm" /></button>{/if}
            <button class="btn sm primary" onclick={() => (adding = true)}><Icon name="plus" size="sm" />Ajouter du matériel</button>
          </div>{/if}
        </div>
        {#if cont?.kind === 'monitor_wall' && cont.details?.wall}
          <WallView wall={cont.details.wall} screens={gearOf(cont)} {writable} onsave={(w) => saveDetails(cont, { ...cont.details, wall: w })} />
        {:else if (cont?.kind === 'desk' || cont?.kind === 'operator_position') && cont.details?.positions?.length}
          <DeskView positions={cont.details.positions} gear={gearOf(cont)} {writable} onsave={(ps) => saveDetails(cont, { ...cont.details, positions: ps })} />
        {/if}
        {#if cont && (cont.totals.over_u || cont.totals.over_weight || cont.totals.over_power)}
          <div class="alert err" style="margin-top:12px"><Icon name="alert" /><span>{[cont.totals.over_u && 'la hauteur disponible', cont.totals.over_weight && 'le poids maximum', cont.totals.over_power && 'la puissance maximum'].filter(Boolean).join(', ')} {cont.totals.over_u || cont.totals.over_weight || cont.totals.over_power ? 'dépassé(e)' : ''}.</span></div>
        {/if}
        <div class="tbl-wrap"><table class="tbl">
          <thead><tr><th>Article</th><th>Quantité</th><th class="num hide-m">Hauteur</th><th class="num hide-m">Poids</th><th class="num hide-m">Conso</th><th></th></tr></thead>
          <tbody>
            {#each arts as a (a.id)}
              {@const it = item(a)}
              {@const inst = instOf(a)}
              <tr>
                <td><div class="art-name"><span><b>{a.label}</b><span class="small muted">{a.nature === 'equipment' ? 'Équipement' : a.nature === 'adapter' ? 'Adaptateur' : a.accessory?.kind === 'panel' ? `Panel · ${a.accessory.panel.slots.length} connecteur${a.accessory.panel.slots.length > 1 ? 's' : ''}` : 'Accessoire'}{it?.brand ? ` · ${it.brand}` : ''}{it && !it.active ? ' · fiche inactive' : ''}</span></span></div></td>
                <td>{#if writable && a.nature !== 'accessory' && !it?.modular}<span class="stepper"><button onclick={() => setQty(a, a.quantity - 1)} aria-label="Retirer un">−</button><span>{a.quantity}</span><button onclick={() => setQty(a, a.quantity + 1)} aria-label="Ajouter un">+</button></span>{:else}{a.quantity}{/if}</td>
                <td class="num hide-m">{heightOf(a) ? `${nf(heightOf(a)! * a.quantity, 1).replace(',0', '')} U` : '—'}</td>
                <td class="num hide-m">{it?.weight_kg ? `${nf(it.weight_kg * a.quantity, 1)} kg` : '—'}</td>
                <td class="num hide-m">{it?.power_w ? `${nf(it.power_w * a.quantity)} W` : '—'}</td>
                <td><div class="dl-actions">
                  {#if a.accessory?.kind === 'panel'}<button class="btn sm ghost" onclick={() => (panelOf = a)}><Icon name="grid" size="sm" />{writable ? 'Modifier' : 'Voir'}</button>{/if}
                  {#if inst.length}<button class="btn sm ghost" onclick={() => toggle(a.id)} aria-expanded={open.has(a.id)}>{inst.length} repère{inst.length > 1 ? 's' : ''}<Icon name="chevd" size="sm" /></button>{/if}
                  {#if writable}<button class="btn sm ghost" title="Déplacer" onclick={() => (moving = { a, target: '', instance: inst.at(-1)?.id ?? null })}><Icon name="arrowr" size="sm" /></button><button class="btn sm ghost danger" title="Supprimer" onclick={() => (confirmDel = { kind: 'article', id: a.id, name: a.label })}><Icon name="trash" size="sm" /></button>{/if}
                </div></td>
              </tr>
              {#if open.has(a.id)}
                <tr><td colspan="6" style="background:var(--surface-2)">
                  <div class="inst-grid">
                    <span class="lbl-t">Repère</span><span class="lbl-t">N° de série</span><span class="lbl-t">Adresse MAC</span><span class="lbl-t">Étiquette d’inventaire</span>
                    {#each inst as i (i.id)}
                      <b class="mono">{i.tag}</b>
                      <input class="input mono" value={i.serial ?? ''} disabled={!writable} onchange={(e) => saveInstance(i, 'serial', (e.target as HTMLInputElement).value)} aria-label="Numéro de série de {i.tag}" placeholder="—">
                      <input class="input mono" value={i.mac ?? ''} disabled={!writable} onchange={(e) => saveInstance(i, 'mac', (e.target as HTMLInputElement).value)} aria-label="Adresse MAC de {i.tag}" placeholder="00:00:00:00:00:00">
                      <input class="input mono" value={i.inventory_tag ?? ''} disabled={!writable} onchange={(e) => saveInstance(i, 'inventory_tag', (e.target as HTMLInputElement).value)} aria-label="Étiquette d’inventaire de {i.tag}" placeholder="Code-barres ou QR">
                    {/each}
                  </div>
                </td></tr>
              {/if}
            {:else}<tr><td colspan="6" class="empty">Rien ici pour l’instant.{#if writable} <button class="ilink" onclick={() => (adding = true)}>Ajouter du matériel</button>{/if}</td></tr>{/each}
          </tbody>
        </table></div>
      </section>
    </div>
  {/if}
</div>

{#if locEdit}
  <Dialog title={locEdit.id ? 'Modifier l’emplacement' : 'Nouvel emplacement'} eyebrow={view.project.name} onclose={() => (locEdit = null)}>
    <form id="locf" class="stack" onsubmit={saveLoc}>
      <div class="field"><label for="l-name">Nom</label><input class="input" id="l-name" bind:value={locEdit.name} required maxlength="120" placeholder="Ex. Bord terrain"><FieldError message={locErr.name} /></div>
      <div class="field"><label for="l-code">Code câble</label><input class="input mono" id="l-code" bind:value={locEdit.code} maxlength="5" placeholder="Tiré du nom"><span class="help">5 caractères au plus ; il entre dans la numérotation des câbles.</span></div>
    </form>
    {#snippet footer()}<button class="btn ghost" onclick={() => (locEdit = null)}>Annuler</button><button class="btn primary" type="submit" form="locf">Enregistrer</button>{/snippet}
  </Dialog>
{/if}
{#if contEdit && selLoc}<ContainerDialog {view} container={contEdit.c} locationId={selLoc} onclose={() => (contEdit = null)} ondone={async (id) => { contEdit = null; await reload(); const c = view.containers.find((x) => x.id === id); if (c) { selLoc = c.location_id; selCont = id; } }} />{/if}
{#if fromTemplate && selLoc}<TemplatePick {view} locationId={selLoc} onclose={() => (fromTemplate = false)} ondone={async (id) => { fromTemplate = false; await reload(); selCont = id; }} />{/if}
{#if adding && selLoc}<AddArticle {view} container={cont} locationId={selLoc} onclose={() => (adding = false)} ondone={async () => { await reload(); }} />{/if}
{#if moving}
  <Dialog title="Déplacer « {moving.a.label} »" eyebrow={view.project.name} onclose={() => (moving = null)}>
    {#if moving.a.nature === 'equipment' && moving.a.quantity > 1}
      <div class="field"><label for="mv-i">Exemplaire à déplacer</label><select class="input" id="mv-i" bind:value={moving.instance}>{#each instOf(moving.a) as i}<option value={i.id}>{i.tag}{i.serial ? ` · ${i.serial}` : ''}</option>{/each}</select><span class="help">L’article compte {moving.a.quantity} exemplaires : un seul part, avec son repère.</span></div>
    {/if}
    <div class="field"><label for="mv-t">Vers</label>
      <select class="input" id="mv-t" bind:value={moving.target}>
        <option value="" disabled>Choisir…</option>
        {#each view.locations as l}
          <optgroup label={l.name}>
            {#each view.containers.filter((c) => c.location_id === l.id && c.id !== moving!.a.container_id) as c}<option value="c:{c.id}" disabled={moving.a.nature === 'accessory' && !c.rackable}>{c.name}</option>{/each}
            {#if moving.a.nature !== 'accessory' && !(moving.a.container_id === null && moving.a.location_id === l.id)}<option value="l:{l.id}">Équipements libres de {l.name}</option>{/if}
          </optgroup>
        {/each}
      </select>
    </div>
    {#snippet footer()}<button class="btn ghost" onclick={() => (moving = null)}>Annuler</button><button class="btn primary" onclick={doMove} disabled={!moving?.target}>Déplacer</button>{/snippet}
  </Dialog>
{/if}
{#if confirmDel}
  <Confirm title="Supprimer « {confirmDel.name} » ?" eyebrow={confirmDel.kind === 'container' ? 'Élément' : confirmDel.kind === 'location' ? 'Emplacement' : 'Article'} confirmLabel="Supprimer" danger onconfirm={del} onclose={() => (confirmDel = null)}>
    <p>{confirmDel.kind === 'container' ? 'Ses articles et leurs exemplaires partent avec lui.' : confirmDel.kind === 'location' ? 'L’emplacement doit être vide : sans élément ni équipement libre.' : 'Ses exemplaires et leurs repères partent avec lui.'}</p>
  </Confirm>
{/if}

{#if panelOf}<ProjectPanel article={panelOf} readonly={!writable} eyebrow="Panel du projet" onclose={() => (panelOf = null)} ondone={reload} />{/if}

<style>
  .kind-vis { display: grid; place-items: center; width: 84px; height: 148px; --ki: var(--sig-v); }
  .inst-grid { display: grid; grid-template-columns: 80px repeat(3, minmax(0, 1fr)); gap: 8px 10px; align-items: center; padding: 6px 0; }
  .inst-grid .input { height: 36px; font-size: 13.5px; }
  @media (max-width: 860px) { .inst-grid { grid-template-columns: 70px minmax(0, 1fr); } .inst-grid .lbl-t:nth-child(n + 3) { display: none; } }
</style>
