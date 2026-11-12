<script lang="ts">
  // Planche d'élévations (section 9.8) : composition libre des racks (faces au choix), des
  // titres de lieux et des emplacements réservés, sur une ou plusieurs feuilles avec cartouche.
  // Réglages mémorisés par projet ; export PDF vectoriel, PNG ou JPG.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import SheetPage from '../../components/SheetPage.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { loadRefs } from '../../lib/catalog.svelte';
  import type { ProjectView } from '../../lib/projects';
  import {
    PAPERS, SCALES, FACE_LABEL, MARGIN, autoLayout, blankSheet, fileName, itemSize, pageSize, uid,
    type Face, type Paper, type RackData, type Sheet, type SheetItem,
  } from '../../lib/sheet';

  let { view }: { view: ProjectView } = $props();
  loadRefs();
  const pid = $derived(view.project.id);
  const writable = $derived(can('projets', 'ecriture'));
  const rackables = $derived(view.containers.filter((c) => c.rackable));

  let sheet = $state<Sheet | null>(null);
  let rev = 0;
  let racks = $state<Record<string, RackData>>({});
  let site = $state('Strata Arkyo');
  let cur = $state(0);
  let selected = $state<string | null>(null);
  let saving = $state<'idle' | 'pending' | 'saving' | 'saved'>('idle');
  let relayout = $state(false);
  let faces = $state<Face[]>(['front']);
  let addRack = $state({ cid: '', face: 'front' as Face });
  let exporting = $state(false);
  let svgHost = $state<HTMLDivElement>(null!);
  let exportHost = $state<HTMLDivElement>(null!);

  const info = $derived({
    project: view.project.name, client: view.project.client ?? 'Projet interne', venue: view.project.venue ?? '', site,
    date: new Date().toLocaleDateString('fr-FR'),
    locations: Object.fromEntries(view.locations.map((l) => [l.id, { name: l.name, code: l.code }])),
  });
  const units = (cid: string) => racks[cid]?.units ?? 42;

  async function loadRacks() {
    const out: Record<string, RackData> = {};
    await Promise.all(rackables.map(async (c) => {
      try { const r = await api.get<{ elements: any[] }>(`/containers/${c.id}/rack`); out[c.id] = { name: c.name, units: c.height_u ?? 42, elements: r.elements }; }
      catch { out[c.id] = { name: c.name, units: c.height_u ?? 42, elements: [] }; }
    }));
    racks = out;
  }
  async function load() {
    try {
      const [s, st] = await Promise.all([api.get<{ data: Sheet | null; rev: number }>(`/projects/${pid}/settings/rack_sheet`), api.get<{ name: string }>('/site')]);
      site = st.name;
      await loadRacks();
      rev = s.rev;
      if (s.data && s.data.pages) sheet = { ...blankSheet(), ...s.data, block: { ...blankSheet().block, ...s.data.block } };
      else { const b = blankSheet(); b.pages = layout(b, ['front']); sheet = b; }
      cur = Math.min(cur, sheet.pages.length - 1);
    } catch (e) { fail(e); }
  }
  // Le projet se recharge sur les événements des autres membres : les racks suivent.
  $effect(() => { view; untrack(() => { if (!sheet) load(); else loadRacks(); }); });

  const layout = (s: Sheet, f: Face[]) => autoLayout(s, rackables.map((c) => ({ id: c.id, location_id: c.location_id, units: c.height_u ?? 42 })), view.locations, f);

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
    const [w, h] = itemSize(drag.it, sheet.scale, units);
    drag.it.x = Math.round(Math.min(Math.max(MARGIN / 2, x - drag.dx), W - MARGIN / 2 - w));
    drag.it.y = Math.round(Math.min(Math.max(MARGIN / 2, y - drag.dy), H - MARGIN / 2 - h));
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
  function doLayout() { if (!sheet) return; sheet.pages = layout(sheet, faces.length ? faces : ['front']); cur = 0; selected = null; relayout = false; changed(); }
  const missing = $derived(rackables.filter((c) => !sheet?.pages.some((p) => p.items.some((i) => i.kind === 'rack' && i.container_id === c.id))));

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
  async function exportSheet() {
    if (!sheet) return;
    exporting = true;
    try {
      await new Promise((r) => setTimeout(r, 50));
      const svgs = [...exportHost.querySelectorAll('svg')] as SVGSVGElement[];
      const [W, H] = pageSize(sheet);
      if (sheet.format === 'pdf') {
        const { jsPDF } = await import('jspdf');
        await import('svg2pdf.js');
        const doc = new jsPDF({ unit: 'mm', format: [W, H], orientation: W > H ? 'landscape' : 'portrait' });
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
{:else if !rackables.length}
  <section class="card" style="padding:28px;display:grid;gap:12px;justify-items:start"><p class="lead" style="margin:0">Aucun élément rackable : ajoutez une baie ou un flight-case pour composer la planche.</p><a class="btn primary" href="/projets/{pid}/inventaire"><Icon name="plus" size="sm" />Ajouter un élément</a></section>
{:else}
  <div class="sh">
    <aside class="sh-side">
      <section class="card sh-card">
        <span class="lbl-t">Feuille</span>
        <div class="field"><span class="flabel">Format</span><div class="seg wrap">{#each Object.keys(PAPERS) as p}<button class:on={sheet.paper === p} disabled={!writable} onclick={() => { sheet!.paper = p as Paper; changed(); }}>{p}</button>{/each}</div></div>
        <div class="sh-2 wide">
          <div class="field"><span class="flabel">Orientation</span><div class="seg">{#each [['portrait', 'Portrait'], ['landscape', 'Paysage']] as [k, l]}<button class:on={sheet.orientation === k} disabled={!writable} onclick={() => { sheet!.orientation = k as Sheet['orientation']; changed(); }}>{l}</button>{/each}</div></div>
          <div class="field"><label for="sh-sc">Échelle</label><select class="input" id="sh-sc" value={sheet.scale} disabled={!writable} onchange={(e) => { sheet!.scale = Number((e.target as HTMLSelectElement).value); changed(); }}>{#each SCALES as s}<option value={s}>1:{s}</option>{/each}</select></div>
        </div>
        {#if writable}<button class="btn sm" onclick={() => (relayout = true)}><Icon name="grid" size="sm" />Disposer automatiquement</button>{/if}
      </section>

      {#if writable}
        <section class="card sh-card">
          <span class="lbl-t">Ajouter sur la feuille</span>
          <div class="sh-2">
            <select class="input" bind:value={addRack.cid} aria-label="Rack à ajouter"><option value="">Rack…</option>{#each rackables as c}<option value={c.id}>{c.name}{missing.includes(c) ? ' · absent' : ''}</option>{/each}</select>
            <select class="input" bind:value={addRack.face} aria-label="Face">{#each Object.entries(FACE_LABEL) as [f, l]}<option value={f}>{l}</option>{/each}</select>
          </div>
          <button class="btn sm" disabled={!addRack.cid} onclick={() => add({ kind: 'rack', container_id: addRack.cid, face: addRack.face })}><Icon name="server" size="sm" />Ajouter le rack</button>
          <div class="sh-2">
            <select class="input" aria-label="Titre de lieu" onchange={(e) => { const s = e.target as HTMLSelectElement; if (s.value) add({ kind: 'location', location_id: s.value, w: 160 }); s.value = ''; }}><option value="">+ Titre de lieu…</option>{#each view.locations as l}<option value={l.id}>{l.name}</option>{/each}</select>
            <button class="btn sm" onclick={() => add({ kind: 'reserved', label: 'Emplacement réservé', w: 60, h: 80 })}><Icon name="plus" size="sm" />Réservé</button>
          </div>
          {#if missing.length}<span class="muted small">Absents de la planche : {missing.map((c) => c.name).join(', ')}</span>{/if}
        </section>
      {/if}

      {#if sel && writable}
        <section class="card sh-card sel">
          <span class="lbl-t">Sélection</span>
          {#if sel.kind === 'rack'}
            <b>{racks[sel.container_id ?? '']?.name ?? 'Rack'}</b>
            <div class="field"><span class="flabel">Face</span><div class="seg wrap">{#each Object.entries(FACE_LABEL) as [f, l]}<button class:on={sel.face === f} onclick={() => { sel.face = f as Face; changed(); }}>{l}</button>{/each}</div></div>
          {:else if sel.kind === 'reserved'}
            <div class="field"><label for="sh-rl">Libellé</label><input class="input" id="sh-rl" bind:value={sel.label} oninput={changed} maxlength="60"></div>
            <div class="sh-2">
              <div class="field"><label for="sh-rw">Largeur (mm)</label><input class="input mono" id="sh-rw" type="number" min="10" max="1000" bind:value={sel.w} oninput={changed}></div>
              <div class="field"><label for="sh-rh">Hauteur (mm)</label><input class="input mono" id="sh-rh" type="number" min="10" max="1000" bind:value={sel.h} oninput={changed}></div>
            </div>
          {:else}
            <div class="field"><label for="sh-lw">Largeur du titre (mm)</label><input class="input mono" id="sh-lw" type="number" min="30" max="1100" bind:value={sel.w} oninput={changed}></div>
          {/if}
          <div class="sh-2">
            <select class="input" aria-label="Envoyer sur une autre feuille" onchange={(e) => { const s = e.target as HTMLSelectElement; if (s.value !== '') toPage(Number(s.value)); s.value = ''; }}><option value="">Envoyer sur…</option>{#each sheet.pages as _, i}{#if i !== cur}<option value={i}>Feuille {i + 1}</option>{/if}{/each}<option value={sheet.pages.length}>Nouvelle feuille</option></select>
            <button class="btn sm ghost danger" onclick={removeSel}><Icon name="trash" size="sm" />Retirer</button>
          </div>
        </section>
      {/if}

      <section class="card sh-card">
        <label class="toggle-row"><input type="checkbox" bind:checked={sheet.titleBlock} disabled={!writable} onchange={changed}><span class="lbl-t">Cartouche</span></label>
        {#if sheet.titleBlock}
          <div class="field"><label for="sh-t">Titre</label><input class="input" id="sh-t" bind:value={sheet.block.title} oninput={changed} disabled={!writable} maxlength="80"></div>
          <div class="sh-2">
            <div class="field"><label for="sh-ref">Référence</label><input class="input mono" id="sh-ref" bind:value={sheet.block.reference} oninput={changed} disabled={!writable} maxlength="40"></div>
            <div class="field"><label for="sh-v">Version</label><input class="input mono" id="sh-v" bind:value={sheet.block.version} oninput={changed} disabled={!writable} maxlength="10"></div>
            <div class="field"><label for="sh-a">Auteur</label><input class="input" id="sh-a" bind:value={sheet.block.author} oninput={changed} disabled={!writable} maxlength="60"></div>
            <div class="field"><label for="sh-c">Vérificateur</label><input class="input" id="sh-c" bind:value={sheet.block.checker} oninput={changed} disabled={!writable} maxlength="60"></div>
          </div>
          <div class="field"><label for="sh-n">Normes</label><input class="input" id="sh-n" bind:value={sheet.block.standards} oninput={changed} disabled={!writable} maxlength="80"></div>
          <div class="field"><label for="sh-no">Notes</label><input class="input" id="sh-no" bind:value={sheet.block.notes} oninput={changed} disabled={!writable} maxlength="120"></div>
          <span class="muted small">Site, projet, client, lieu, date, numéro de feuille et échelle sont remplis automatiquement.</span>
        {/if}
      </section>
    </aside>

    <section class="sh-main">
      <div class="sh-bar">
        <div class="seg" role="tablist" aria-label="Feuilles">{#each sheet.pages as p, i (p.id)}<button class:on={cur === i} onclick={() => { cur = i; selected = null; }}>Feuille {i + 1} <span class="muted mono">{p.items.filter((x) => x.kind === 'rack').length}</span></button>{/each}</div>
        {#if writable}<button class="icon-btn sm" title="Nouvelle feuille" aria-label="Nouvelle feuille" onclick={addPage}><Icon name="plus" size="sm" /></button>{#if sheet.pages.length > 1}<button class="icon-btn sm" title="Supprimer cette feuille" aria-label="Supprimer cette feuille" onclick={dropPage}><Icon name="trash" size="sm" /></button>{/if}{/if}
        <span style="flex:1"></span>
        <span class="muted small sh-save">{saving === 'pending' || saving === 'saving' ? 'Enregistrement…' : saving === 'saved' ? 'Réglages enregistrés' : ''}</span>
        <div class="seg">{#each [['pdf', 'PDF'], ['png', 'PNG'], ['jpg', 'JPG']] as [k, l]}<button class:on={sheet.format === k} onclick={() => { sheet!.format = k as Sheet['format']; changed(); }}>{l}</button>{/each}</div>
        <button class="btn primary" disabled={exporting} onclick={exportSheet}>{#if exporting}<span class="spin"></span>{:else}<Icon name="download" size="sm" />{/if}Exporter</button>
      </div>
      <div class="sh-desk" role="presentation" onpointerdown={(e) => { if (!(e.target as Element).closest('.it')) selected = null; }}>
        <div class="sh-paper" bind:this={svgHost} style="aspect-ratio:{pageSize(sheet)[0]} / {pageSize(sheet)[1]}">
          {#if page}<SheetPage {sheet} {page} {racks} {info} index={cur} total={sheet.pages.length} editable={writable} {selected} onpick={pick} />{/if}
        </div>
      </div>
      <p class="muted small" style="margin:6px 2px 0">{sheet.paper} {sheet.orientation === 'portrait' ? 'portrait' : 'paysage'} · 1:{sheet.scale}{writable ? ' · glissez les racks, titres et réservations pour composer la feuille' : ''} · fichier {fileName(view.project.name, sheet.format)}</p>
    </section>
  </div>

  <div class="sh-export" bind:this={exportHost} aria-hidden="true">
    {#each sheet.pages as p, i (p.id)}<SheetPage {sheet} page={p} {racks} {info} index={i} total={sheet.pages.length} />{/each}
  </div>
{/if}

{#if relayout}
  <Confirm title="Disposer automatiquement ?" confirmLabel="Disposer" onconfirm={doLayout} onclose={() => (relayout = false)}>
    <p>Les racks sont rangés lieu par lieu, de gauche à droite, sur autant de feuilles que nécessaire. La composition actuelle est remplacée.</p>
    <div class="seg" style="margin-top:8px">{#each Object.entries(FACE_LABEL) as [f, l]}<button class:on={faces.includes(f as Face)} onclick={() => (faces = faces.includes(f as Face) ? faces.filter((x) => x !== f) : [...faces, f as Face])}>{l}</button>{/each}</div>
  </Confirm>
{/if}

<style>
  .sh { display: grid; grid-template-columns: 340px minmax(0, 1fr); gap: 18px; align-items: start; }
  .sh-side { display: grid; gap: 12px; position: sticky; top: 12px; max-height: calc(100vh - 24px); overflow: auto; padding-bottom: 4px; }
  .sh-card { padding: 14px; display: grid; gap: 10px; }
  .sh-card.sel { box-shadow: 0 0 0 1.5px var(--accent, #4F6BFF); }
  .sh-2 { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; align-items: end; }
  .sh-2 .input { height: 38px; font-size: 14px; }
  .seg.wrap { flex-wrap: wrap; }
  .sh-2.wide { grid-template-columns: 1.4fr 1fr; }
  .sh-main { min-width: 0; display: grid; gap: 10px; }
  .sh-bar { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .sh-desk { padding: 28px; border-radius: 18px; background: repeating-linear-gradient(0deg, transparent 0 23px, color-mix(in srgb, var(--fg) 5%, transparent) 23px 24px), repeating-linear-gradient(90deg, transparent 0 23px, color-mix(in srgb, var(--fg) 5%, transparent) 23px 24px), var(--surface-2); overflow: auto; }
  .sh-paper { width: 100%; max-width: 1400px; margin: 0 auto; background: #fff; box-shadow: 0 18px 40px -18px rgba(0, 0, 0, .45), 0 0 0 1px rgba(0, 0, 0, .08); touch-action: none; }
  .sh-export { position: fixed; left: -100000px; top: 0; width: 1200px; pointer-events: none; }
  @media (max-width: 1000px) { .sh { grid-template-columns: 1fr; } .sh-side { position: static; max-height: none; } }
</style>
