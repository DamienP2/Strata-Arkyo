<script lang="ts">
  // Mode tableau des câbles (section 10.9) : tous les câbles du projet, filtrables, avec les
  // deux extrémités et le signal. Saisie en série des numéros, couleurs, notes et longueurs,
  // recopie vers le bas, enregistrement groupé ; écran partagé en direct.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import { api, download } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { nf } from '../../lib/format';
  import { loadRefs, fnColor, refList } from '../../lib/catalog.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import type { ProjectView } from '../../lib/projects';

  interface End { port_ref: string; element: string; container_id: string | null; location: string; location_id: string | null; equipment: string; tag: string; port: string; connector: string }
  interface Row { id: string; rev: number; number: string | null; color: string | null; notes: string | null; length_m: number | null; estimate_m: number | null; signal: string; family: string | null; a: End; b: End; warnings: string[] }
  type Field = 'number' | 'color' | 'notes' | 'length_m';

  let { view }: { view: ProjectView } = $props();
  loadRefs();
  const pid = $derived(view.project.id);
  const writable = $derived(can('conception', 'ecriture'));
  let rows = $state<Row[] | null>(null);
  /** Modifications locales, par câble puis par champ. */
  let edits = $state<Record<string, Partial<Record<Field, string | number | null>>>>({});
  let picked = $state<Set<string>>(new Set());
  let q = $state('');
  let fam = $state<string>('');
  let element = $state('');
  let onlyMissing = $state(false);
  let onlyWarn = $state(false);
  let saving = $state(false);
  let focusRow = $state<string | null>(null);

  async function load() {
    try {
      const r = await api.get<{ rows: Row[] }>(`/projects/${pid}/cables/table`);
      // Les saisies en cours sur une ligne modifiée ailleurs sont gardées, à valider.
      rows = r.rows;
      for (const id of Object.keys(edits)) if (!r.rows.some((x) => x.id === id)) delete edits[id];
      picked = new Set([...picked].filter((id) => r.rows.some((x) => x.id === id)));
    } catch (e) { fail(e); }
  }
  $effect(() => { view; untrack(() => { if (!saving) load(); }); });

  const val = (r: Row, f: Field) => (f in (edits[r.id] ?? {}) ? edits[r.id]![f] : r[f]);
  function set(r: Row, f: Field, v: string | number | null) {
    const e = { ...(edits[r.id] ?? {}) };
    if ((v ?? null) === (r[f] ?? null) || (v === '' && r[f] == null)) delete e[f]; else e[f] = v === '' ? null : v;
    if (Object.keys(e).length) edits[r.id] = e; else delete edits[r.id];
  }
  const dirty = $derived(Object.keys(edits).length);
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const shown = $derived((rows ?? []).filter((r) => {
    if (onlyMissing && val(r, 'number')) return false;
    if (onlyWarn && !r.warnings.length) return false;
    if (fam && r.family !== fam) return false;
    if (element && r.a.container_id !== element && r.b.container_id !== element) return false;
    if (q) { const t = norm(q); const hay = norm([val(r, 'number') ?? '', r.signal, r.a.element, r.a.equipment, r.a.tag, r.a.port, r.b.element, r.b.equipment, r.b.tag, r.b.port, val(r, 'notes') ?? ''].join(' ')); if (!hay.includes(t)) return false; }
    return true;
  }));
  const families = $derived([...new Set((rows ?? []).map((r) => r.family).filter(Boolean))] as string[]);
  const elements = $derived(view.containers.filter((c) => (rows ?? []).some((r) => r.a.container_id === c.id || r.b.container_id === c.id)));
  const allPicked = $derived(shown.length > 0 && shown.every((r) => picked.has(r.id)));

  /** Numéro suivant : « V-0099 » → « V-0100 », en gardant la largeur. */
  function bump(s: string, k: number): string | null {
    const m = s.match(/^(.*?)(\d+)(\D*)$/);
    if (!m) return null;
    return `${m[1]}${String(Number(m[2]) + k).padStart(m[2]!.length, '0')}${m[3]}`;
  }
  /** Recopie vers le bas : la valeur de la ligne active part sur les lignes choisies en dessous
   *  (toutes les lignes affichées en dessous si rien n'est choisi). Les numéros s'incrémentent. */
  function fillDown(f: Field) {
    const start = shown.findIndex((r) => r.id === focusRow);
    if (start < 0) { toast('Cliquez d’abord une cellule de la ligne à recopier.', 'info'); return; }
    const from = shown[start]!;
    const v = val(from, f);
    const targets = shown.slice(start + 1).filter((r) => !picked.size || picked.has(r.id));
    let k = 1;
    for (const r of targets) {
      if (f === 'number') { const n = typeof v === 'string' ? bump(v, k++) : null; if (!n) { toast('Le numéro de départ doit finir par des chiffres.', 'warn'); return; } set(r, f, n); }
      else set(r, f, v ?? null);
    }
    toast(`${targets.length} ligne(s) remplie(s) : pensez à enregistrer.`, 'info');
  }
  async function save() {
    if (!rows || !dirty) return;
    saving = true;
    try {
      const changes = Object.entries(edits).map(([id, e]) => ({ id, rev: rows!.find((r) => r.id === id)!.rev, ...e }));
      for (let i = 0; i < changes.length; i += 500) {
        const r = await api.post<{ updated: { id: string }[]; conflicts: { id: string; reason: string }[] }>(`/projects/${pid}/cables/bulk`, { changes: changes.slice(i, i + 500) });
        for (const u of r.updated) delete edits[u.id];
        if (r.conflicts.length) toast(`${r.conflicts.length} ligne(s) modifiée(s) entre-temps par quelqu’un d’autre : vos saisies sont gardées, vérifiez-les puis enregistrez à nouveau.`, 'warn');
        else toast(`${r.updated.length} câble(s) enregistré(s).`);
      }
    } catch (e) { fail(e); }
    finally { saving = false; await load(); }
  }
  function cancel() { edits = {}; }
  function pickAll() { picked = allPicked ? new Set() : new Set(shown.map((r) => r.id)); }
  function togglePick(id: string) { const s = new Set(picked); if (s.has(id)) s.delete(id); else s.add(id); picked = s; }

  async function pdf() {
    const kit = await import('../../lib/pdfkit');
    const doc = await kit.newDoc('A4', true);
    const h = kit.projectHeader(view.project, `Câbles (${shown.length})`);
    kit.drawTable(doc, h, [{ label: 'N°', w: 24 }, { label: 'Signal', w: 22 }, { label: 'De', w: 90 }, { label: 'Vers', w: 90 }, { label: 'Long. (m)', w: 18, align: 'right' }, { label: 'Notes', w: 32 }],
      shown.map((r) => [String(val(r, 'number') ?? '—'), r.signal, `${r.a.element} › ${r.a.equipment} ${r.a.tag} › ${r.a.port}`, `${r.b.element} › ${r.b.equipment} ${r.b.tag} › ${r.b.port}`, String(val(r, 'length_m') ?? r.estimate_m ?? ''), String(val(r, 'notes') ?? '')]), 'A4', true);
    kit.paginate(doc, 1, undefined, view.project.name);
    doc.save(`${kit.fileStem('Cables', view.project.name)}.pdf`);
  }

  // ----- Étiquettes (section 11.7) -----
  let lbl = $state<{ sheets: import('../../lib/pdfkit').LabelSheet[]; key: string; perEnd: boolean; skip: number; scope: 'rule' | 'shown' } | null>(null);
  async function openLabels() {
    try { const r = await api.get<{ items: import('../../lib/pdfkit').LabelSheet[] }>('/print/label-sheets'); const act = r.items.filter((x) => x.active); lbl = { sheets: act, key: act[0]?.key ?? '', perEnd: true, skip: 0, scope: 'rule' }; } catch (e) { fail(e); }
  }
  // Règle : en intégration tous les câbles, en prestation le câblage interne des éléments.
  const ruleRows = $derived((rows ?? []).filter((r) => view.project.intervention === 'integration' || (r.a.container_id && r.a.container_id === r.b.container_id)));
  const labelRows = $derived((lbl?.scope === 'shown' ? shown : ruleRows).filter((r) => val(r, 'number')));
  async function printLabels() {
    if (!lbl) return;
    const sh = lbl.sheets.find((x) => x.key === lbl!.key); if (!sh) return;
    const sigColor = (r: Row) => { const sg = refList('signals').find((x) => x.name === r.signal) as any; return sg?.data?.color || (r.family ? fnColor(r.family) : null); };
    const { cableLabels } = await import('../../lib/labels');
    const kit = await import('../../lib/pdfkit');
    const blob = await cableLabels(labelRows.map((r) => ({ number: String(val(r, 'number')), color: (val(r, 'color') as string | null) || sigColor(r), a: r.a, b: r.b })), sh, { perEnd: lbl.perEnd, skip: lbl.skip });
    const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = `${kit.fileStem('Etiquettes', view.project.name)}.pdf`; a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 2000);
    lbl = null;
  }
