<script lang="ts">
  // Éditeur de panel de brassage (sections 6.3 et 9.5) : pose des connecteurs (unique, reste
  // de la rangée, tout le panel ou un nombre donné), déplacement, propriétés par port,
  // groupes de ports colorés par rangée et numérotation des libellés.
  import { untrack, type Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import Confirm from './Confirm.svelte';
  import PanelFace from './PanelFace.svelte';
  import NumField from './NumField.svelte';
  import FieldError from './FieldError.svelte';
  import Seg from './Seg.svelte';
  import { refList, refById, familyColor } from '../lib/catalog.svelte';
  import { canvasKeys } from '../lib/canvaskeys';
  import { toast } from '../lib/toast.svelte';
  import { ApiError } from '../lib/api';
  import {
    PER_U, D_FORMAT_MAX_PER_U, clonePanel, fill, moveSlot, rowOf, colOf, slotAt, stats, dFormatAllowed, total, groupFamily,
    type PanelData, type PanelGroup, type PanelSlot, type FillMode,
  } from '../lib/panel';

  let { initial, readonly = false, onsave, actions }: {
    initial: PanelData; readonly?: boolean; onsave?: (p: PanelData) => Promise<PanelData | void>; actions?: Snippet;
  } = $props();

  let p = $state<PanelData>(untrack(() => clonePanel(initial)));
  let base = $state(untrack(() => JSON.stringify(initial)));
  const dirty = $derived(JSON.stringify(p) !== base);
  let sel = $state<number[]>([]);
  let anchor = 0;
  let moving = $state<PanelSlot | null>(null);
  let group = $state<PanelGroup | null>(null);
  let resize = $state<{ h: number; per: number } | null>(null);
  let saving = $state(false);
  let errs = $state<{ field: string; message: string }[]>([]);

  // Outil de pose
  let conn = $state('bnc');
  let variant = $state<string | null>(null);
  let fn = $state<string | null>(null);
  let width = $state(1);
  let dfmt = $state(false);
  let mode = $state<FillMode>('one');
  let count = $state(4);
  // Groupe et numérotation
  let gLabel = $state('');
  // Le groupe prend la couleur d'une famille de signal (clé du référentiel Fonctions), jamais une couleur libre.
  let gFn = $state('V');
  let prefix = $state('');
  let from = $state(1);

  const connectors = $derived(refList('connectors', true));
  const info = (c: string) => refById('connectors', c)?.data ?? {};
  const fnsOf = (c: string) => (info(c).functions ?? []) as string[];
  const st = $derived(stats(p));
  const one = $derived(sel.length === 1 ? slotAt(p, sel[0]!) : undefined);
  const range = $derived(sel.length > 1);
  const PALETTE = $derived(refList('functions').map((f) => ({ key: f.id, name: f.name, color: familyColor(f.id, { screen: false }) })));
  const mix = $derived.by(() => {
    const m = new Map<string, number>();
    for (const s of p.slots) m.set(s.connector, (m.get(s.connector) ?? 0) + 1);
    return [...m.entries()].map(([c, n]) => `${n} × ${refById('connectors', c)?.name ?? c}`).join(' · ');
  });

  $effect(() => { conn; variant = null; if (fn && !fnsOf(conn).includes(fn)) fn = null; if (!dFormatAllowed(p, conn)) dfmt = false; });

  function click(i: number, e: MouseEvent) {
    if (readonly) { sel = [i]; return; }
    if (moving) {
      const m = moving;
      moving = null;
      if (moveSlot(p, m, i)) sel = [i]; else toast('Emplacement occupé ou trop étroit.', 'warn');
      return;
    }
    const s = slotAt(p, i);
    const at = s ? s.index : i;
    if (e.shiftKey && sel.length && rowOf(p, anchor) === rowOf(p, at)) {
      const [a, b] = [Math.min(anchor, at), Math.max(anchor, at)];
      sel = Array.from({ length: b - a + 1 }, (_, k) => a + k);
    } else { sel = [at]; anchor = at; }
    group = null;
  }

  function place(start = sel[0] ?? 1) {
    const proto = { connector: conn, variant, label: '', notes: null, width, function: fn, d_format: dfmt && dFormatAllowed(p, conn) };
    let n: number;
    if (range) {
      n = 0;
      for (let i = sel[0]!; i <= sel[sel.length - 1]!; i++) n += fill(p, i, 'one', proto);
    } else n = fill(p, start, mode, proto, count);
    if (!n) toast('Aucune place libre à partir de cet emplacement.', 'warn');
  }

  function removeSel() {
    const before = p.slots.length;
    p.slots = p.slots.filter((s) => !sel.some((i) => i >= s.index && i < s.index + Math.max(1, s.width)));
    if (before === p.slots.length) toast('Aucun connecteur dans la sélection.', 'warn');
  }

  function makeGroup() {
    const row = rowOf(p, sel[0]!), start = colOf(p, sel[0]!), len = sel.length;
    p.groups = p.groups.filter((g) => !(g.row === row && g.start < start + len && start < g.start + g.len));
    p.groups.push({ row, start, len, label: gLabel.trim(), function: gFn, color: familyColor(gFn, { screen: false }) });
    p.groups.sort((a, b) => a.row - b.row || a.start - b.start);
    gLabel = '';
  }

  function number() {
    let n = from;
    const inSel = p.slots.filter((s) => sel.includes(s.index));
    for (const s of inSel) s.label = `${prefix}${n++}`;
    if (!inSel.length) toast('Aucun connecteur à numéroter dans la sélection.', 'warn');
  }

  function askResize(h: number, per: number) {
    if (h === p.height_u && per === p.per_u) return;
    if (p.slots.length || p.groups.length) resize = { h, per };
    else { p.height_u = h; p.per_u = per; sel = []; }
  }
  function doResize() {
    if (!resize) return;
    p.height_u = resize.h; p.per_u = resize.per; p.slots = []; p.groups = []; sel = []; group = null;
    if (p.per_u > D_FORMAT_MAX_PER_U) dfmt = false;
    resize = null;
  }

  async function save() {
    if (!onsave) return;
    saving = true; errs = [];
    try {
      const r = await onsave($state.snapshot(p) as PanelData);
      if (r) { p = clonePanel(r); }
      base = JSON.stringify(p);
    } catch (e) {
      if (e instanceof ApiError && e.fields?.length) errs = e.fields;
      else throw e;
    } finally { saving = false; }
  }
  const err = (f: string) => errs.find((e) => e.field === f || e.field.startsWith(`${f}.`))?.message;
  export function isDirty() { return dirty; }

  // Raccourcis du canevas : Échap désélectionne (et annule un déplacement), Suppr retire la sélection.
  $effect(() => canvasKeys({
    enabled: () => !readonly,
    onEscape: () => { moving = null; sel = []; group = null; },
    onDelete: () => { if (sel.length) removeSel(); },
  }));
</script>

<div class="pe">
  <section class="pe-head card">
    {#if readonly}
      <div class="pe-title"><b>{p.name}</b><span class="muted small">{p.height_u} U · {p.per_u} par U · câblage arrière {p.rear_depth_mm} mm</span></div>
    {:else}
      <div class="pe-grid">
        <div class="field" style="grid-column:span 2"><label for="pe-n">Nom</label><input class="input" class:err={!!err('name')} id="pe-n" bind:value={p.name} maxlength="100" placeholder="Patch vidéo 16 BNC" aria-invalid={err('name') ? 'true' : undefined} aria-describedby={err('name') ? 'pe-n-err' : undefined}><FieldError message={err('name')} for="pe-n" /></div>
        <div class="field"><label for="pe-h">Hauteur</label><select class="input" id="pe-h" value={p.height_u} onchange={(e) => askResize(Number((e.target as HTMLSelectElement).value), p.per_u)}>{#each [1, 2, 3, 4] as u}<option value={u}>{u} U</option>{/each}</select></div>
        <div class="field"><label for="pe-d">Emplacements par U</label><select class="input" id="pe-d" value={p.per_u} onchange={(e) => askResize(p.height_u, Number((e.target as HTMLSelectElement).value))}>{#each PER_U as n}<option value={n}>{n}</option>{/each}</select></div>
        <NumField id="pe-r" label="Câblage arrière" unit="mm" min={30} max={300} step={5} error={err('rear_depth_mm')} bind:value={() => p.rear_depth_mm, (v) => (p.rear_depth_mm = v ?? 80)} />
        <div class="field" style="grid-column:span 3"><label for="pe-no">Notes (facultatif)</label><input class="input" id="pe-no" value={p.notes ?? ''} oninput={(e) => (p.notes = (e.target as HTMLInputElement).value || null)}></div>
      </div>
    {/if}
    <div class="pe-stats">
      <div><span class="flabel">Occupés</span><b class="mono">{st.used}</b></div>
      <div><span class="flabel">Libres</span><b class="mono">{st.free}</b></div>
      <div><span class="flabel">Remplissage</span><b class="mono">{st.fill_pct} %</b></div>
      <div class="pe-bar" aria-hidden="true"><i style="width:{st.fill_pct}%"></i></div>
    </div>
  </section>

  <div class="pe-face">
    <PanelFace panel={p} detail selected={sel} target={null} onslot={click} ongroup={(g) => { if (!readonly) { group = g; sel = []; } }} />
    <p class="muted small pe-hint">{#if moving}<Icon name="hand" size="sm" /> Cliquez l’emplacement de destination · <button class="ilink" onclick={() => (moving = null)}>annuler (Échap)</button>{:else if !readonly}Cliquez un emplacement · Maj + clic pour sélectionner une plage dans la rangée · cliquez un bandeau pour modifier un groupe{/if}{#if mix}&nbsp;· {mix}{/if}</p>
  </div>

  {#if !readonly}
    <div class="pe-tools">
      {#if group}
        {@const g = group}
        <section class="card pe-card">
          <span class="flabel">Groupe · rangée {g.row} · ports {g.start} à {g.start + g.len - 1}</span>
          <div class="field"><label for="pe-gl">Nom du groupe</label><input class="input" id="pe-gl" bind:value={g.label} maxlength="40"></div>
          <div class="field"><span class="flabel" id="pe-gc-l">Famille de signal</span><div class="swatches" role="group" aria-labelledby="pe-gc-l">{#each PALETTE as c}<button class="sw" class:on={groupFamily(g) === c.key} aria-pressed={groupFamily(g) === c.key} style="--s:{c.color}" title={c.name} aria-label={c.name} onclick={() => { g.function = c.key; g.color = c.color; }}></button>{/each}</div></div>
          <div class="row-actions"><button class="btn ghost danger" onclick={() => { p.groups = p.groups.filter((x) => x !== g); group = null; }}><Icon name="trash" size="sm" />Supprimer le groupe</button><button class="btn" onclick={() => (group = null)}>Terminé</button></div>
        </section>
      {:else if one}
        {@const s = one}
        <section class="card pe-card">
          <span class="flabel">Emplacement {s.index}{s.width > 1 ? `–${s.index + s.width - 1}` : ''} · {refById('connectors', s.connector)?.name ?? s.connector}</span>
          <div class="fg2">
            <div class="field"><label for="pe-sl">Libellé</label><input class="input" id="pe-sl" bind:value={s.label} maxlength="40" placeholder="CAM 1"></div>
            <div class="field"><label for="pe-sf">Famille de signal</label><select class="input" id="pe-sf" bind:value={s.function}><option value={null}>Celle du groupe</option>{#each fnsOf(s.connector) as k}<option value={k}>{refById('functions', k)?.name ?? k}</option>{/each}</select></div>
            {#if (info(s.connector).variants ?? []).length}<div class="field"><label for="pe-sv">Variante</label><select class="input" id="pe-sv" bind:value={s.variant}><option value={null}>—</option>{#each info(s.connector).variants as v}<option value={v}>{v}</option>{/each}</select></div>{/if}
            <div class="field"><label for="pe-sw">Largeur</label><select class="input" id="pe-sw" value={s.width} onchange={(e) => { const w = Number((e.target as HTMLSelectElement).value); const was = s.width; s.width = w; if (!p.slots.every((x) => x === s || !(s.index < x.index + x.width && x.index < s.index + w)) || colOf(p, s.index) + w - 1 > p.per_u) { s.width = was; toast('Pas assez de place à droite.', 'warn'); } }}>{#each [1, 2, 3, 4] as w}<option value={w}>{w} emplacement{w > 1 ? 's' : ''}</option>{/each}</select></div>
          </div>
          <label class="toggle-row"><input type="checkbox" bind:checked={s.d_format} disabled={!dFormatAllowed(p, s.connector)}>Format D (coque standard){#if !dFormatAllowed(p, s.connector)}<span class="muted small">{' · '}{p.per_u > D_FORMAT_MAX_PER_U ? 'densité trop forte' : 'non disponible pour ce connecteur'}</span>{/if}</label>
          <div class="field"><label for="pe-sn">Notes (facultatif)</label><input class="input" id="pe-sn" value={s.notes ?? ''} oninput={(e) => (s.notes = (e.target as HTMLInputElement).value || null)}></div>
          <div class="row-actions"><button class="btn ghost danger" onclick={removeSel}><Icon name="trash" size="sm" />Retirer</button><button class="btn" onclick={() => (moving = s)}><Icon name="hand" size="sm" />Déplacer</button></div>
        </section>
      {/if}

      {#if !group && (!one || range)}
        <section class="card pe-card">
          <span class="flabel">{range ? `Sélection · ${sel.length} emplacements` : sel.length ? `Poser à partir de l’emplacement ${sel[0]}` : 'Poser des connecteurs'}</span>
          <div class="fg2">
            <div class="field"><label for="pe-c">Connecteur</label><select class="input" id="pe-c" bind:value={conn}>{#each connectors as c}<option value={c.id}>{c.name}</option>{/each}</select></div>
            {#if (info(conn).variants ?? []).length}<div class="field"><label for="pe-v">Variante</label><select class="input" id="pe-v" bind:value={variant}><option value={null}>—</option>{#each info(conn).variants as v}<option value={v}>{v}</option>{/each}</select></div>{/if}
            {#if fnsOf(conn).length > 1}<div class="field"><label for="pe-f">Famille de signal</label><select class="input" id="pe-f" bind:value={fn}><option value={null}>Celle du groupe</option>{#each fnsOf(conn) as k}<option value={k}>{refById('functions', k)?.name ?? k}</option>{/each}</select></div>{/if}
            <div class="field"><label for="pe-w">Largeur</label><select class="input" id="pe-w" bind:value={width}>{#each [1, 2, 3, 4] as w}<option value={w}>{w} emplacement{w > 1 ? 's' : ''}</option>{/each}</select></div>
          </div>
          <label class="toggle-row"><input type="checkbox" bind:checked={dfmt} disabled={!dFormatAllowed(p, conn)}>Format D{#if !dFormatAllowed(p, conn)}<span class="muted small">{' · '}{p.per_u > D_FORMAT_MAX_PER_U ? 'densité trop forte' : 'non disponible'}</span>{/if}</label>
          {#if !range}
            <div class="pe-mode"><Seg label="Mode de pose" bind:value={mode} options={[{ value: 'one', label: 'Unique' }, { value: 'row', label: 'Reste de la rangée' }, { value: 'all', label: 'Tout le panel' }, { value: 'count', label: 'Nombre' }]} /></div>
            {#if mode === 'count'}<div style="max-width:160px"><NumField id="pe-k" label="Nombre" min={1} max={total(p)} step={1} bind:value={() => count, (v) => (count = Math.max(1, Math.round(v ?? 1)))} /></div>{/if}
          {/if}
          <div class="row-actions">
            {#if range}<button class="btn ghost danger" onclick={removeSel}><Icon name="trash" size="sm" />Retirer</button>{/if}
            <button class="btn primary" onclick={() => place()}><Icon name="plus" size="sm" />{range ? 'Poser sur la sélection' : 'Poser'}</button>
          </div>
        </section>
        {#if range}
          <section class="card pe-card">
            <span class="flabel">Étiquetage de la sélection</span>
            <div class="fg2">
              <div class="field"><label for="pe-gn">Groupe</label><input class="input" id="pe-gn" bind:value={gLabel} maxlength="40" placeholder="Caméras"></div>
              <div class="field"><span class="flabel" id="pe-gf-l">Famille de signal</span><div class="swatches" role="group" aria-labelledby="pe-gf-l">{#each PALETTE as c}<button class="sw" class:on={gFn === c.key} aria-pressed={gFn === c.key} style="--s:{c.color}" title={c.name} aria-label={c.name} onclick={() => (gFn = c.key)}></button>{/each}</div></div>
            </div>
            <div class="row-actions"><button class="btn" onclick={makeGroup}><Icon name="layers" size="sm" />Créer le groupe</button></div>
            <div class="fg2">
              <div class="field"><label for="pe-px">Préfixe</label><input class="input mono" id="pe-px" bind:value={prefix} maxlength="30" placeholder="CAM "></div>
              <NumField id="pe-fr" label="À partir de" min={0} step={1} bind:value={() => from, (v) => (from = Math.max(0, Math.round(v ?? 1)))} />
            </div>
            <div class="row-actions"><button class="btn" onclick={number}><Icon name="hash" size="sm" />Numéroter les ports</button></div>
          </section>
        {/if}
      {/if}
    </div>
    <div class="pe-foot">
      <button class="btn ghost danger" disabled={!p.slots.length} onclick={() => { p.slots = []; sel = []; }}><Icon name="trash" size="sm" />Vider le panel</button>
      {#if errs.length}<span class="help err" role="alert">{errs[0]?.message}</span>{/if}
      <span style="flex:1"></span>
      {@render actions?.()}
      {#if onsave}<button class="btn ghost" disabled={!dirty || saving} onclick={() => { p = clonePanel(JSON.parse(base)); sel = []; errs = []; }}>Annuler les modifications</button>
        <button class="btn primary" disabled={!dirty || saving} onclick={save}>{#if saving}<span class="spin" aria-hidden="true"></span>{/if}Enregistrer</button>{/if}
    </div>
  {/if}
</div>

{#if resize}
  <Confirm title="Changer la taille du panel ?" confirmLabel="Changer et vider" danger onconfirm={doResize} onclose={() => (resize = null)}>
    <p>Passer à {resize.h} U et {resize.per} emplacements par U efface les {p.slots.length} connecteurs et les {p.groups.length} groupes du panel.</p>
  </Confirm>
{/if}

<style>
  .pe { display: grid; gap: 16px; }
  .pe-head { display: grid; gap: 16px; }
  .pe-title { display: flex; flex-direction: column; gap: 2px; }
  .pe-title b { font: 650 var(--fs-lg) var(--display); }
  .pe-grid { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 14px 16px; }
  .pe-stats { display: flex; gap: 22px; align-items: flex-end; flex-wrap: wrap; }
  .pe-stats > div:not(.pe-bar) { display: flex; flex-direction: column; gap: 2px; }
  .pe-stats b { font-size: var(--fs-lg); }
  .pe-bar { flex: 1; min-width: 120px; height: 8px; border-radius: 99px; background: color-mix(in srgb, var(--fg) 10%, transparent); overflow: hidden; align-self: center; }
  .pe-bar i { display: block; height: 100%; background: var(--ok); border-radius: inherit; transition: width .3s; }
  .pe-face { padding: 22px 16px 10px; border-radius: 18px; background: radial-gradient(120% 140% at 50% 0%, #2B3038 0%, #16191D 70%); overflow-x: auto; }
  .pe-face :global(.pface) { min-width: 640px; }
  .pe-hint { color: #A4ADB7; margin: 10px 4px 0; display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
  .pe-hint .ilink { color: #fff; }
  .pe-tools { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 16px; align-items: start; }
  .pe-card { display: grid; gap: 12px; }
  .pe-card .toggle-row { flex-wrap: wrap; }
  .pe-mode { max-width: 100%; overflow-x: auto; }
  .swatches { display: flex; gap: 6px; flex-wrap: wrap; padding-top: 4px; }
  .sw { width: 24px; height: 24px; border-radius: 7px; background: var(--s); border: 2px solid transparent; box-shadow: inset 0 0 0 1px rgba(0, 0, 0, .15); cursor: pointer; }
  .sw.on { border-color: var(--fg); }
  .pe-foot { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; position: sticky; bottom: 0; padding: 12px 0; background: linear-gradient(transparent, var(--bg) 30%); }
  @media (max-width: 860px) { .pe-grid { grid-template-columns: 1fr 1fr; } .pe-grid > :first-child, .pe-grid > :last-child { grid-column: 1 / -1 !important; } }
</style>
