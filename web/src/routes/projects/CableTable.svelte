<script lang="ts">
  import { registerLeaveGuard, route, navigate } from '../../lib/router.svelte';
  // Mode tableau des câbles (section 10.9) : tous les câbles du projet, filtrables et triables,
  // avec les deux extrémités et le signal. Saisie en série des numéros, notes et longueurs,
  // recopie vers le bas, enregistrement groupé ; écran partagé en direct. Sur téléphone, une
  // carte par câble (.tbl.stack).
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import { api, download } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { nf, pl } from '../../lib/format';
  import { loadRefs, familyColor, refById } from '../../lib/catalog.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Loading from '../../components/Loading.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Menu from '../../components/Menu.svelte';
  import SortTh, { type Sort } from '../../components/SortTh.svelte';
  import AnomalyGroups from '../../components/AnomalyGroups.svelte';
  import type { Anomaly } from '../../lib/anomalies';
  import { endLabel } from '../../lib/pv';
  import type { ProjectView } from '../../lib/projects';

  interface End { port_ref: string; element: string; container_id: string | null; location: string; location_id: string | null; equipment: string; tag: string; port: string; connector: string; face?: string | null; missing: boolean }
  interface Row { id: string; rev: number; number: string | null; color: string | null; notes: string | null; length_m: number | null; estimate_m: number | null; signal: string; family: string | null; a: End; b: End; warnings: string[] }
  type Field = 'number' | 'notes' | 'length_m';

  let { view }: { view: ProjectView } = $props();
  loadRefs();
  const pid = $derived(view.project.id);
  const writable = $derived(can('conception', 'ecriture'));
  let rows = $state<Row[] | null>(null);
  /** Modifications locales, par câble puis par champ. */
  let edits = $state<Record<string, Partial<Record<Field, string | number | null>>>>({});
  /** Câbles choisis (cases de la première colonne). */
  let sel = $state<Set<string>>(new Set());
  let q = $state('');
  let fam = $state<string>('');
  let element = $state('');
  let onlyMissing = $state(false);
  let onlyWarn = $state(false);
  let saving = $state(false);
  let exporting = $state(false);
  let focusRow = $state<string | null>(null);
  // Arrivée depuis une anomalie ou la palette : ?cable=id1,id2 ne montre qu'eux.
  const only = $derived(new Set(route.query.get('cable')?.split(',').filter(Boolean) ?? []));
  const showAllCables = () => navigate(`/projets/${pid}/cables`);
  // Un seul câble ciblé : sa ligne devient la ligne active.
  $effect(() => { if (only.size === 1) { const id = [...only][0]!; untrack(() => (focusRow = id)); } });

  const MISSING = 'Extrémité manquante';
  /** Extrémité en clair : « Équipement ID › port (face d'un panel) », ou « Extrémité manquante » (lib/pv, comme le PV). */
  const endText = (x: End, withElement = false) => (x.missing ? MISSING : endLabel({ ...x, element: withElement ? x.element : '' }));
  /** Nom d'un câble : son numéro, à défaut ses deux extrémités (jamais un libellé générique). */
  const cableName = (r: Row) => (r.number ? r.number : `${endText(r.a)} → ${endText(r.b)}`);

  interface Issue { severity: string; title: string; subject: string; place: string | null; detail: string; cables: string[] }
  let issues = $state<Issue[]>([]);
  async function loadIssues() { try { issues = (await api.get<{ issues: Issue[] }>(`/projects/${pid}/synoptic/validate`)).issues.filter((i) => i.cables.length && i.severity !== 'info'); } catch { issues = []; } }
  const rowById = $derived(new Map((rows ?? []).map((r) => [r.id, r])));
  // Anomalies des câbles, regroupées par type ; un clic filtre la liste sur le câble. Un câble
  // non numéroté est nommé par ses extrémités plutôt que par « Câble non numéroté ».
  const anomalies = $derived<Anomaly<Issue>[]>(issues.map((i) => {
    const one = i.cables.length === 1 ? rowById.get(i.cables[0]!) : undefined;
    const named = one && !one.number && /non num[ée]rot[ée]/i.test(i.subject);
    return { group: i.title, severity: i.severity, subject: named ? cableName(one) : i.subject, place: named ? null : i.place, detail: i.detail, href: `/projets/${pid}/cables?cable=${i.cables.join(',')}`, src: i };
  }));
  const groupHref = (g: { items: Anomaly<Issue>[] }) => `/projets/${pid}/cables?cable=${[...new Set(g.items.flatMap((a) => a.src?.cables ?? []))].join(',')}`;

  async function load() {
    try {
      const r = await api.get<{ rows: Row[] }>(`/projects/${pid}/cables/table`);
      // Les saisies en cours sur une ligne modifiée ailleurs sont gardées, à valider.
      rows = r.rows;
      for (const id of Object.keys(edits)) if (!r.rows.some((x) => x.id === id)) delete edits[id];
      sel = new Set([...sel].filter((id) => r.rows.some((x) => x.id === id)));
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
  // Changer de page ou fermer l'onglet avec des saisies non enregistrées : on demande.
  $effect(() => registerLeaveGuard(() => dirty > 0));

  // ----- Tri (mémorisé dans ce navigateur) -----
  const SORT_KEY = 'strata.cables.sort';
  function readSort(): Sort {
    try { const s = JSON.parse(localStorage.getItem(SORT_KEY) ?? 'null'); if (s && typeof s.key === 'string' && (s.dir === 1 || s.dir === -1)) return s; } catch { /* stockage indisponible */ }
    return { key: '', dir: 1 };
  }
  let sort = $state<Sort>(readSort());
  $effect(() => { const s = JSON.stringify({ key: sort.key, dir: sort.dir }); try { localStorage.setItem(SORT_KEY, s); } catch { /* stockage indisponible */ } });
  const coll = new Intl.Collator('fr', { numeric: true, sensitivity: 'base' });
  // Le tri suit les valeurs enregistrées : une ligne ne saute pas pendant la saisie.
  function sortVal(r: Row): string | number | null {
    switch (sort.key) {
      case 'number': return r.number;
      case 'signal': return r.signal || null;
      case 'a': return r.a.missing ? null : endText(r.a);
      case 'b': return r.b.missing ? null : endText(r.b);
      case 'length': return r.length_m ?? r.estimate_m;
      default: return null;
    }
  }
  /** Valeurs vides toujours en fin de liste, quel que soit le sens. */
  function cmp(a: Row, b: Row): number {
    const x = sortVal(a), y = sortVal(b);
    const ex = x == null || x === '', ey = y == null || y === '';
    if (ex || ey) return ex && ey ? 0 : ex ? 1 : -1;
    return sort.dir * (typeof x === 'number' && typeof y === 'number' ? x - y : coll.compare(String(x), String(y)));
  }

  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const shown = $derived.by(() => {
    const list = (rows ?? []).filter((r) => {
      if (only.size && !only.has(r.id)) return false;
      if (onlyMissing && val(r, 'number')) return false;
      if (onlyWarn && !r.warnings.length) return false;
      if (fam && r.family !== fam) return false;
      if (element && r.a.container_id !== element && r.b.container_id !== element) return false;
      if (q) { const t = norm(q); const hay = norm([val(r, 'number') ?? '', r.signal, r.a.element, r.a.equipment, r.a.tag, r.a.port, r.b.element, r.b.equipment, r.b.tag, r.b.port, val(r, 'notes') ?? ''].join(' ')); if (!hay.includes(t)) return false; }
      return true;
    });
    return sort.key ? list.sort(cmp) : list;
  });
  const filtered = $derived(!!(q || fam || element || onlyMissing || onlyWarn));
  function clearFilters() { q = ''; fam = ''; element = ''; onlyMissing = false; onlyWarn = false; }
  const families = $derived([...new Set((rows ?? []).map((r) => r.family).filter(Boolean))] as string[]);
  const famName = (f: string | null) => (f ? refById('functions', f)?.name ?? f : 'Famille non définie');
  const elements = $derived(view.containers.filter((c) => (rows ?? []).some((r) => r.a.container_id === c.id || r.b.container_id === c.id)));
  const allSel = $derived(shown.length > 0 && shown.every((r) => sel.has(r.id)));

  // ----- Recopie vers le bas -----
  /** Numéro suivant : « V-0099 » → « V-0100 », en gardant la largeur. */
  function bump(s: string, k: number): string | null {
    const m = s.match(/^(.*?)(\d+)(\D*)$/);
    if (!m) return null;
    return `${m[1]}${String(Number(m[2]) + k).padStart(m[2]!.length, '0')}${m[3]}`;
  }
  const focusIdx = $derived(shown.findIndex((r) => r.id === focusRow));
  /** Lignes qui recevront la valeur de la ligne active : les lignes choisies en dessous, ou
   *  toutes les lignes affichées en dessous si rien n'est choisi. */
  const fillTargets = $derived(focusIdx < 0 ? [] : shown.slice(focusIdx + 1).filter((r) => !sel.size || sel.has(r.id)));
  const fillN = $derived(`${fillTargets.length} ligne${pl(fillTargets.length)}`);
  function fillDown(f: Field) {
    if (focusIdx < 0) return;
    const v = val(shown[focusIdx]!, f);
    const targets = fillTargets;
    if (f === 'number' && (typeof v !== 'string' || !bump(v, 1))) { toast('Le numéro de départ doit finir par des chiffres.', 'warn'); return; }
    let k = 1;
    for (const r of targets) set(r, f, f === 'number' ? bump(v as string, k++) : v ?? null);
    toast(`${targets.length} ligne${pl(targets.length)} remplie${pl(targets.length)} : pensez à enregistrer.`, 'info');
  }

  // ----- Enregistrement groupé -----
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
  /** Abandonner les saisies, avec Annuler dans la notification. */
  function cancel() {
    const prev = edits, n = dirty;
    edits = {};
    toast(`${n} modification${pl(n)} abandonnée${pl(n)}.`, 'ok', { run: () => { edits = { ...prev, ...edits }; } });
  }
  function selAll() { sel = allSel ? new Set() : new Set(shown.map((r) => r.id)); }
  function toggleSel(id: string) { const s = new Set(sel); if (s.has(id)) s.delete(id); else s.add(id); sel = s; }

  // ----- PDF -----
  async function pdf() {
    if (exporting) return;
    exporting = true;
    try {
      const kit = await import('../../lib/pdfkit');
      const doc = await kit.newDoc('A4', true);
      // Estimation marquée « ≈ » (« ~ » avec les polices de repli), comme à l'écran.
      const approx = kit.FONT.unicode ? '≈' : '~';
      let estimated = false;
      const len = (r: Row) => {
        const v = val(r, 'length_m');
        if (v != null && v !== '') return String(v).replace('.', ',');
        if (r.estimate_m == null) return '';
        estimated = true;
        return `${approx} ${nf(r.estimate_m, 1)}`;
      };
      // Extrémité manquante signalée et en gras : l'anomalie se voit aussi en noir et blanc.
      const end = (x: End) => (x.missing ? `⚠ ${MISSING}` : endText(x, true));
      const h = kit.projectHeader(view.project, `Liste des câbles (${shown.length})`);
      // Largeurs selon le contenu (les indications ne servent qu'à répartir la place restante).
      let y = kit.drawTable(doc, h, [{ label: 'N°', w: 14 }, { label: 'Signal', w: 18 }, { label: 'De', w: 90 }, { label: 'Vers', w: 90 }, { label: 'Long. (m)', w: 12, align: 'right' }, { label: 'Notes', w: 30 }],
        shown.map((r) => [String(val(r, 'number') || 'non numéroté'), r.signal || 'Non défini', end(r.a), end(r.b), len(r), String(val(r, 'notes') ?? '')]), 'A4', true,
        { bold: (ri, ci) => (ci === 2 && !!shown[ri]?.a.missing) || (ci === 3 && !!shown[ri]?.b.missing) });
      if (estimated) {
        const [, H] = kit.size(doc);
        if (y + 6 > H - 16) { kit.addPage(doc, 'A4', true); y = kit.drawHeader(doc, h, { cont: true }); }
        kit.ink(doc, 7.5);
        doc.text(kit.txt(`${approx} = longueur estimée depuis le plan, non mesurée.`), kit.MARGIN, y + 4);
      }
      kit.paginate(doc, 1, undefined, view.project.name);
      doc.save(`${kit.fileStem('Cables', view.project.name)}.pdf`);
      toast('Liste des câbles exportée.');
    } catch (e) { fail(e); }
    finally { exporting = false; }
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
    const { cableLabels } = await import('../../lib/labels');
    const kit = await import('../../lib/pdfkit');
    const blob = await cableLabels(labelRows.map((r) => ({ number: String(val(r, 'number')), color: familyColor(r.family, { screen: false }), a: r.a, b: r.b })), sh, { perEnd: lbl.perEnd, skip: lbl.skip });
    const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = `${kit.fileStem('Etiquettes', view.project.name)}.pdf`; a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 2000);
    toast('Étiquettes exportées.');
    lbl = null;
  }
</script>

{#snippet end(x: End, side: 'a' | 'b')}
  {#if x.missing}<td class="end gone c-{side}" data-l={side === 'a' ? 'De' : 'Vers'} title="L’équipement ou le port n’existe plus."><span class="end-l"><Icon name="alert" size="sm" /><b>{MISSING}</b></span></td>
  {:else if !x.port}<td class="end c-{side}" data-l={side === 'a' ? 'De' : 'Vers'} title="{x.equipment} · {x.location}"><span class="end-l"><b>{x.equipment}</b> <span class="muted">· {x.location}</span></span></td>
  {:else}<td class="end c-{side}" data-l={side === 'a' ? 'De' : 'Vers'} title="{x.element} › {x.equipment}{x.tag ? ` ${x.tag}` : ''} › {x.port}{x.face ? ` ${x.face}` : ''}{x.connector ? ` · ${x.connector}` : ''}"><span class="end-l"><b>{x.equipment}</b>{#if x.tag}{" "}<span class="mono muted">{x.tag}</span>{/if}{" "}<span class="muted">›</span>{" "}<b class="pt">{x.port}</b>{#if x.face}{" "}<span class="muted face">{x.face}</span>{/if}</span></td>{/if}
{/snippet}

{#if !rows}<Loading label="Chargement des câbles" />
{:else}
  <div class="ct-bar">
    <label class="search" style="flex:1;min-width:220px"><Icon name="search" /><input class="input" bind:value={q} placeholder="Numéro, équipement, port, notes" aria-label="Rechercher un câble"></label>
    <select class="input ct-sel" bind:value={element} aria-label="Élément"><option value="">Tous les éléments</option>{#each elements as c}<option value={c.id}>{c.name}</option>{/each}</select>
    <div class="seg" role="group" aria-label="Famille de signal">
      <button type="button" class:on={!fam} aria-pressed={!fam} onclick={() => (fam = '')}>Toutes</button>
      {#each families as f}<button type="button" class:on={fam === f} aria-pressed={fam === f} onclick={() => (fam = f)}><i class="dot" style="--c:{familyColor(f, { screen: true })}"></i>{famName(f)}</button>{/each}
    </div>
    <label class="toggle-row small"><input type="checkbox" bind:checked={onlyMissing}>Sans numéro</label>
    <label class="toggle-row small"><input type="checkbox" bind:checked={onlyWarn}>À vérifier</label>
  </div>
  {#if only.size}
    <div class="alert info ct-only"><Icon name="info" /><span>Liste filtrée sur {only.size > 1 ? `les ${only.size} câbles concernés` : 'le câble concerné'}.</span><button class="btn sm" onclick={showAllCables}>Afficher tous les câbles</button></div>
  {:else if anomalies.length}
    <details class="card compact ct-issues"><summary><Icon name="alert" size="sm" />{anomalies.length} anomalie{pl(anomalies.length)} sur les câbles</summary><AnomalyGroups list={anomalies} {groupHref} groupLink="Filtrer la liste" /></details>
  {/if}
  <div class="ct-bar">
    <span class="muted small">{shown.length} / {rows.length} câble{pl(rows.length)}{sel.size ? ` · ${sel.size} choisi${pl(sel.size)}` : ''}</span>
    {#if writable}
      <Menu label="Recopier vers le bas" icon="copy" btnClass="btn sm ghost" align="left" disabled={!fillTargets.length}
        title={focusIdx < 0 ? 'Cliquez d’abord une cellule de la ligne à recopier' : fillTargets.length ? `Recopier la ligne active sur ${fillN}` : 'Aucune ligne à remplir en dessous de la ligne active'}>
        {#snippet children()}
          <button type="button" role="menuitem" onclick={() => fillDown('number')}><Icon name="hash" size="sm" />Numéro, en incrémentant, sur {fillN}</button>
          <button type="button" role="menuitem" onclick={() => fillDown('length_m')}><Icon name="cable" size="sm" />Longueur sur {fillN}</button>
          <button type="button" role="menuitem" onclick={() => fillDown('notes')}><Icon name="text" size="sm" />Notes sur {fillN}</button>
        {/snippet}
      </Menu>
    {/if}
    <span style="flex:1"></span>
    <button class="btn sm ghost" title="Exporter la liste en CSV" onclick={() => download(`/projects/${pid}/cables/export?format=csv`)}><Icon name="download" size="sm" />CSV</button>
    <button class="btn sm ghost" title="Exporter la liste en Excel" onclick={() => download(`/projects/${pid}/cables/export?format=xlsx`)}><Icon name="download" size="sm" />Excel</button>
    <button class="btn sm ghost" title="Exporter les câbles affichés en PDF" disabled={exporting} onclick={pdf}>{#if exporting}<span class="spin" aria-hidden="true"></span>{:else}<Icon name="download" size="sm" />{/if}PDF</button>
    <button class="btn sm ghost" onclick={openLabels}><Icon name="printer" size="sm" />Étiquettes</button>
    {#if writable && (dirty || saving)}
      <span class="ct-save"><span class="muted small">{dirty} modification{pl(dirty)} non enregistrée{pl(dirty)}</span>
      <button class="btn sm ghost" disabled={saving} onclick={cancel}>Annuler</button>
      <button class="btn sm primary" disabled={!dirty || saving} onclick={save}>{#if saving}<span class="spin" aria-hidden="true"></span>{/if}Enregistrer</button></span>
    {/if}
  </div>

  <section class="card tbl-card dense">
    <div class="tbl-wrap"><table class="tbl dense stack ct-tbl">
      <thead><tr>
        <th class="ck"><input type="checkbox" checked={allSel} onchange={selAll} aria-label="Tout choisir"></th>
        <SortTh label="N°" key="number" bind:sort cls="c-num" />
        <SortTh label="Signal" key="signal" bind:sort />
        <SortTh label="De" key="a" bind:sort cls="c-end" />
        <SortTh label="Vers" key="b" bind:sort cls="c-end" />
        <SortTh label="Longueur (m)" key="length" bind:sort cls="num" />
        <th class="hide-m" title="Couleur de la famille du signal (non modifiable)">Couleur</th>
        <th class="c-notes">Notes</th>
        <th><span class="sr-only">Alertes</span></th>
      </tr></thead>
      <tbody>
        {#each shown as r (r.id)}
          {@const e = edits[r.id] ?? {}}
          {@const gone = r.a.missing || r.b.missing}
          <tr class:focus={focusRow === r.id} class:sel={sel.has(r.id)} onfocusin={() => (focusRow = r.id)} onclick={() => (focusRow = r.id)}>
            <td class="ck"><input type="checkbox" checked={sel.has(r.id)} onchange={() => toggleSel(r.id)} aria-label="Choisir le câble {cableName(r)}"></td>
            <td class="c-num"><input class="cell mono" class:dirty={'number' in e} value={val(r, 'number') ?? ''} disabled={!writable} placeholder="non numéroté" maxlength="40" oninput={(x) => set(r, 'number', (x.target as HTMLInputElement).value.trim())} aria-label="Numéro du câble {endText(r.a)} → {endText(r.b)}"></td>
            <td class="c-sig"><span class="sig"><i class="dot" style="--c:{familyColor(r.family, { screen: true })}"></i>{r.signal || 'Non défini'}</span></td>
            {@render end(r.a, 'a')}{@render end(r.b, 'b')}
            <td class="num c-len" data-l="Longueur (m)"><input class="cell mono" class:dirty={'length_m' in e} type="number" min="0" step="0.5" value={val(r, 'length_m') ?? ''} disabled={!writable} placeholder={r.estimate_m ? `≈ ${nf(r.estimate_m, 1)}` : ''} title={r.estimate_m && val(r, 'length_m') == null ? 'Longueur estimée depuis le plan' : undefined} oninput={(x) => { const v = (x.target as HTMLInputElement).value; set(r, 'length_m', v === '' ? null : Number(v)); }} aria-label="Longueur en mètres"></td>
            <td class="hide-m"><span class="swatch-ro" style="background:{familyColor(r.family, { screen: true })}" title={famName(r.family)} aria-label="Couleur de la famille du signal : {famName(r.family)}"></span></td>
            <td class="c-notes" data-l="Notes"><input class="cell" class:dirty={'notes' in e} value={val(r, 'notes') ?? ''} disabled={!writable} maxlength="2000" oninput={(x) => set(r, 'notes', (x.target as HTMLInputElement).value)} aria-label="Notes" title={(val(r, 'notes') as string) || undefined}></td>
            <td class="c-warn">{#if r.warnings.length}<span class="warn-i" class:err={gone} title={r.warnings.join(' ')} aria-label={r.warnings.join(' ')} role="img"><Icon name="alert" size="sm" /></span>{/if}</td>
          </tr>
        {:else}
          <tr><td colspan="9" class="c-empty">
            {#if rows.length && filtered}
              <EmptyState card={false} icon="search" message="Aucun câble ne correspond aux filtres.">
                {#snippet action()}<button class="btn" onclick={clearFilters}>Effacer les filtres</button>{/snippet}
              </EmptyState>
            {:else if rows.length}<EmptyState card={false} icon="search" message="Aucun câble ne correspond aux filtres." />
            {:else}
              <EmptyState card={false} icon="cable" message="Aucun câble" hint="Les câbles se tracent dans le synoptique.">
                {#snippet action()}<a class="btn" class:primary={writable} href="/projets/{pid}/synoptique"><Icon name="syn" size="sm" />Ouvrir le synoptique</a>{/snippet}
              </EmptyState>
            {/if}
          </td></tr>
        {/each}
      </tbody>
    </table></div>
  </section>
  {#if writable}<button type="button" class="ux3-info" title="Cliquez une cellule d’une ligne, puis « Recopier vers le bas » : la valeur part sur les lignes choisies en dessous (toutes les lignes affichées en dessous si aucune n’est choisie) ; les numéros s’incrémentent. Rien n’est enregistré avant « Enregistrer »." aria-label="Cliquez une cellule d’une ligne, puis « Recopier vers le bas » : la valeur part sur les lignes choisies en dessous (toutes les lignes affichées en dessous si aucune n’est choisie) ; les numéros s’incrémentent. Rien n’est enregistré avant « Enregistrer »."><Icon name="info" size="sm" /></button>{/if}
{/if}

{#if lbl}
  <Dialog title="Étiquettes de câbles" eyebrow="Impression" onclose={() => (lbl = null)} dirty={() => false}
    onsubmit={printLabels} submitLabel="Imprimer les étiquettes" cancelLabel={lbl.sheets.length ? 'Annuler' : 'Fermer'} submitDisabled={!lbl.sheets.length || !labelRows.length || !lbl.key}>
    {#if !lbl.sheets.length}<p class="muted" title="Ajoutez-en une dans Catalogue › Impression.">Aucune planche active</p>
    {:else}
      <div class="field"><label for="lbl-sheet">Planche</label><select class="input" id="lbl-sheet" bind:value={lbl.key}>{#each lbl.sheets as s}<option value={s.key}>{s.name} · {s.reference}{s.brand ? ` · ${s.brand}` : ''} ({s.cols} × {s.rows}{s.diam_min ? `, Ø ${s.diam_min}–${s.diam_max} mm` : ''})</option>{/each}</select></div>
      <div class="field" role="radiogroup" aria-labelledby="lbl-scope"><span class="flabel" id="lbl-scope">Câbles</span>
        <label class="toggle-row"><input type="radio" bind:group={lbl.scope} value="all">Tous les câbles</label>
        <label class="toggle-row"><input type="radio" bind:group={lbl.scope} value="internal">Câblage interne des éléments (pré-câblé en atelier)</label>
        <label class="toggle-row"><input type="radio" bind:group={lbl.scope} value="shown">Les câbles affichés par les filtres</label>
      </div>
      <label class="toggle-row"><input type="checkbox" bind:checked={lbl.perEnd}>Une étiquette par extrémité (deux par câble)</label>
      <div class="field"><label for="lbl-skip">Emplacements déjà utilisés sur la première planche</label><input class="input" id="lbl-skip" type="number" min="0" bind:value={lbl.skip}></div>
      <p class="small">{labelRows.length} câble{pl(labelRows.length)} numéroté{pl(labelRows.length)} · {labelRows.length * (lbl.perEnd ? 2 : 1)} étiquette{pl(labelRows.length * (lbl.perEnd ? 2 : 1))}. Les câbles sans numéro ne sont pas imprimés.</p>
    {/if}
  </Dialog>
{/if}

<style>
  .ct-bar { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; margin-bottom: 10px; }
  .ct-sel { width: auto; }
  .dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; background: var(--c); margin-right: 6px; vertical-align: 1px; flex: none; }
  .ct-tbl th { white-space: nowrap; }
  .ct-save { display: inline-flex; gap: 8px; align-items: center; }
  /* Largeurs des colonnes : l'apparence des cellules (.cell) est commune (harmonie.css). */
  .c-num { min-width: 120px; }
  .c-len .cell { width: 90px; text-align: right; }
  .c-end { width: 34%; }
  .end { display: table-cell; min-width: 200px; max-width: 0; }
  .end-l { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .end b { font-weight: 600; }
  .c-notes { width: 180px; }
  .c-notes .cell { max-width: 220px; }
  .sig { white-space: nowrap; }
  .warn-i { color: var(--warn); }
  .warn-i.err { color: var(--err); }
  .end.gone .end-l { color: var(--err); }
  .end.gone .end-l :global(.ic) { vertical-align: -2px; margin-right: 4px; }
  .ct-only { display: flex; gap: 10px; align-items: center; margin-bottom: 10px; }
  .ct-only span { flex: 1; }
  .ct-issues { margin-bottom: 12px; box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--warn) 40%, var(--line)), var(--card-sh); }
  .ct-issues summary { cursor: pointer; font-weight: 600; display: flex; gap: 8px; align-items: center; color: var(--warn); }
  .ct-issues[open] summary { margin-bottom: 10px; }
  .swatch-ro { display: inline-block; width: 28px; height: 18px; border-radius: 5px; vertical-align: middle; box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--fg) 25%, transparent); }
  /* Téléphone : une carte par câble (numéro et signal, De, Vers, puis longueur et notes). */
  @media (max-width: 860px) {
    .ct-tbl.stack tr { grid-template-columns: auto minmax(0, 1fr) auto auto; grid-template-areas: "ck num sig warn" "a a a a" "b b b b" "len notes notes notes"; gap: 6px 10px; padding: 12px 4px; }
    .ct-tbl.stack td { height: auto; padding: 0; min-width: 0; }
    .ct-tbl.stack td.ck { grid-area: ck; }
    .ct-tbl.stack td.c-num { grid-area: num; min-width: 0; }
    .ct-tbl.stack td.c-num .cell { width: 100%; min-width: 0; }
    .ct-tbl.stack td.c-sig { grid-area: sig; }
    .ct-tbl.stack td.c-warn { grid-area: warn; }
    .ct-tbl.stack td.c-a { grid-area: a; }
    .ct-tbl.stack td.c-b { grid-area: b; }
    .ct-tbl.stack td.c-len { grid-area: len; }
    .ct-tbl.stack td.c-notes { grid-area: notes; width: auto; }
    .ct-tbl.stack td.c-empty { grid-column: 1 / -1; }
    .ct-tbl.stack .end { display: flex; max-width: none; }
    .ct-tbl.stack td[data-l]::before { flex: none; min-width: 40px; }
    .ct-tbl.stack .end-l { flex: 1; min-width: 0; }
    .ct-tbl.stack .c-notes .cell { max-width: none; }
  }
</style>