</script>

<svelte:window onbeforeunload={(e) => { if (dirty > 0) e.preventDefault(); }} />

{#if !rows}<span class="spin"></span>
{:else}
  <div class="ct-bar">
    <label class="search" style="flex:1;min-width:220px"><Icon name="search" /><input class="input" bind:value={q} placeholder="Numéro, équipement, port, notes…" aria-label="Rechercher un câble"></label>
    <select class="input" bind:value={element} aria-label="Élément"><option value="">Tous les éléments</option>{#each elements as c}<option value={c.id}>{c.name}</option>{/each}</select>
    <div class="seg">
      <button class:on={!fam} onclick={() => (fam = '')}>Tous</button>
      {#each families as f}<button class:on={fam === f} onclick={() => (fam = f)}><i class="dot" style="--c:{fnColor(f)}"></i>{refList('functions').find((x) => x.id === f)?.name ?? f}</button>{/each}
    </div>
    <label class="toggle-row small"><input type="checkbox" bind:checked={onlyMissing}>Sans numéro</label>
    <label class="toggle-row small"><input type="checkbox" bind:checked={onlyWarn}>À vérifier</label>
  </div>
  <div class="ct-bar">
    <span class="muted small">{shown.length} / {rows.length} câble{rows.length > 1 ? 's' : ''}{picked.size ? ` · ${picked.size} choisi${picked.size > 1 ? 's' : ''}` : ''}</span>
    <span style="flex:1"></span>
    <button class="btn sm ghost" onclick={() => download(`/projects/${pid}/cables/export?format=csv`)}><Icon name="download" size="sm" />CSV</button>
    <button class="btn sm ghost" onclick={() => download(`/projects/${pid}/cables/export?format=xlsx`)}><Icon name="download" size="sm" />XLSX</button>
    <button class="btn sm ghost" onclick={pdf}><Icon name="download" size="sm" />PDF</button>
    <button class="btn sm ghost" onclick={openLabels}><Icon name="file" size="sm" />Étiquettes</button>
    {#if writable}
      <button class="btn sm" disabled={!dirty || saving} onclick={cancel}>Annuler</button>
      <button class="btn sm primary" disabled={!dirty || saving} onclick={save}>{#if saving}<span class="spin"></span>{/if}Enregistrer{dirty ? ` (${dirty})` : ''}</button>
    {/if}
  </div>

  <section class="card ct">
    <div class="tbl-wrap"><table class="tbl ct-tbl">
      <thead><tr>
        <th class="ck"><input type="checkbox" checked={allPicked} onchange={pickAll} aria-label="Tout choisir"></th>
        <th>N° {#if writable}<button class="fd" title="Recopier vers le bas en incrémentant" aria-label="Recopier les numéros vers le bas" onclick={() => fillDown('number')}>↓</button>{/if}</th>
        <th>Signal</th><th>De</th><th>Vers</th>
        <th class="num">Longueur {#if writable}<button class="fd" title="Recopier vers le bas" aria-label="Recopier les longueurs vers le bas" onclick={() => fillDown('length_m')}>↓</button>{/if}</th>
        <th>Couleur {#if writable}<button class="fd" title="Recopier vers le bas" aria-label="Recopier les couleurs vers le bas" onclick={() => fillDown('color')}>↓</button>{/if}</th>
        <th>Notes {#if writable}<button class="fd" title="Recopier vers le bas" aria-label="Recopier les notes vers le bas" onclick={() => fillDown('notes')}>↓</button>{/if}</th>
        <th></th>
      </tr></thead>
      <tbody>
        {#each shown as r (r.id)}
          {@const e = edits[r.id] ?? {}}
          <tr class:focus={focusRow === r.id} class:picked={picked.has(r.id)} onfocusin={() => (focusRow = r.id)} onclick={() => (focusRow = r.id)}>
            <td class="ck"><input type="checkbox" checked={picked.has(r.id)} onchange={() => togglePick(r.id)} aria-label="Choisir {r.number ?? 'ce câble'}"></td>
            <td><input class="cell mono" class:dirty={'number' in e} value={val(r, 'number') ?? ''} disabled={!writable} placeholder="—" maxlength="40" oninput={(x) => set(r, 'number', (x.target as HTMLInputElement).value.trim())} aria-label="Numéro"></td>
            <td><span class="sig"><i class="dot" style="--c:{r.family ? fnColor(r.family) : 'var(--fg-3)'}"></i>{r.signal || '—'}</span></td>
            <td class="end"><b>{r.a.equipment} <span class="mono muted">{r.a.tag}</span></b><span class="muted small">{r.a.element} › <b class="pt">{r.a.port}</b> · {r.a.connector}</span></td>
            <td class="end"><b>{r.b.equipment} <span class="mono muted">{r.b.tag}</span></b><span class="muted small">{r.b.element} › <b class="pt">{r.b.port}</b> · {r.b.connector}</span></td>
            <td class="num"><input class="cell mono num" class:dirty={'length_m' in e} type="number" min="0" step="0.5" value={val(r, 'length_m') ?? ''} disabled={!writable} placeholder={r.estimate_m ? `≈ ${nf(r.estimate_m, 1)}` : ''} oninput={(x) => { const v = (x.target as HTMLInputElement).value; set(r, 'length_m', v === '' ? null : Number(v)); }} aria-label="Longueur"></td>
            <td><input class="swatch" class:dirty={'color' in e} type="color" value={(val(r, 'color') as string) ?? (r.family ? fnColor(r.family) : '#8A919B')} disabled={!writable} oninput={(x) => set(r, 'color', (x.target as HTMLInputElement).value)} aria-label="Couleur"></td>
            <td><input class="cell" class:dirty={'notes' in e} value={val(r, 'notes') ?? ''} disabled={!writable} maxlength="2000" oninput={(x) => set(r, 'notes', (x.target as HTMLInputElement).value)} aria-label="Notes"></td>
            <td>{#if r.warnings.length}<span class="warn-i" title={r.warnings.join(' ')}><Icon name="alert" size="sm" /></span>{/if}</td>
          </tr>
        {:else}<tr><td colspan="9" class="empty">{rows.length ? 'Aucun câble ne correspond aux filtres.' : 'Aucun câble : tracez-les dans le synoptique.'}</td></tr>{/each}
      </tbody>
    </table></div>
  </section>
  {#if writable}<p class="muted small">Cliquez une ligne puis ↓ dans l’en-tête d’une colonne : la valeur part sur les lignes choisies en dessous (toutes les lignes affichées en dessous si aucune n’est choisie) ; les numéros s’incrémentent. Rien n’est enregistré avant « Enregistrer ».</p>{/if}
{/if}

{#if lbl}
  <Dialog title="Étiquettes de câbles" eyebrow="Impression" onclose={() => (lbl = null)}>
    {#if !lbl.sheets.length}<p class="muted">Aucune planche active : ajoutez-en une dans Catalogue › Impression.</p>
    {:else}
      <label class="field">Planche<select class="input" bind:value={lbl.key}>{#each lbl.sheets as s}<option value={s.key}>{s.name} · {s.reference}{s.brand ? ` · ${s.brand}` : ''} ({s.cols} × {s.rows}{s.diam_min ? `, Ø ${s.diam_min}–${s.diam_max} mm` : ''})</option>{/each}</select></label>
      <div class="field"><span class="flabel">Câbles</span>
        <label class="toggle-row small"><input type="radio" bind:group={lbl.scope} value="rule">{view.project.intervention === 'integration' ? 'Tous les câbles (intégration)' : 'Câblage interne des éléments (prestation)'}</label>
        <label class="toggle-row small"><input type="radio" bind:group={lbl.scope} value="shown">Les câbles affichés par les filtres</label>
      </div>
      <label class="toggle-row small"><input type="checkbox" bind:checked={lbl.perEnd}>Une étiquette par extrémité (deux par câble)</label>
      <label class="field">Emplacements déjà utilisés sur la première planche<input class="input" type="number" min="0" bind:value={lbl.skip}></label>
      <p class="small">{labelRows.length} câble{labelRows.length > 1 ? 's' : ''} numéroté{labelRows.length > 1 ? 's' : ''} · {labelRows.length * (lbl.perEnd ? 2 : 1)} étiquette{labelRows.length * (lbl.perEnd ? 2 : 1) > 1 ? 's' : ''}. Les câbles sans numéro ne sont pas imprimés.</p>
    {/if}
    {#snippet footer()}<button class="btn" onclick={() => (lbl = null)}>Annuler</button><button class="btn primary" disabled={!labelRows.length || !lbl?.key} onclick={printLabels}>Imprimer les étiquettes</button>{/snippet}
  </Dialog>
{/if}

<style>
  .ct-bar { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; margin-bottom: 10px; }
  .ct-bar select { width: auto; height: 40px; }
  .dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; background: var(--c); margin-right: 6px; vertical-align: 1px; }
  .ct { padding: 4px 8px 8px; }
  .ct-tbl td { vertical-align: middle; padding-top: 6px; padding-bottom: 6px; }
  .ct-tbl tr.focus td { background: color-mix(in srgb, var(--accent, #4F6BFF) 6%, transparent); }
  .ct-tbl tr.picked td { background: color-mix(in srgb, var(--accent, #4F6BFF) 10%, transparent); }
  .ck { width: 30px; }
  .cell { width: 100%; min-width: 90px; height: 34px; border-radius: 8px; border: 1px solid transparent; background: transparent; padding: 0 8px; font: inherit; color: inherit; }
  .cell:hover:not(:disabled), .cell:focus { border-color: var(--line); background: var(--surface); outline: none; }
  .cell.num { width: 90px; min-width: 0; text-align: right; }
  .cell.dirty, .swatch.dirty { box-shadow: inset 0 0 0 2px var(--accent, #4F6BFF); background: color-mix(in srgb, var(--accent, #4F6BFF) 8%, var(--surface)); }
  .swatch { width: 40px; height: 30px; padding: 2px; border-radius: 8px; border: 1px solid var(--line); background: var(--surface); cursor: pointer; }
  .end { display: table-cell; min-width: 220px; }
  .end b { display: block; font-weight: 600; }
  .end .pt { display: inline; }
  .sig { white-space: nowrap; }
  .fd { margin-left: 4px; width: 22px; height: 22px; border-radius: 6px; border: 1px solid var(--line); background: var(--surface); cursor: pointer; font-size: 12px; line-height: 1; }
  .fd:hover { background: var(--surface-2); }
  .warn-i { color: var(--warn, #B78400); }
</style>
