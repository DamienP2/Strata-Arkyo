<script lang="ts">
  import { registerLeaveGuard, route, navigate } from '../../lib/router.svelte';
  // Mode tableau des câbles (section 10.9) : tous les câbles du projet, filtrables, avec les
  // deux extrémités et le signal. Saisie en série des numéros, couleurs, notes et longueurs,
  // recopie vers le bas, enregistrement groupé ; écran partagé en direct.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import { api, download } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { nf, pl } from '../../lib/format';
  import { loadRefs, fnColor, refList, refById } from '../../lib/catalog.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import AnomalyGroups from '../../components/AnomalyGroups.svelte';
  import type { Anomaly } from '../../lib/anomalies';
  import type { ProjectView } from '../../lib/projects';

  interface End { port_ref: string; element: string; container_id: string | null; location: string; location_id: string | null; equipment: string; tag: string; port: string; connector: string; missing: boolean }
  interface Row { id: string; rev: number; number: string | null; color: string | null; notes: string | null; length_m: number | null; estimate_m: number | null; signal: string; family: string | null; a: End; b: End; warnings: string[] }
  type Field = 'number' | 'notes' | 'length_m';

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
  // Arrivée depuis une anomalie (vue d'ensemble, validation) : ?cable=id1,id2 ne montre qu'eux.
  const only = $derived(new Set(route.query.get('cable')?.split(',').filter(Boolean) ?? []));
  const showAllCables = () => navigate(`/projets/${pid}/cables`);
  interface Issue { severity: string; title: string; subject: string; place: string | null; detail: string; cables: string[] }
  let issues = $state<Issue[]>([]);
  async function loadIssues() { try { issues = (await api.get<{ issues: Issue[] }>(`/projects/${pid}/synoptic/validate`)).issues.filter((i) => i.cables.length && i.severity !== 'info'); } catch { issues = []; } }
  // Anomalies des câbles, regroupées par type ; un clic filtre la liste sur le câble.
  const anomalies = $derived<Anomaly<Issue>[]>(issues.map((i) => ({ group: i.title, severity: i.severity, subject: i.subject, place: i.place, detail: i.detail, href: `/projets/${pid}/cables?cable=${i.cables.join(',')}`, src: i })));
  const groupHref = (g: { items: Anomaly<Issue>[] }) => `/projets/${pid}/cables?cable=${[...new Set(g.items.flatMap((a) => a.src?.cables ?? []))].join(',')}`;

  async function load() {
    try {
      const r = await api.get<{ rows: Row[] }>(`/projects/${pid}/cables/table`);
      // Les saisies en cours sur une ligne modifiée ailleurs sont gardées, à valider.
      rows = r.rows;
      for (const id of Object.keys(edits)) if (!r.rows.some((x) => x.id === id)) delete edits[id];
      picked = new Set([...picked].filter((id) => r.rows.some((x) => x.id === id)));
    } catch (e) { fail(e); }
  }
  $effect(() => { view; untrack(() => { if (!saving) { load(); loadIssues(); } }); });

  const val = (r: Row, f: Field) => (f in (edits[r.id] ?? {}) ? edits[r.id]![f] : r[f]);
  function set(r: Row, f: Field, v: string | number | null) {
    const e = { ...(edits[r.id] ?? {}) };
    if ((v ?? null) === (r[f] ?? null) || (v === '' && r[f] == null)) delete e[f]; else e[f] = v === '' ? null : v;
    if (Object.keys(e).length) edits[r.id] = e; else delete edits[r.id];
  }
  const dirty = $derived(Object.keys(edits).length);
  // Changer de page avec des saisies non enregistrées : on demande.
  $effect(() => registerLeaveGuard(() => dirty > 0));
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const shown = $derived((rows ?? []).filter((r) => {
    if (only.size && !only.has(r.id)) return false;
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
    toast(`${targets.length} ligne${pl(targets.length)} remplie${pl(targets.length)} : pensez à enregistrer.`, 'info');
  }
  async function save() {
    if (!rows || !dirty) return;
    saving = true;
    try {
      const changes = Object.entries(edits).map(([id, e]) => ({ id, rev: rows!.find((r) => r.id === id)!.rev, ...e }));
      for (let i = 0; i < changes.length; i += 500) {
        const r = await api.post<{ updated: { id: string }[]; conflicts: { id: string; reason: string }[] }>(`/projects/${pid}/cables/bulk`, { changes: changes.slice(i, i + 500) });
        for (const u of r.updated) delete edits[u.id];
        if (r.conflicts.length) toast(`${r.conflicts.length} ligne${pl(r.conflicts.length)} modifiée${pl(r.conflicts.length)} entre-temps par quelqu’un d’autre : vos saisies sont gardées, vérifiez-les puis enregistrez à nouveau.`, 'warn');
        else toast(`${r.updated.length} câble${pl(r.updated.length)} enregistré${pl(r.updated.length)}.`);
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
    const h = kit.projectHeader(view.project, `Liste des câbles (${shown.length})`);
    // Largeurs selon le contenu (les indications ne servent qu'à répartir la place restante).
    kit.drawTable(doc, h, [{ label: 'N°', w: 14 }, { label: 'Signal', w: 18 }, { label: 'De', w: 90 }, { label: 'Vers', w: 90 }, { label: 'Long. (m)', w: 12, align: 'right' }, { label: 'Notes', w: 30 }],
      shown.map((r) => [String(val(r, 'number') ?? '—'), r.signal || 'Non défini', `${r.a.element} › ${r.a.equipment} ${r.a.tag} › ${r.a.port}`, `${r.b.element} › ${r.b.equipment} ${r.b.tag} › ${r.b.port}`, String(val(r, 'length_m') ?? r.estimate_m ?? ''), String(val(r, 'notes') ?? '')]), 'A4', true);
    kit.paginate(doc, 1, undefined, view.project.name);
    doc.save(`${kit.fileStem('Cables', view.project.name)}.pdf`);
  }

  // ----- Étiquettes (section 11.7) -----
  let lbl = $state<{ sheets: import('../../lib/pdfkit').LabelSheet[]; key: string; perEnd: boolean; skip: number; scope: 'all' | 'internal' | 'shown' } | null>(null);
  async function openLabels() {
    try { const r = await api.get<{ items: import('../../lib/pdfkit').LabelSheet[] }>('/print/label-sheets'); const act = r.items.filter((x) => x.active); lbl = { sheets: act, key: act[0]?.key ?? '', perEnd: true, skip: 0, scope: 'all' }; } catch (e) { fail(e); }
  }
  // Tous les câbles, le câblage interne des éléments (pré-câblé en atelier), ou la sélection des filtres.
  const internalRows = $derived((rows ?? []).filter((r) => r.a.container_id && r.a.container_id === r.b.container_id));
  const labelRows = $derived((lbl?.scope === 'shown' ? shown : lbl?.scope === 'internal' ? internalRows : (rows ?? [])).filter((r) => val(r, 'number')));
  async function printLabels() {
    if (!lbl) return;
    const sh = lbl.sheets.find((x) => x.key === lbl!.key); if (!sh) return;
    const sigColor = (r: Row) => { const sg = refList('signals').find((x) => x.name === r.signal) as any; return sg?.data?.color || (r.family ? fnColor(r.family) : null); };
    const { cableLabels } = await import('../../lib/labels');
    const kit = await import('../../lib/pdfkit');
    const blob = await cableLabels(labelRows.map((r) => ({ number: String(val(r, 'number')), color: r.family ? fnColor(r.family) : sigColor(r), a: r.a, b: r.b })), sh, { perEnd: lbl.perEnd, skip: lbl.skip });
    const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = `${kit.fileStem('Etiquettes', view.project.name)}.pdf`; a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 2000);
    lbl = null;
  }
</script>

<svelte:window onbeforeunload={(e) => { if (dirty > 0) e.preventDefault(); }} />

{#snippet end(x: End)}
  {#if x.missing}<td class="end gone" title="L’équipement ou le port n’existe plus."><span class="end-l"><b>Extrémité supprimée</b></span></td>
  {:else if !x.port}<td class="end" title="{x.equipment} · {x.location}"><span class="end-l"><b>{x.equipment}</b> <span class="muted">· {x.location}</span></span></td>
  {:else}<td class="end" title="{x.element} › {x.equipment}{x.tag ? ` ${x.tag}` : ''} › {x.port}{x.connector ? ` · ${x.connector}` : ''}"><span class="end-l"><b>{x.equipment}</b>{#if x.tag}{" "}<span class="mono muted">{x.tag}</span>{/if}{" "}<span class="muted">›</span>{" "}<b class="pt">{x.port}</b></span></td>{/if}
{/snippet}

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
  {#if only.size}
    <div class="alert info ct-only"><Icon name="info" /><span>Liste filtrée sur {only.size > 1 ? `les ${only.size} câbles concernés` : 'le câble concerné'} par une anomalie.</span><button class="btn sm" onclick={showAllCables}>Afficher tous les câbles</button></div>
  {:else if anomalies.length}
    <details class="card ct-issues"><summary><Icon name="alert" size="sm" />{anomalies.length} anomalie{anomalies.length > 1 ? 's' : ''} sur les câbles</summary><AnomalyGroups list={anomalies} {groupHref} groupLink="Filtrer la liste" /></details>
  {/if}
  <div class="ct-bar">
    <span class="muted small">{shown.length} / {rows.length} câble{rows.length > 1 ? 's' : ''}{picked.size ? ` · ${picked.size} choisi${picked.size > 1 ? 's' : ''}` : ''}</span>
    <span style="flex:1"></span>
    <button class="btn sm ghost" onclick={() => download(`/projects/${pid}/cables/export?format=csv`)}><Icon name="download" size="sm" />CSV</button>
    <button class="btn sm ghost" onclick={() => download(`/projects/${pid}/cables/export?format=xlsx`)}><Icon name="download" size="sm" />XLSX</button>
    <button class="btn sm ghost" onclick={pdf}><Icon name="download" size="sm" />PDF</button>
    <button class="btn sm ghost" onclick={openLabels}><Icon name="file" size="sm" />Étiquettes</button>
    {#if writable && (dirty || saving)}
      <span class="ct-save"><span class="muted small">{dirty} modification{dirty > 1 ? 's' : ''} non enregistrée{dirty > 1 ? 's' : ''}</span>
      <button class="btn sm" disabled={saving} onclick={cancel}>Annuler</button>
      <button class="btn sm primary" disabled={!dirty || saving} onclick={save}>{#if saving}<span class="spin"></span>{/if}Enregistrer</button></span>
    {/if}
  </div>

  <section class="card ct">
    <div class="tbl-wrap"><table class="tbl ct-tbl">
      <thead><tr>
        <th class="ck"><input type="checkbox" checked={allPicked} onchange={pickAll} aria-label="Tout choisir"></th>
        <th>N° {#if writable}<button class="fd" title="Recopier vers le bas en incrémentant" aria-label="Recopier les numéros vers le bas" onclick={() => fillDown('number')}>↓</button>{/if}</th>
        <th>Signal</th><th class="c-end">De</th><th class="c-end">Vers</th>
        <th class="num">Longueur (m) {#if writable}<button class="fd" title="Recopier vers le bas" aria-label="Recopier les longueurs vers le bas" onclick={() => fillDown('length_m')}>↓</button>{/if}</th>
        <th title="Couleur de la catégorie du signal (non modifiable)">Couleur</th>
        <th class="c-notes">Notes {#if writable}<button class="fd" title="Recopier vers le bas" aria-label="Recopier les notes vers le bas" onclick={() => fillDown('notes')}>↓</button>{/if}</th>
        <th></th>
      </tr></thead>
      <tbody>
        {#each shown as r (r.id)}
          {@const e = edits[r.id] ?? {}}
          <tr class:focus={focusRow === r.id} class:picked={picked.has(r.id)} onfocusin={() => (focusRow = r.id)} onclick={() => (focusRow = r.id)}>
            <td class="ck"><input type="checkbox" checked={picked.has(r.id)} onchange={() => togglePick(r.id)} aria-label="Choisir {r.number ?? 'ce câble'}"></td>
            <td><input class="cell mono" class:dirty={'number' in e} value={val(r, 'number') ?? ''} disabled={!writable} placeholder="non numéroté" maxlength="40" oninput={(x) => set(r, 'number', (x.target as HTMLInputElement).value.trim())} aria-label="Numéro"></td>
            <td><span class="sig"><i class="dot" style="--c:{r.family ? fnColor(r.family) : 'var(--fg-3)'}"></i>{r.signal || '—'}</span></td>
            {@render end(r.a)}{@render end(r.b)}
            <td class="num"><input class="cell mono num" class:dirty={'length_m' in e} type="number" min="0" step="0.5" value={val(r, 'length_m') ?? ''} disabled={!writable} placeholder={r.estimate_m ? `≈ ${nf(r.estimate_m, 1)}` : ''} oninput={(x) => { const v = (x.target as HTMLInputElement).value; set(r, 'length_m', v === '' ? null : Number(v)); }} aria-label="Longueur"></td>
            <td><span class="swatch-ro" style="background:{r.family ? fnColor(r.family) : '#8A919B'}" title={r.family ? (refById('functions', r.family)?.name ?? '') : 'Catégorie non définie'} aria-label="Couleur de la catégorie du signal"></span></td>
            <td class="c-notes"><input class="cell" class:dirty={'notes' in e} value={val(r, 'notes') ?? ''} disabled={!writable} maxlength="2000" oninput={(x) => set(r, 'notes', (x.target as HTMLInputElement).value)} aria-label="Notes" title={(val(r, 'notes') as string) || undefined}></td>
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
        <label class="toggle-row small"><input type="radio" bind:group={lbl.scope} value="all">Tous les câbles</label>
        <label class="toggle-row small"><input type="radio" bind:group={lbl.scope} value="internal">Câblage interne des éléments (pré-câblé en atelier)</label>
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
  .ct-tbl td { vertical-align: middle; padding-top: 3px; padding-bottom: 3px; height: 40px; box-sizing: border-box; }
  .ct-tbl th { white-space: nowrap; }
  .ct-save { display: inline-flex; gap: 8px; align-items: center; }
  .ct-tbl tr.focus td { background: color-mix(in srgb, var(--accent, #4F6BFF) 6%, transparent); }
  .ct-tbl tr.picked td { background: color-mix(in srgb, var(--accent, #4F6BFF) 10%, transparent); }
  .ck { width: 30px; }
  .cell { width: 100%; min-width: 110px; height: 30px; border-radius: 8px; border: 1px solid transparent; background: transparent; padding: 0 8px; font: inherit; color: inherit; }
  .cell:hover:not(:disabled), .cell:focus { border-color: var(--line); background: var(--surface); outline: none; }
  .cell.num { width: 90px; min-width: 0; text-align: right; }
  .cell.dirty, .swatch.dirty { box-shadow: inset 0 0 0 2px var(--accent, #4F6BFF); background: color-mix(in srgb, var(--accent, #4F6BFF) 8%, var(--surface)); }
  .swatch { display: block; width: 36px; height: 26px; padding: 2px; border-radius: 8px; border: 1px solid var(--line); background: var(--surface); cursor: pointer; }
  .c-end { width: 34%; }
  .end { display: table-cell; min-width: 200px; max-width: 0; }
  .end-l { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .end b { font-weight: 600; }
  .c-notes { width: 180px; }
  .c-notes .cell { min-width: 0; max-width: 220px; }
  .sig { white-space: nowrap; }
  .fd { margin-left: 4px; width: 22px; height: 22px; border-radius: 6px; border: 1px solid var(--line); background: var(--surface); cursor: pointer; font-size: 12px; line-height: 1; }
  .fd:hover { background: var(--surface-2); }
  .warn-i { color: var(--warn, #B78400); }
  .end.gone b { color: var(--warn, #B78400); }
  .ct-only { display: flex; gap: 10px; align-items: center; margin-bottom: 10px; }
  .ct-only span { flex: 1; }
  .ct-issues { padding: 10px 14px; margin-bottom: 12px; }
  .ct-issues summary { cursor: pointer; font-weight: 600; display: flex; gap: 8px; align-items: center; color: var(--warn, #B78400); margin-bottom: 4px; }
  .ct-issues[open] summary { margin-bottom: 10px; }
  .swatch-ro { display: inline-block; width: 28px; height: 18px; border-radius: 5px; vertical-align: middle; box-shadow: inset 0 0 0 1px rgba(0,0,0,.25); }
</style>
