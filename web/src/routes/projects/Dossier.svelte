<script lang="ts">
  // Assistant de dossier (section 11.6) : départ (nouvelle composition, dernière exportation
  // ou modèle), composition, une étape par plan, récapitulatif, téléchargement. La dernière
  // exportation est mémorisée ; une composition peut devenir un modèle de dossier (6.4).
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Seg from '../../components/Seg.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { fileStem } from '../../lib/pdfkit';
  import { build, probe, FIXED, DEFAULT_FMT, COL_LABEL, GROUPS, type DocDef, type DocOpt, type Probe } from '../../lib/dossier';
  import type { ProjectView } from '../../lib/projects';

  interface DT { id: string; name: string; description: string | null; docs: DocOpt[] }
  interface CM { id: string; table: string; name: string; columns: string[] }
  let { view }: { view: ProjectView } = $props();
  const pid = $derived(view.project.id);
  const writable = $derived(can('projets', 'ecriture'));

  let defs = $state<DocDef[]>([]);
  let templates = $state<DT[]>([]);
  let colModels = $state<CM[]>([]);
  let last = $state<{ data: { docs?: DocOpt[]; title?: string } | null; rev: number }>({ data: null, rev: 0 });
  let docs = $state<DocOpt[]>([]);
  let title = $state('Dossier technique');
  let step = $state(0);
  let probes = $state<Record<string, Probe>>({});
  let probing: Promise<void> | null = null;
  let busy = $state<string | null>(null);
  let busyKey = $state<string | null>(null);
  let done = $state<{ pages: number; skipped: string[] } | null>(null);
  /** « Plans » désigne ailleurs la page du site : le groupe des dessins porte son vrai nom. */

  async function init() {
    try {
      const [d, t, c, l] = await Promise.all([api.get<{ docs: DocDef[] }>('/print/docs'), api.get<{ items: DT[] }>('/print/dossier-templates'), api.get<{ items: CM[] }>('/print/column-models'), api.get<typeof last>(`/projects/${pid}/settings/dossier`)]);
      defs = d.docs; templates = t.items; colModels = c.items; last = l;
      // Les sondes disent dès le départ quelles étapes de choix de pages existent : leur nombre ne change plus en route.
      probing = runProbes();
    } catch (e) { fail(e); }
  }
  init();

  const blank = (d: DocDef): DocOpt => ({ key: d.key, include: true, paper: null, landscape: null, columns: [...d.columns], hidden: [], title: null });
  function fresh(): DocOpt[] { return defs.map(blank); }
  /** Une composition reprise est complétée : documents nouveaux insérés, colonnes nouvelles cochées. */
  function complete(list: DocOpt[]): DocOpt[] {
    return defs.map((d) => { const o = list.find((x) => x.key === d.key); if (!o) return blank(d); const cols = o.columns.filter((c) => d.columns.includes(c)); const hid = (o.hidden ?? []).filter((c) => d.columns.includes(c) && !cols.includes(c)); for (const c of d.columns) if (!cols.includes(c) && !hid.includes(c)) cols.push(c); return { ...o, columns: cols, hidden: hid }; });
  }
  async function startWith(list: DocOpt[], t?: string) {
    docs = complete(list); if (t) title = t; step = 1; done = null;
    // Données relues au départ de chaque composition.
    probing = runProbes(); await probing;
    // Les pages des plans sont toutes retenues par défaut, sauf choix déjà fait.
    for (const o of docs) { const ch = probes[o.key]?.choices; if (ch && !o.pages) o.pages = ch.map((c) => c.id); }
  }
  /** Données lues pour la composition en cours (réutilisées par le récapitulatif). */
  let pctx: { view: ProjectView; cache: Map<string, any> } | null = null;
  async function runProbes() {
    pctx = { view, cache: new Map() };
    const out: Record<string, Probe> = {};
    await Promise.all(defs.map(async (d) => { out[d.key] = await probe(pctx as any, d.key); }));
    probes = out;
  }
  // Récapitulatif : chaque document à pages choisies est relu avec ses options (format, orientation,
  // pages retenues) ; le synoptique y donne le nombre exact de feuilles et leur orientation.
  let recap = $state<Record<string, Probe>>({});
  let recapRun = 0;
  $effect(() => {
    if (step !== steps.length - 1 || !docs.length) return;
    const list = included.filter((o) => o.pages && probes[o.key]?.state === 'ok').map((o) => { const f = fmt(o); return { ...$state.snapshot(o), paper: f[0], landscape: f[1] } as DocOpt; });
    const run = ++recapRun;
    untrack(async () => {
      const out: Record<string, Probe> = {};
      await Promise.all(list.map(async (o) => { out[o.key] = await probe((pctx ??= { view, cache: new Map() }) as any, o.key, o); }));
      if (run === recapRun) recap = out;
    });
  });

  // Étapes : Départ, Composition, une par plan à choix de pages (grisée si le document n'est pas retenu), Récapitulatif.
  const planDefs = $derived(defs.filter((d) => probes[d.key]?.choices && probes[d.key]!.state === 'ok'));
  const planOn = (k: string) => !!docs.find((o) => o.key === k)?.include;
  const steps = $derived([
    { label: 'Départ', on: true }, { label: 'Composition', on: true },
    ...planDefs.map((d) => ({ label: d.title, on: planOn(d.key) })),
    { label: 'Récapitulatif', on: true },
  ]);
  function go(d: 1 | -1) { let i = step + d; while (i > 1 && i < steps.length - 1 && !steps[i]!.on) i += d; step = i; }
  const fmt = (o: DocOpt) => FIXED[o.key] ?? [o.paper ?? DEFAULT_FMT[o.key]?.[0] ?? 'A4', o.landscape ?? DEFAULT_FMT[o.key]?.[1] ?? false];
  const def = (k: string) => defs.find((d) => d.key === k)!;
  const included = $derived(docs.filter((o) => o.include));
  /** Tout en A4, ou A3 pour les plans (formats par défaut) : un choix explicite, annoncé au sommaire. */
  const allA4 = $derived(included.every((o) => fmt(o)[0] === 'A4'));
  function setPlans(p: 'A4' | 'A3') { for (const o of docs) { if (FIXED[o.key]) continue; if (def(o.key)?.group === 'plans') o.paper = p; else if (p === 'A4') o.paper = 'A4'; } }
  /** Format annoncé au récapitulatif, dans les mots du sommaire du PDF. */
  function fmtText(o: DocOpt) {
    const f = fmt(o);
    if (o.key === 'cover') return 'A4 portrait';
    if (o.key === 'plan_synoptic') return `${f[0]}, orientation selon le dessin`;
    return `${f[0]} ${f[1] ? 'paysage' : 'portrait'}`;
  }
  function contentText(o: DocOpt, pr: Probe | undefined) {
    if (pr?.state === 'empty') return 'Vide : ignoré';
    if (pr?.state !== 'ok') return `En échec : ${pr?.message ?? ''}`;
    if (!o.pages) return pr.message;
    const n = o.pages.length;
    if (o.key === 'plan_synoptic') return recap[o.key]?.state === 'ok' ? recap[o.key]!.message : `${n} synoptique${n > 1 ? 's' : ''} (calcul des feuilles en cours)`;
    return `${n} page${n > 1 ? 's' : ''}`;
  }

  function toggleCol(o: DocOpt, c: string, on: boolean) { if (on) { o.columns = [...o.columns, c]; o.hidden = o.hidden.filter((x) => x !== c); } else { o.columns = o.columns.filter((x) => x !== c); o.hidden = [...o.hidden, c]; } }
  function moveCol(o: DocOpt, i: number, d: number) { const j = i + d; if (j < 0 || j >= o.columns.length) return; const l = [...o.columns]; [l[i], l[j]] = [l[j]!, l[i]!]; o.columns = l; }
  let colModel = $state<{ o: DocOpt; name: string } | null>(null);
  async function saveColModel() {
    if (!colModel?.name.trim()) return;
    const { o, name } = colModel;
    try { const r = await api.post<CM>('/print/column-models', { id: '', table: o.key, name: name.trim(), columns: o.columns }); colModels = [...colModels, r]; colModel = null; toast(`Modèle de colonnes « ${r.name} » enregistré.`); } catch (e) { fail(e); }
  }
  function applyColModel(o: DocOpt, id: string) { const m = colModels.find((x) => x.id === id); if (!m) return; const all = def(o.key).columns; o.columns = m.columns.filter((c) => all.includes(c)); o.hidden = all.filter((c) => !o.columns.includes(c)); }

  async function download(only?: string) {
    if (busy) return;
    busyKey = only ?? null;
    busy = only ? `Document ${def(only).title}` : 'Assemblage';
    try {
      const r = await build(view, defs, $state.snapshot(docs) as DocOpt[], title, (m) => (busy = m.replace(/…$/, '')), only);
      const a = document.createElement('a'); a.href = URL.createObjectURL(r.blob);
      a.download = `${fileStem(only ? (def(only).title.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^A-Za-z0-9]+/g, '-')) : 'Dossier', view.project.name)}.pdf`;
      a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 3000);
      if (!only) {
        done = { pages: r.pages, skipped: r.skipped };
        try { last = await api.put(`/projects/${pid}/settings/dossier`, { rev: last.rev, data: { docs: $state.snapshot(docs), title } }); } catch { /* mémorisation au prochain export */ }
        toast(`Dossier exporté : ${r.pages} pages.`);
      } else toast(`« ${def(only).title} » exporté.`);
    } catch (e) { fail(e); } finally { busy = null; busyKey = null; }
  }

  let saveTpl = $state<{ name: string; description: string } | null>(null);
  let tplErr = $state<ApiError | null>(null);
  async function saveAsTemplate() {
    if (!saveTpl) return;
    try { await api.post('/print/dossier-templates', { id: '', name: saveTpl.name, description: saveTpl.description || null, docs: $state.snapshot(docs).map(({ pages, ...o }: DocOpt) => { void pages; return o; }) }); toast(`Modèle de dossier « ${saveTpl.name} » enregistré.`); saveTpl = null; templates = (await api.get<{ items: DT[] }>('/print/dossier-templates')).items; } catch (e) { if (e instanceof ApiError && e.fields.length) tplErr = e; else fail(e); }
  }
