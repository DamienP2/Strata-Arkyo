<script lang="ts">
  // Planche d'élévations (section 9.8) : composition des baies (vues avant, arrière, côté, 3D)
  // et du mobilier (face, côté, dessus, 3D, casquettes), titres de lieux et emplacements
  // réservés, sur une ou plusieurs feuilles avec cartouche. La feuille occupe la page ; le
  // panneau de droite montre l'élément sélectionné, ou la bibliothèque et le cartouche.
  // Réglages mémorisés par projet ; export PDF vectoriel, PNG ou JPG.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import SheetPage from '../../components/SheetPage.svelte';
  import Sheet3DView from '../../components/Sheet3DView.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { loadRefs } from '../../lib/catalog.svelte';
  import type { ProjectView } from '../../lib/projects';
  import {
    PAPERS, SCALES, RACK_VIEWS, DESK_VIEWS, FRAME, MARGIN, autoLayout, blankSheet, fileName, groupLayout, itemSize, normalize, pageSize, uid, usableW, viewsOf,
    type Ctx, type Paper, type Sheet, type SheetItem,
  } from '../../lib/sheet';
  import { isDesk, loadCtx, shot3D, shotsFor } from '../../lib/sheetctx';

  let { view }: { view: ProjectView } = $props();
  loadRefs();
  const pid = $derived(view.project.id);
  const writable = $derived(can('projets', 'ecriture'));
  const bays = $derived(view.containers.filter((c) => c.rackable && c.kind !== 'casquette'));
  const desks = $derived(view.containers.filter(isDesk));

  let sheet = $state<Sheet | null>(null);
  let rev = 0;
  let ctx = $state<Ctx>({ racks: {}, desks: {} });
  let shots = $state<Record<string, string>>({});
  let site = $state('Strata Arkyo');
  let cur = $state(0);
  let selected = $state<string | null>(null);
  let saving = $state<'idle' | 'pending' | 'saving' | 'saved'>('idle');
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
    date: new Date().toLocaleDateString('fr-FR'),
    locations: Object.fromEntries(view.locations.map((l) => [l.id, { name: l.name, code: l.code }])),
  });

  async function load() {
    try {
      const [s, st, c] = await Promise.all([api.get<{ data: Sheet | null; rev: number }>(`/projects/${pid}/settings/rack_sheet`), api.get<{ name: string }>('/site'), loadCtx(view)]);
      site = st.name; ctx = c; rev = s.rev;
      if (s.data && s.data.pages) sheet = normalize({ ...blankSheet(), ...s.data, block: { ...blankSheet().block, ...s.data.block } });
      else { const b = blankSheet(); b.pages = [{ id: uid(), items: [] }]; sheet = b; }
      cur = Math.min(cur, sheet.pages.length - 1);
    } catch (e) { fail(e); }
  }
  // Le projet se recharge sur les événements des autres membres : les élévations suivent.
  $effect(() => { view; untrack(() => { if (!sheet) load(); else loadCtx(view).then((c) => (ctx = c)); }); });

  const layout = (s: Sheet) => autoLayout(s, ctx, bays, desks, view.locations, rackViews.length ? rackViews : ['front'], deskViews.length ? deskViews : ['front']);

  // Enregistrement automatique, regroupé.
  let timer: ReturnType<typeof setTimeout>;
  function changed() {
    if (!writable || !sheet) return;
    saving = 'pending';
    clearTimeout(timer);
    timer = setTimeout(save, 700);
  }
  async function save() {
    if (!sheet) return;
    saving = 'saving';
    try { const r = await api.put<{ rev: number }>(`/projects/${pid}/settings/rack_sheet`, { rev, data: $state.snapshot(sheet) }); rev = r.rev; saving = 'saved'; }
    catch (e) {
      if (e instanceof ApiError && e.status === 409) { toast('La planche vient d’être modifiée par un autre membre : elle a été rechargée.', 'warn'); sheet = null; await load(); saving = 'idle'; }
      else { fail(e); saving = 'idle'; }
    }
  }

  const page = $derived(sheet?.pages[cur] ?? null);
  const sel = $derived(page?.items.find((i) => i.id === selected) ?? null);
  const selName = $derived(sel ? (sel.kind === 'rack' ? ctx.racks[sel.container_id ?? '']?.name : sel.kind === 'desk' ? ctx.desks[sel.container_id ?? '']?.name : sel.kind === 'location' ? (info.locations[sel.location_id ?? '']?.name ?? 'Titre de lieu') : 'Emplacement réservé') ?? 'Élément retiré' : '');
  const box3d = $derived(sel && sheet && (sel.kind === 'rack' || sel.kind === 'desk') ? groupLayout(sel, sheet.scale, ctx).views.find((v) => v.v === '3d') ?? null : null);

  // Vues 3D de la feuille affichée : rendues sans fond, au format du cadre, à chaque changement.
  $effect(() => {
    if (!sheet || !page) return;
    const k = sheet.scale;
    const list = page.items.filter((i) => (i.kind === 'rack' || i.kind === 'desk') && i.views?.includes('3d'));
    void JSON.stringify(list); void ctx;
    untrack(() => { for (const it of list) shot3D($state.snapshot(it) as SheetItem, ctx, k).then((u) => { if (u) shots[it.id] = u; }).catch(() => {}); });
  });

  // Glisser un élément de la feuille, au millimètre.
  let drag: { it: SheetItem; dx: number; dy: number } | null = null;
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
    drag = { it, dx: x - it.x, dy: y - it.y };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up, { once: true });
  }
  function move(e: PointerEvent) {
    if (!drag || !sheet) return;
    const [x, y] = toMm(e);
    const [W, H] = pageSize(sheet);
    const [w, h] = itemSize(drag.it, sheet.scale, ctx, usableW(sheet));
    drag.it.x = Math.round(Math.min(Math.max(FRAME, x - drag.dx), Math.max(FRAME, W - FRAME - w)));
    drag.it.y = Math.round(Math.min(Math.max(FRAME, y - drag.dy), Math.max(FRAME, H - FRAME - h)));
  }
  function up() { window.removeEventListener('pointermove', move); if (drag) changed(); drag = null; }

  function add(it: Omit<SheetItem, 'id' | 'x' | 'y'>) {
    if (!page) return;
    const n: SheetItem = { id: uid(), x: MARGIN + 4, y: MARGIN + 4, ...it };
    page.items.push(n);
    selected = n.id;
    changed();
  }
  function removeSel() { if (!page || !sel) return; page.items = page.items.filter((i) => i !== sel); selected = null; changed(); }
  function toPage(n: number) {
    if (!sheet || !page || !sel) return;
    if (n >= sheet.pages.length) sheet.pages.push({ id: uid(), items: [] });
    const it = sel;
    page.items = page.items.filter((i) => i !== it);
    sheet.pages[n]!.items.push(it);
    cur = n; changed();
  }
  function addPage() { if (!sheet) return; sheet.pages.push({ id: uid(), items: [] }); cur = sheet.pages.length - 1; selected = null; changed(); }
  function dropPage() { if (!sheet || sheet.pages.length < 2) return; sheet.pages.splice(cur, 1); cur = Math.max(0, cur - 1); selected = null; changed(); }
  function doLayout() { if (!sheet) return; sheet.pages = layout(sheet); cur = 0; selected = null; relayout = false; changed(); }
  /** Feuilles où figure un élément (numéros à partir de 1). */
  const where = (cid: string) => (sheet?.pages ?? []).flatMap((p, i) => (p.items.some((x) => x.container_id === cid && (x.kind === 'rack' || x.kind === 'desk')) ? [i + 1] : []));
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
      toast(`Planche exportée (${sheet.format.toUpperCase()}, ${svgs.length} feuille${svgs.length > 1 ? 's' : ''}).`);
    } catch (e) { fail(e); } finally { exporting = false; }
  }