</script>

<div class="dz">
  <ol class="dz-steps" aria-label="Étapes">
    {#each steps as s, i}<li class:on={i === step} class:done={i < step && s.on}>{#if i}<span class="sep" aria-hidden="true"></span>{/if}<button disabled={(i > 0 && !docs.length) || !s.on} title={s.on ? undefined : 'Ne s’applique pas : document non retenu dans la composition'} aria-current={i === step ? 'step' : undefined} onclick={() => (step = i)}><span class="n">{#if i < step && s.on}<Icon name="check" size="sm" />{:else}{i + 1}{/if}</span>{s.label}</button></li>{/each}
  </ol>

  {#if step === 0}
    <section class="card st0">
      <h2>Composer le dossier</h2>
      <button type="button" class="ux3-info" title="Un seul PDF : page de garde avec sommaire, documents à leur format, page de fin, pagination continue. Les données sont relues au moment de l’export." aria-label="Un seul PDF : page de garde avec sommaire, documents à leur format, page de fin, pagination continue. Les données sont relues au moment de l’export."><Icon name="info" size="sm" /></button>
      <div class="starts">
        <button class="card link start" onclick={() => startWith(fresh())}><Icon name="plus" /><b>Nouvelle composition</b><span class="muted small">Tous les documents, réglages par défaut.</span></button>
        <button class="card link start" disabled={!last.data?.docs} onclick={() => startWith(last.data!.docs!, last.data!.title)}><Icon name="refresh" /><b>Reprendre la dernière</b><span class="muted small">{last.data?.docs ? 'Composition du dernier export réussi.' : 'Aucun export réussi pour ce projet.'}</span></button>
        <div class="card start" class:off={!templates.length} aria-disabled={!templates.length}><Icon name="file" /><b id="dz-tpl-l">Modèle enregistré</b>
          {#if templates.length}<select class="input sm" aria-labelledby="dz-tpl-l" onchange={(e) => { const t = templates.find((x) => x.id === (e.target as HTMLSelectElement).value); if (t) startWith(t.docs, t.name); }}><option value="">Choisir un modèle</option>{#each templates as t}<option value={t.id}>{t.name}</option>{/each}</select>{:else}<span class="muted small">Aucun modèle de dossier.</span>{/if}
        </div>
      </div>
    </section>
  {:else if step === 1}
    <section class="card st1">
      <div class="field"><label for="dz-title">Titre du dossier</label><input class="input" id="dz-title" bind:value={title} maxlength="80"></div>
      <div class="fmtmode"><span class="flabel">Formats</span><Seg label="Formats du dossier" value={allA4 ? 'A4' : 'A3'} onchange={(v) => setPlans(v as 'A4' | 'A3')} options={[{ value: 'A4', label: 'Tout en A4' }, { value: 'A3', label: 'A3 pour les plans' }]} /><span class="muted small">{allA4 ? 'Un seul format : A4.' : 'Formats mélangés : A4 pour les listes, A3 pour les plans (indiqué au sommaire).'}</span></div>
      {#each Object.entries(GROUPS) as [g, gl]}
        <h3 class="eyebrow grp">{gl}</h3>
        {#each docs.filter((o) => def(o.key)?.group === g) as o (o.key)}
          {@const d = def(o.key)}{@const pr = probes[o.key]}{@const f = fmt(o)}
          <div class="doc" class:off={!o.include} class:empty={pr?.state === 'empty'} class:err={pr?.state === 'error'}>
            <label class="toggle-row inc"><input type="checkbox" bind:checked={o.include}><b>{d.title}</b></label>
            <span class="pr small">{#if !pr}<span class="spin sm" aria-hidden="true"></span><span class="sr-only">Lecture en cours</span>{:else if pr.state === 'ok'}<span class="muted">{pr.message}</span>{:else if pr.state === 'empty'}<span class="muted">Vide : ne sera pas imprimé</span>{:else}<span class="errt">En échec : {pr.message}</span>{/if}</span>
            {#if o.key !== 'cover'}
              <input class="input sm ttl" placeholder="Titre court" value={o.title ?? ''} maxlength="60" oninput={(e) => (o.title = (e.target as HTMLInputElement).value || null)} aria-label="Titre court de {d.title}">
              <select class="input sm" value={f[0]} disabled={!!FIXED[o.key]} onchange={(e) => (o.paper = (e.target as HTMLSelectElement).value as 'A4' | 'A3')} aria-label="Papier de {d.title}"><option value="A4">A4</option><option value="A3">A3</option></select>
              <select class="input sm" value={f[1] ? 'l' : 'p'} disabled={!!FIXED[o.key] || o.key === 'recette'} onchange={(e) => (o.landscape = (e.target as HTMLSelectElement).value === 'l')} aria-label="Orientation de {d.title}"><option value="p">Portrait</option><option value="l">Paysage</option></select>
            {/if}
            {#if d.columns.length && o.include}
              <div class="cols">
                {#each o.columns as c, i (c)}<span class="col on">{COL_LABEL[c] ?? c}<button class="x" aria-label="Avancer {COL_LABEL[c] ?? c}" title="Avancer" disabled={i === 0} onclick={() => moveCol(o, i, -1)}>‹</button><button class="x" aria-label="Reculer {COL_LABEL[c] ?? c}" title="Reculer" disabled={i === o.columns.length - 1} onclick={() => moveCol(o, i, 1)}>›</button><button class="x" aria-label="Retirer {COL_LABEL[c] ?? c}" title="Retirer" onclick={() => toggleCol(o, c, false)}>×</button></span>{/each}
                {#each d.columns.filter((c) => !o.columns.includes(c)) as c}<button class="col" onclick={() => toggleCol(o, c, true)}>+ {COL_LABEL[c] ?? c}</button>{/each}
                {#if colModels.some((m) => m.table === o.key)}<select class="input sm" onchange={(e) => applyColModel(o, (e.target as HTMLSelectElement).value)} aria-label="Modèle de colonnes de {d.title}"><option value="">Appliquer un modèle de colonnes</option>{#each colModels.filter((m) => m.table === o.key) as m}<option value={m.id}>{m.name}</option>{/each}</select>{/if}
                {#if writable}<button class="btn sm ghost" onclick={() => (colModel = { o, name: '' })}>Enregistrer ces colonnes</button>{/if}
              </div>
            {/if}
          </div>
        {/each}
      {/each}
      <div class="nav"><button class="btn ghost" onclick={() => (step = 0)}>Retour</button><span class="grow"></span><button class="btn primary" disabled={!included.length} onclick={() => go(1)}>Continuer</button></div>
    </section>
  {:else if step < steps.length - 1}
    {@const pd = planDefs[step - 2]}
    {@const o = pd ? docs.find((x) => x.key === pd.key) : undefined}
    {@const pr = pd ? probes[pd.key] : undefined}
    <section class="card st1">
      {#if pd && o && pr}
        <h2>{pd.title}</h2>
        <button type="button" class="ux3-info" title="Pages imprimées, dans cet ordre ; chacune reçoit le cartouche du site." aria-label="Pages imprimées, dans cet ordre ; chacune reçoit le cartouche du site."><Icon name="info" size="sm" /></button>
        <div class="pick">{#each pr.choices ?? [] as ch}<label class="toggle-row"><input type="checkbox" checked={o.pages?.includes(ch.id)} onchange={(e) => { const on = (e.target as HTMLInputElement).checked; o.pages = on ? [...(o.pages ?? []), ch.id] : (o.pages ?? []).filter((x) => x !== ch.id); }}>{ch.label}</label>{/each}</div>
      {/if}
      <div class="nav"><button class="btn ghost" onclick={() => go(-1)}>Retour</button><span class="grow"></span><button class="btn primary" onclick={() => go(1)}>Continuer</button></div>
    </section>
  {:else}
    <section class="card st1">
      <h2>Récapitulatif</h2>
      <div class="tbl-wrap"><table class="tbl rec"><thead><tr><th>Document</th><th>Format</th><th>Contenu</th><th><span class="sr-only">Actions</span></th></tr></thead><tbody>
        {#each included as o (o.key)}
          {@const pr = probes[o.key]}
          {@const name = o.title ?? def(o.key).title}
          <tr class:muted={pr?.state !== 'ok'}><td><b>{name}</b><span class="muted small">{' · '}{GROUPS[def(o.key).group]}</span></td><td>{fmtText(o)}</td>
            <td class="small">{contentText(o, pr)}</td>
            <td class="acts">{#if o.key !== 'cover' && pr?.state === 'ok'}<button class="btn sm ghost" disabled={!!busy} aria-label="Télécharger « {name} » seul" onclick={() => download(o.key)}>{#if busyKey === o.key}<span class="spin sm" aria-hidden="true"></span>{:else}<Icon name="download" size="sm" />{/if}Télécharger seul</button>{/if}</td></tr>
        {/each}
      </tbody></table></div>
      {#if done}<p class="ok-t" role="status">Dossier exporté : {done.pages} pages.{#if done.skipped.length} Ignorés : {done.skipped.join(' ; ')}.{/if}</p>{/if}
      <div class="nav">
        <button class="btn ghost" onclick={() => go(-1)}>Retour</button><span class="grow"></span>
        {#if writable}<button class="btn" onclick={() => { saveTpl = { name: title, description: '' }; tplErr = null; }}>Enregistrer comme modèle</button>{/if}
        <button class="btn primary" disabled={!!busy} onclick={() => download()}>{#if busy && !busyKey}<span class="spin sm" aria-hidden="true"></span>{busy}{:else}<Icon name="download" size="sm" />Télécharger le dossier{/if}</button>
      </div>
    </section>
  {/if}
</div>

{#if saveTpl}
  <Dialog title="Enregistrer comme modèle de dossier" eyebrow="Dossier" onclose={() => (saveTpl = null)} onsubmit={saveAsTemplate} submitLabel="Enregistrer" submitDisabled={!saveTpl.name.trim()}>
    <div class="stack">
      <div class="field"><label for="tpl-name">Nom</label><input class="input" id="tpl-name" bind:value={saveTpl.name} maxlength="100" required aria-invalid={!!tplErr?.field('name')} aria-describedby={tplErr?.field('name') ? 'tpl-name-err' : undefined}><FieldError message={tplErr?.field('name')} for="tpl-name" /></div>
      <div class="field"><label for="tpl-desc">Description (facultatif)</label><textarea class="input" id="tpl-desc" rows="3" maxlength="500" bind:value={saveTpl.description}></textarea></div>
      <button type="button" class="ux3-info" title="Le modèle garde la composition et les réglages, sans aucune donnée du projet." aria-label="Le modèle garde la composition et les réglages, sans aucune donnée du projet."><Icon name="info" size="sm" /></button>
    </div>
  </Dialog>
{/if}

{#if colModel}
  <Dialog title="Enregistrer ces colonnes" eyebrow={def(colModel.o.key).title} onclose={() => (colModel = null)} onsubmit={saveColModel} submitLabel="Enregistrer" submitDisabled={!colModel.name.trim()}>
    <div class="field"><label for="cm-name">Nom du modèle de colonnes</label><input class="input" id="cm-name" bind:value={colModel.name} maxlength="100" required></div>
  </Dialog>
{/if}

<style>
  .dz { display: grid; gap: 14px; }
  /* Étapes : une rangée compacte (pastilles reliées par un trait), qui défile si elle déborde. */
  .dz-steps { display: flex; align-items: center; gap: 0; list-style: none; padding: 0; margin: 0; overflow-x: auto; }
  .dz-steps li { flex: none; display: flex; align-items: center; margin: 0; padding: 0; border: 0; }
  .dz-steps .sep { width: 24px; height: 1px; margin: 0 6px; background: var(--line-2); }
  .dz-steps button { display: inline-flex; gap: 8px; align-items: center; height: 34px; white-space: nowrap; border: 0; box-shadow: inset 0 0 0 1px var(--line); background: var(--surface); border-radius: 99px; padding: 0 12px 0 5px; font: 600 13px var(--font); color: var(--fg-2); cursor: pointer; }
  .dz-steps button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .dz-steps button:disabled { cursor: not-allowed; opacity: .45; }
  .dz-steps .n { flex: none; width: 24px; height: 24px; border-radius: 50%; display: grid; place-items: center; background: var(--surface-2); font: 600 12px/1 var(--font); }
  .dz-steps .n :global(.ic) { width: 14px; height: 14px; }
  .dz-steps li.on button { box-shadow: inset 0 0 0 1.5px var(--accent); background: var(--accent-soft); color: var(--fg); }
  .dz-steps li.on .n { background: var(--accent); color: var(--primary-fg); }
  .dz-steps li.done .n { background: var(--ok); color: var(--primary-fg); }
  .st0, .st1 { padding: 20px; display: grid; gap: 10px; }
  .starts { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 12px; }
  .start { padding: 16px; display: grid; gap: 6px; justify-items: start; align-content: start; text-align: left; font: inherit; }
  button.start { cursor: pointer; }
  button.start:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  button.start:disabled { cursor: not-allowed; opacity: .45; }
  button.start.card.link:disabled:hover { transform: none; box-shadow: var(--card-sh); }
  .start.off { opacity: .45; }
  .grow { flex: 1; }
  .grp { margin: 14px 0 2px; }
  .doc { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; padding: 8px 10px; border-radius: 12px; box-shadow: inset 0 0 0 1px var(--line); }
  .doc.off { opacity: .6; }
  .doc.empty { background: var(--surface-2); color: var(--fg-3); }
  .doc.err { box-shadow: inset 0 0 0 1px var(--err); }
  .inc { min-width: 220px; }
  .pr { flex: 1; min-width: 140px; }
  .ttl { width: 160px; }
  .doc select.input.sm { width: auto; }
  .cols { flex-basis: 100%; display: flex; gap: 6px; flex-wrap: wrap; align-items: center; padding-left: 26px; }
  .col { font: 600 12px var(--font); padding: 3px 8px; border-radius: 99px; border: 1px dashed var(--line-2); background: none; color: var(--fg-2); cursor: pointer; display: inline-flex; gap: 2px; align-items: center; }
  .col.on { border-style: solid; border-color: var(--line); color: var(--fg); background: var(--surface-2); cursor: default; }
  .col .x { border: 0; background: none; cursor: pointer; padding: 0 3px; color: var(--fg-2); font: inherit; border-radius: 4px; }
  .col .x:focus-visible, button.col:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .col .x:disabled { opacity: .45; cursor: not-allowed; }
  .pick { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 4px; }
  .nav { display: flex; gap: 8px; align-items: center; margin-top: 10px; flex-wrap: wrap; }
  .rec td { vertical-align: middle; }
  .errt { color: var(--err); }
  .ok-t { color: var(--ok); font-weight: 600; }
  .spin.sm { width: 14px; height: 14px; }
  .fmtmode { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
</style>