</script>

{#if !sheet}<span class="spin"></span>
{:else if !bays.length && !desks.length}
  <section class="card" style="padding:28px;display:grid;gap:12px;justify-items:start"><p class="lead" style="margin:0">Aucune baie ni mobilier : ajoutez-en pour composer la planche.</p><a class="btn primary" href="/projets/{pid}/inventaire"><Icon name="plus" size="sm" />Ajouter un élément</a></section>
{:else}
  <div class="pl">
    <div class="pl-bar">
      <select class="input pl-sel" aria-label="Format" value={sheet.paper} disabled={!writable} onchange={(e) => { sheet!.paper = (e.target as HTMLSelectElement).value as Paper; changed(); }}>{#each Object.keys(PAPERS) as p}<option value={p}>{p}</option>{/each}</select>
      <div class="seg">{#each [['portrait', 'Portrait'], ['landscape', 'Paysage']] as [k, l]}<button class:on={sheet.orientation === k} disabled={!writable} onclick={() => { sheet!.orientation = k as Sheet['orientation']; changed(); }}>{l}</button>{/each}</div>
      <select class="input pl-sel" aria-label="Échelle" value={sheet.scale} disabled={!writable} onchange={(e) => { sheet!.scale = Number((e.target as HTMLSelectElement).value); changed(); }}>{#each SCALES as s}<option value={s}>1:{s}</option>{/each}</select>
      {#if writable}<button class="btn sm ghost" aria-label="Disposer automatiquement" title="Placer toutes les baies et le mobilier sur les feuilles, d’un coup" onclick={() => (relayout = true)}><Icon name="grid" size="sm" /><span class="pl-lbl">Disposer automatiquement</span></button>{/if}
      <span class="pl-sep"></span>
      <div class="seg" role="tablist" aria-label="Feuilles">{#each sheet.pages as p, i (p.id)}<button class:on={cur === i} role="tab" aria-selected={cur === i} title="Feuille {i + 1}" onclick={() => { cur = i; selected = null; }}>Feuille {i + 1}</button>{/each}</div>
      {#if writable}<button class="btn sm ghost" aria-label="Ajouter une feuille" title="Ajouter une feuille à la planche" onclick={addPage}><Icon name="plus" size="sm" /><span class="pl-lbl">Ajouter une feuille</span></button>{#if sheet.pages.length > 1}<button class="icon-btn sm" title="Supprimer cette feuille" aria-label="Supprimer cette feuille" onclick={dropPage}><Icon name="trash" size="sm" /></button>{/if}{/if}
      <span style="flex:1"></span>
      <span class="muted small">{saving === 'pending' || saving === 'saving' ? 'Enregistrement…' : saving === 'saved' ? 'Enregistré' : ''}</span>
      <div class="seg">{#each [['pdf', 'PDF'], ['png', 'PNG'], ['jpg', 'JPG']] as [k, l]}<button class:on={sheet.format === k} onclick={() => { sheet!.format = k as Sheet['format']; changed(); }}>{l}</button>{/each}</div>
      <span title={hasItems ? undefined : 'Planche vide : aucune feuille ne contient d’élément, composez la planche avant de l’exporter.'}><button class="btn primary" disabled={exporting || !hasItems} onclick={exportSheet}>{#if exporting}<span class="spin"></span>Génération…{:else}<Icon name="download" size="sm" />Exporter{/if}</button></span>
    </div>

    <div class="pl-body">
      <div class="pl-desk" role="presentation" onpointerdown={(e) => { if (!(e.target as Element).closest('.it')) selected = null; }}>
        <div class="pl-paper" bind:this={svgHost} style="aspect-ratio:{pageSize(sheet)[0]} / {pageSize(sheet)[1]}">
          {#if page && !page.items.length}<div class="pl-empty"><b>Feuille vide</b><span>{writable ? 'Ajoutez des baies ou du mobilier depuis le panneau de droite, ou « Disposer automatiquement » pour tout placer d’un coup.' : 'Aucun élément sur cette feuille.'}</span></div>{/if}
          {#if page}<SheetPage {sheet} {page} {ctx} {info} {shots} index={cur} total={sheet.pages.length} editable={writable} {selected} onpick={pick} />{/if}
        </div>
        <p class="muted small pl-foot">{sheet.paper} {sheet.orientation === 'portrait' ? 'portrait' : 'paysage'} · 1:{sheet.scale} · feuille {cur + 1} / {sheet.pages.length}{writable ? ' · cliquez un élément pour le régler, glissez-le pour le placer' : ''}</p>
      </div>

      <aside class="pl-panel">
        {#if sel}
          <header class="pl-ph">
            <span class="pl-ic"><Icon name={sel.kind === 'rack' ? 'server' : sel.kind === 'desk' ? 'grid' : sel.kind === 'location' ? 'pin' : 'plus'} size="sm" /></span>
            <span class="pl-pt"><b>{selName}</b><small>{sel.kind === 'rack' ? 'Baie' : sel.kind === 'desk' ? 'Mobilier' : sel.kind === 'location' ? 'Titre de lieu' : 'Emplacement réservé'}</small></span>
            <button class="icon-btn sm" aria-label="Fermer" title="Fermer" onclick={() => (selected = null)}><Icon name="x" size="sm" /></button>
          </header>

          {#if sel.kind === 'rack' || sel.kind === 'desk'}
            {@const opts = sel.kind === 'rack' ? RACK_VIEWS : DESK_VIEWS}
            <section class="pl-sec">
              <span class="lbl-t">Vues affichées</span>
              <div class="pl-chips">{#each Object.entries(opts) as [v, l]}{@const on = viewsOf(sel).includes(v)}<button class="pl-chip" class:on disabled={!writable || (sel.kind === 'desk' && v === 'casq' && !ctx.desks[sel.container_id ?? '']?.casq.length)} aria-pressed={on} onclick={() => toggleView(sel, v)}>{#if on}<Icon name="check" size="sm" />{/if}{l}</button>{/each}</div>
            </section>
            {#if box3d}
              <section class="pl-sec">
                <span class="lbl-t">Vue 3D</span>
                <Sheet3DView bind:this={viewer} it={sel} {ctx} aspect={box3d.w / box3d.h} oncamera={(p) => { sel.cam = p; changed(); }} />
                <span class="muted small">Glisser : tourner · molette : zoom · clic droit : décaler. L’angle est repris sur la feuille.</span>
                <div class="seg wrap">{#each [['quarter', '3/4'], ['front', sel.kind === 'desk' ? 'Opérateur' : 'Avant'], ['side', 'Côté'], ['rear', 'Arrière'], ['top', 'Dessus']] as [k, l]}<button disabled={!writable} onclick={() => viewer?.frame(k as never)}>{l}</button>{/each}</div>
                {#if sel.kind === 'rack' && !/case/.test(ctx.racks[sel.container_id ?? '']?.c?.kind ?? '')}
                  <div class="pl-row"><span class="pl-l">Portes</span><div class="seg">{#each [['removed', 'Retirées'], ['closed', 'Fermées']] as [k, l]}<button class:on={(sel.doors ?? 'removed') === k} disabled={!writable} onclick={() => { sel.doors = k as 'removed' | 'closed'; changed(); }}>{l}</button>{/each}</div></div>
                {/if}
                <div class="pl-row"><span class="pl-l">{sel.kind === 'desk' ? 'Joues' : 'Flancs'}</span><div class="seg">{#each [[true, 'Avec'], [false, 'Sans']] as [k, l]}<button class:on={(sel.sides ?? true) === k} disabled={!writable} onclick={() => { sel.sides = k as boolean; changed(); }}>{l}</button>{/each}</div></div>
                <label class="pl-row"><span class="pl-l">Cadre</span><input type="range" min="0.4" max="3" step="0.05" value={sel.r3d ?? (sel.kind === 'desk' ? 1.8 : 0.8)} disabled={!writable} oninput={(e) => { sel.r3d = Number((e.target as HTMLInputElement).value); keepIn(sel); changed(); }}><span class="mono small">{(sel.r3d ?? (sel.kind === 'desk' ? 1.8 : 0.8)).toFixed(2)}</span></label>
              </section>
            {/if}
          {:else if sel.kind === 'reserved'}
            <section class="pl-sec">
              <div class="field"><label for="pl-rl">Libellé</label><input class="input" id="pl-rl" bind:value={sel.label} oninput={changed} maxlength="60" disabled={!writable}></div>
              <div class="pl-2">
                <div class="field"><label for="pl-rw">Largeur (mm)</label><input class="input mono" id="pl-rw" type="number" min="10" max="1000" bind:value={sel.w} oninput={changed} disabled={!writable}></div>
                <div class="field"><label for="pl-rh">Hauteur (mm)</label><input class="input mono" id="pl-rh" type="number" min="10" max="1000" bind:value={sel.h} oninput={changed} disabled={!writable}></div>
              </div>
            </section>
          {:else}
            <section class="pl-sec"><div class="field"><label for="pl-lw">Largeur du titre (mm)</label><input class="input mono" id="pl-lw" type="number" min="30" max="1100" bind:value={sel.w} oninput={changed} disabled={!writable}></div></section>
          {/if}

          {#if writable}
            <section class="pl-sec pl-2">
              <select class="input" aria-label="Envoyer sur une autre feuille" onchange={(e) => { const s = e.target as HTMLSelectElement; if (s.value !== '') toPage(Number(s.value)); s.value = ''; }}><option value="">Envoyer sur…</option>{#each sheet.pages as _, i}{#if i !== cur}<option value={i}>Feuille {i + 1}</option>{/if}{/each}<option value={sheet.pages.length}>Nouvelle feuille</option></select>
              <button class="btn sm ghost danger" onclick={removeSel}><Icon name="trash" size="sm" />Retirer</button>
            </section>
          {/if}
        {:else}
          {#if writable}
            <section class="pl-sec">
              <span class="lbl-t">Baies</span>
              {#each bays as c (c.id)}
                {@const w = where(c.id)}
                <div class="pl-lib"><span class="pl-n"><b>{c.name}</b><small>{locName(c.location_id)?.code || locName(c.location_id)?.name || ''} · {c.height_u} U</small></span>{#if w.length}<span class="pl-ok" title="Déjà sur la feuille {w.join(', ')}">Feuille {w.join(', ')}</span>{/if}<button class="btn sm" onclick={() => add({ kind: 'rack', container_id: c.id, views: [...rackViews] })}><Icon name="plus" size="sm" />Ajouter</button></div>
              {:else}<span class="muted small">Aucune baie.</span>{/each}
            </section>
            {#if desks.length}
              <section class="pl-sec">
                <span class="lbl-t">Mobilier</span>
                {#each desks as c (c.id)}
                  {@const w = where(c.id)}
                  <div class="pl-lib"><span class="pl-n"><b>{c.name}</b><small>{locName(c.location_id)?.code || locName(c.location_id)?.name || ''} · {ctx.desks[c.id]?.casq.length ?? 0} casquette(s)</small></span>{#if w.length}<span class="pl-ok" title="Déjà sur la feuille {w.join(', ')}">Feuille {w.join(', ')}</span>{/if}<button class="btn sm" onclick={() => add({ kind: 'desk', container_id: c.id, views: [...deskViews] })}><Icon name="plus" size="sm" />Ajouter</button></div>
                {/each}
              </section>
            {/if}
            <section class="pl-sec pl-2">
              <select class="input" aria-label="Ajouter un titre de lieu" title="Ajoute sur la feuille le nom d’un emplacement, en titre au-dessus de ses baies" onchange={(e) => { const s = e.target as HTMLSelectElement; if (s.value) add({ kind: 'location', location_id: s.value, w: 160 }); s.value = ''; }}><option value="">+ Titre de lieu…</option>{#each view.locations as l}<option value={l.id}>{l.name}</option>{/each}</select>
              <button class="btn sm" title="Ajoute un cadre « Emplacement réservé » : la place d’une baie ou d’un meuble à venir" onclick={() => add({ kind: 'reserved', label: 'Emplacement réservé', w: 60, h: 80 })}><Icon name="plus" size="sm" />Réservé</button>
            </section>
          {/if}
          <section class="pl-sec">
            <label class="toggle-row"><input type="checkbox" bind:checked={sheet.titleBlock} disabled={!writable} onchange={changed}><span class="lbl-t">Cartouche</span></label>
            {#if sheet.titleBlock}
              <div class="field"><label for="pl-t">Titre</label><input class="input" id="pl-t" bind:value={sheet.block.title} oninput={changed} disabled={!writable} maxlength="80"></div>
              <div class="pl-2">
                <div class="field"><label for="pl-ref">Référence</label><input class="input mono" id="pl-ref" bind:value={sheet.block.reference} oninput={changed} disabled={!writable} maxlength="40"></div>
                <div class="field"><label for="pl-v">Version</label><input class="input mono" id="pl-v" bind:value={sheet.block.version} oninput={changed} disabled={!writable} maxlength="10"></div>
                <div class="field"><label for="pl-a">Auteur</label><input class="input" id="pl-a" bind:value={sheet.block.author} oninput={changed} disabled={!writable} maxlength="60"></div>
                <div class="field"><label for="pl-c">Vérificateur</label><input class="input" id="pl-c" bind:value={sheet.block.checker} oninput={changed} disabled={!writable} maxlength="60"></div>
              </div>
              <div class="field"><label for="pl-n">Normes</label><input class="input" id="pl-n" bind:value={sheet.block.standards} oninput={changed} disabled={!writable} maxlength="80"></div>
              <div class="field"><label for="pl-no">Notes</label><input class="input" id="pl-no" bind:value={sheet.block.notes} oninput={changed} disabled={!writable} maxlength="120"></div>
              <span class="muted small">Site, projet, client, lieu, date, numéro de feuille et échelle sont remplis automatiquement.</span>
            {/if}
          </section>
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
    <p>Baies puis mobilier, lieu par lieu, de gauche à droite, sur autant de feuilles que nécessaire. La composition actuelle est remplacée.</p>
    <p class="lbl-t" style="margin:10px 0 4px">Vues des baies</p>
    <div class="seg wrap">{#each Object.entries(RACK_VIEWS) as [v, l]}<button class:on={rackViews.includes(v)} onclick={() => (rackViews = rackViews.includes(v) ? rackViews.filter((x) => x !== v) : Object.keys(RACK_VIEWS).filter((x) => x === v || rackViews.includes(x)))}>{l}</button>{/each}</div>
    {#if desks.length}
      <p class="lbl-t" style="margin:10px 0 4px">Vues du mobilier</p>
      <div class="seg wrap">{#each Object.entries(DESK_VIEWS) as [v, l]}<button class:on={deskViews.includes(v)} onclick={() => (deskViews = deskViews.includes(v) ? deskViews.filter((x) => x !== v) : Object.keys(DESK_VIEWS).filter((x) => x === v || deskViews.includes(x)))}>{l}</button>{/each}</div>
    {/if}
  </Confirm>
{/if}

<style>
  .pl { display: grid; gap: 10px; }
  /* Barre étroite : les libellés longs cèdent la place à l’icône (l’infobulle reste). */
  @media (max-width: 1280px) { .pl-lbl { display: none; } }
  .pl-bar { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; padding: 8px 10px; border-radius: 12px; background: var(--surface); box-shadow: inset 0 0 0 1px var(--line); }
  .pl-sel { width: auto; height: 32px; padding: 0 28px 0 10px; font-size: 13px; }
  .pl-sep { width: 1px; height: 22px; background: var(--line-2); margin: 0 4px; }
  .pl-body { display: grid; grid-template-columns: minmax(0, 1fr) 360px; gap: 12px; align-items: start; }
  .pl-desk { padding: 28px; border-radius: 14px; background: radial-gradient(90% 70% at 50% 0%, color-mix(in srgb, var(--fg) 6%, var(--surface-2)), var(--surface-2)); overflow: auto; min-height: 60vh; }
  .pl-paper { width: 100%; max-width: 1500px; margin: 0 auto; background: #fff; box-shadow: 0 24px 50px -22px rgba(0, 0, 0, .55), 0 0 0 1px rgba(0, 0, 0, .08); touch-action: none; }
  .pl-paper { position: relative; }
  .pl-empty { position: absolute; left: 50%; top: 42%; transform: translate(-50%, -50%); display: grid; gap: 4px; text-align: center; max-width: 340px; color: #5B636D; font-size: 13px; pointer-events: none; }
  .pl-empty b { color: #15171A; font-size: 15px; }
  .pl-foot { text-align: center; margin: 12px 0 0; }
  .pl-panel { position: sticky; top: 12px; max-height: calc(100vh - 24px); overflow: auto; border-radius: 14px; background: var(--surface); box-shadow: inset 0 0 0 1px var(--line); }
  .pl-ph { display: flex; align-items: center; gap: 10px; padding: 12px 12px 12px 14px; border-bottom: 1px solid var(--line); position: sticky; top: 0; background: var(--surface); z-index: 1; }
  .pl-ic { width: 32px; height: 32px; display: grid; place-items: center; border-radius: 8px; background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--fg); flex: none; }
  .pl-pt { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.2; }
  .pl-pt b { font-size: 15px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pl-pt small { font-size: 12px; color: var(--fg-3); }
  .pl-sec { padding: 14px; display: grid; gap: 10px; border-bottom: 1px solid var(--line); }
  .pl-sec:last-child { border-bottom: 0; }
  .pl-2 { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; align-items: end; }
  .pl-chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .pl-chip { display: inline-flex; align-items: center; gap: 5px; height: 30px; padding: 0 12px; border: 0; border-radius: 15px; background: var(--surface-2); box-shadow: inset 0 0 0 1px var(--line-2); color: var(--fg-2); font: 600 13px var(--font); cursor: pointer; }
  .pl-chip.on { background: color-mix(in srgb, var(--accent) 18%, transparent); box-shadow: inset 0 0 0 1px var(--accent); color: var(--fg); }
  .pl-chip:disabled { opacity: .45; cursor: default; }
  .pl-row { display: flex; align-items: center; gap: 10px; }
  .pl-row input[type='range'] { flex: 1; }
  .pl-l { width: 58px; font: 600 10.5px var(--mono); letter-spacing: .08em; text-transform: uppercase; color: var(--fg-3); }
  .seg.wrap { flex-wrap: wrap; }
  .pl-lib { display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 10px; border-radius: 9px; background: var(--surface-2); }
  .pl-n { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.2; }
  .pl-n b { font-size: 13.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pl-n small { font-size: 11.5px; color: var(--fg-3); }
  .pl-ok { font: 600 11px var(--mono); color: var(--ok); }
  .pl-export { position: fixed; left: -100000px; top: 0; width: 1200px; pointer-events: none; }
  @media (max-width: 1100px) { .pl-body { grid-template-columns: 1fr; } .pl-panel { position: static; max-height: none; } }
</style>
