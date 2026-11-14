<script lang="ts">
  // Assistant de dossier (section 11.6) : départ (nouvelle composition, dernière exportation
  // ou modèle), composition, une étape par plan, récapitulatif, téléchargement. La dernière
  // exportation est mémorisée ; une composition peut devenir un modèle de dossier (6.4).
  import Icon from '../../components/Icon.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { fileStem } from '../../lib/pdfkit';
  import { build, choices, probe, FIXED, DEFAULT_FMT, COL_LABEL, GROUPS, type DocDef, type DocOpt, type Probe } from '../../lib/dossier';
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
  let busy = $state<string | null>(null);
  let done = $state<{ pages: number; skipped: string[] } | null>(null);

  async function init() {
    try {
      const [d, t, c, l] = await Promise.all([api.get<{ docs: DocDef[] }>('/print/docs'), api.get<{ items: DT[] }>('/print/dossier-templates'), api.get<{ items: CM[] }>('/print/column-models'), api.get<typeof last>(`/projects/${pid}/settings/dossier`)]);
      defs = d.docs; templates = t.items; colModels = c.items; last = l;
    } catch (e) { fail(e); }
  }
  init();

  function fresh(): DocOpt[] { return defs.map((d) => ({ key: d.key, include: true, paper: null, landscape: null, columns: [...d.columns], hidden: [], title: null })); }
  /** Une composition reprise est complétée : documents nouveaux insérés, colonnes nouvelles cochées. */
  function complete(list: DocOpt[]): DocOpt[] {
    return defs.map((d) => { const o = list.find((x) => x.key === d.key); if (!o) return { key: d.key, include: true, paper: null, landscape: null, columns: [...d.columns], hidden: [], title: null }; const cols = o.columns.filter((c) => d.columns.includes(c)); const hid = (o.hidden ?? []).filter((c) => d.columns.includes(c) && !cols.includes(c)); for (const c of d.columns) if (!cols.includes(c) && !hid.includes(c)) cols.push(c); return { ...o, columns: cols, hidden: hid }; });
  }
  async function startWith(list: DocOpt[], t?: string) { docs = complete(list); if (t) title = t; step = 1; done = null; await runProbes(); }
  async function runProbes() {
    const ctx = { view, cache: new Map() };
    const out: Record<string, Probe> = {};
    await Promise.all(defs.map(async (d) => { out[d.key] = await probe(ctx as any, d.key); }));
    probes = out;
    // Les pages des plans sont toutes retenues par défaut, sauf choix déjà fait.
    for (const o of docs) { const ch = out[o.key]?.choices; if (ch && !o.pages) o.pages = ch.map((c) => c.id); }
  }
  void choices;

  const planSteps = $derived(docs.filter((o) => o.include && probes[o.key]?.choices && probes[o.key]!.state === 'ok'));
  const steps = $derived(['Départ', 'Composition', ...planSteps.map((o) => defs.find((d) => d.key === o.key)?.title ?? o.key), 'Récapitulatif']);
  const fmt = (o: DocOpt) => FIXED[o.key] ?? [o.paper ?? DEFAULT_FMT[o.key]?.[0] ?? 'A4', o.landscape ?? DEFAULT_FMT[o.key]?.[1] ?? false];
  const def = (k: string) => defs.find((d) => d.key === k)!;
  const included = $derived(docs.filter((o) => o.include));
  /** Tout en A4, ou A3 pour les plans (formats par défaut) : un choix explicite, annoncé au sommaire. */
  const allA4 = $derived(included.every((o) => fmt(o)[0] === 'A4'));
  function setPlans(p: 'A4' | 'A3') { for (const o of docs) { if (FIXED[o.key]) continue; if (def(o.key)?.group === 'plans') o.paper = p; else if (p === 'A4') o.paper = 'A4'; } }

  function toggleCol(o: DocOpt, c: string, on: boolean) { if (on) { o.columns = [...o.columns, c]; o.hidden = o.hidden.filter((x) => x !== c); } else { o.columns = o.columns.filter((x) => x !== c); o.hidden = [...o.hidden, c]; } }
  function moveCol(o: DocOpt, i: number, d: number) { const j = i + d; if (j < 0 || j >= o.columns.length) return; const l = [...o.columns]; [l[i], l[j]] = [l[j]!, l[i]!]; o.columns = l; }
  async function saveColModel(o: DocOpt) {
    const name = prompt('Nom du modèle de colonnes :'); if (!name?.trim()) return;
    try { const r = await api.post<CM>('/print/column-models', { id: '', table: o.key, name, columns: o.columns }); colModels = [...colModels, r]; toast('Modèle de colonnes enregistré.'); } catch (e) { fail(e); }
  }
  function applyColModel(o: DocOpt, id: string) { const m = colModels.find((x) => x.id === id); if (!m) return; const all = def(o.key).columns; o.columns = m.columns.filter((c) => all.includes(c)); o.hidden = all.filter((c) => !o.columns.includes(c)); }

  async function download(only?: string) {
    busy = only ? `Document ${def(only).title}…` : 'Assemblage…';
    try {
      const r = await build(view, defs, $state.snapshot(docs) as DocOpt[], title, (m) => (busy = m), only);
      const a = document.createElement('a'); a.href = URL.createObjectURL(r.blob);
      a.download = `${fileStem(only ? (def(only).title.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^A-Za-z0-9]+/g, '-')) : 'Dossier', view.project.name)}.pdf`;
      a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 3000);
      if (!only) {
        done = { pages: r.pages, skipped: r.skipped };
        try { last = await api.put(`/projects/${pid}/settings/dossier`, { rev: last.rev, data: { docs: $state.snapshot(docs), title } }); } catch { /* mémorisation au prochain export */ }
        toast(`Dossier exporté : ${r.pages} pages.`);
      }
    } catch (e) { fail(e); } finally { busy = null; }
  }

  let saveTpl = $state<{ name: string; description: string } | null>(null);
  let tplErr = $state<ApiError | null>(null);
  async function saveAsTemplate() {
    if (!saveTpl) return;
    try { await api.post('/print/dossier-templates', { id: '', name: saveTpl.name, description: saveTpl.description || null, docs: $state.snapshot(docs).map(({ pages, ...o }: DocOpt) => { void pages; return o; }) }); saveTpl = null; toast('Modèle de dossier enregistré.'); templates = (await api.get<{ items: DT[] }>('/print/dossier-templates')).items; } catch (e) { if (e instanceof ApiError && e.fields.length) tplErr = e; else fail(e); }
  }
</script>

<div class="dz">
  <ol class="dz-steps" aria-label="Étapes">
    {#each steps as s, i}<li class:on={i === step} class:done={i < step}>{#if i}<span class="sep" aria-hidden="true"></span>{/if}<button disabled={i > 0 && !docs.length} aria-current={i === step ? 'step' : undefined} onclick={() => (step = i)}><span class="n">{#if i < step}<Icon name="check" size="sm" />{:else}{i + 1}{/if}</span>{s}</button></li>{/each}
  </ol>

  {#if step === 0}
    <section class="card st0">
      <h2>Composer le dossier</h2>
      <p class="muted">Un seul PDF : page de garde avec sommaire, documents à leur format, page de fin, pagination continue. Les données sont relues au moment de l’export.</p>
      <div class="starts">
        <button class="card start" onclick={() => startWith(fresh())}><Icon name="plus" /><b>Nouvelle composition</b><span class="muted small">Tous les documents, réglages par défaut.</span></button>
        <button class="card start" disabled={!last.data?.docs} onclick={() => startWith(last.data!.docs!, last.data!.title)}><Icon name="refresh" /><b>Reprendre la dernière</b><span class="muted small">{last.data?.docs ? 'Composition du dernier export réussi.' : 'Aucun export réussi pour ce projet.'}</span></button>
        <div class="card start" class:off={!templates.length} aria-disabled={!templates.length}><Icon name="file" /><b>Modèle enregistré</b>
          {#if templates.length}<select class="input" onchange={(e) => { const t = templates.find((x) => x.id === (e.target as HTMLSelectElement).value); if (t) startWith(t.docs, t.name); }}><option value="">Choisir…</option>{#each templates as t}<option value={t.id}>{t.name}</option>{/each}</select>{:else}<span class="muted small">Aucun modèle de dossier.</span>{/if}
        </div>
      </div>
    </section>
  {:else if step === 1}
    <section class="card st1">
      <div class="row"><label class="field grow">Titre du dossier<input class="input" bind:value={title} maxlength="80"></label></div>
      <div class="row fmtmode"><span class="flabel">Formats</span><div class="seg" role="group" aria-label="Formats du dossier"><button class:on={allA4} onclick={() => setPlans('A4')}>Tout en A4</button><button class:on={!allA4} onclick={() => setPlans('A3')}>A3 pour les plans</button></div><span class="muted small">{allA4 ? 'Un seul format : A4.' : 'Formats mélangés : A4 pour les listes, A3 pour les plans (indiqué au sommaire).'}</span></div>
      {#each Object.entries(GROUPS) as [g, gl]}
        <h3 class="grp">{gl}</h3>
        {#each docs.filter((o) => def(o.key)?.group === g) as o (o.key)}
          {@const d = def(o.key)}{@const pr = probes[o.key]}{@const f = fmt(o)}
          <div class="doc" class:off={!o.include} class:empty={pr?.state === 'empty'} class:err={pr?.state === 'error'}>
            <label class="inc"><input type="checkbox" bind:checked={o.include}><b>{d.title}</b></label>
            <span class="pr small">{#if !pr}<span class="spin sm"></span>{:else if pr.state === 'ok'}<span class="muted">{pr.message}</span>{:else if pr.state === 'empty'}<span class="muted">Vide : ne sera pas imprimé</span>{:else}<span class="errt">En échec : {pr.message}</span>{/if}</span>
            {#if o.key !== 'cover'}
              <input class="input sm ttl" placeholder="Titre court" value={o.title ?? ''} maxlength="60" oninput={(e) => (o.title = (e.target as HTMLInputElement).value || null)} aria-label="Titre court de {d.title}">
              <select class="input sm" value={f[0]} disabled={!!FIXED[o.key]} onchange={(e) => (o.paper = (e.target as HTMLSelectElement).value as 'A4' | 'A3')} aria-label="Papier"><option value="A4">A4</option><option value="A3">A3</option></select>
              <select class="input sm" value={f[1] ? 'l' : 'p'} disabled={!!FIXED[o.key] || o.key === 'recette'} onchange={(e) => (o.landscape = (e.target as HTMLSelectElement).value === 'l')} aria-label="Orientation"><option value="p">Portrait</option><option value="l">Paysage</option></select>
            {/if}
            {#if d.columns.length && o.include}
              <div class="cols">
                {#each o.columns as c, i (c)}<span class="col on">{COL_LABEL[c] ?? c}<button class="x" aria-label="Avancer" disabled={i === 0} onclick={() => moveCol(o, i, -1)}>‹</button><button class="x" aria-label="Reculer" disabled={i === o.columns.length - 1} onclick={() => moveCol(o, i, 1)}>›</button><button class="x" aria-label="Retirer {COL_LABEL[c]}" onclick={() => toggleCol(o, c, false)}>×</button></span>{/each}
                {#each d.columns.filter((c) => !o.columns.includes(c)) as c}<button class="col" onclick={() => toggleCol(o, c, true)}>+ {COL_LABEL[c] ?? c}</button>{/each}
                {#if colModels.some((m) => m.table === o.key)}<select class="input sm" onchange={(e) => applyColModel(o, (e.target as HTMLSelectElement).value)} aria-label="Modèle de colonnes"><option value="">Modèle de colonnes…</option>{#each colModels.filter((m) => m.table === o.key) as m}<option value={m.id}>{m.name}</option>{/each}</select>{/if}
                {#if writable}<button class="btn sm ghost" onclick={() => saveColModel(o)}>Enregistrer ces colonnes</button>{/if}
              </div>
            {/if}
          </div>
        {/each}
      {/each}
      <div class="nav"><button class="btn" onclick={() => (step = 0)}>Retour</button><span style="flex:1"></span><button class="btn primary" disabled={!included.length} onclick={() => (step = 2)}>Continuer</button></div>
    </section>
  {:else if step < steps.length - 1}
    {@const o = planSteps[step - 2]!}
    {@const pr = probes[o.key]!}
    <section class="card st1">
      <h2>{def(o.key).title}</h2>
      <p class="muted small">Pages imprimées, dans cet ordre ; chacune reçoit le cartouche du site.</p>
      <div class="pick">{#each pr.choices ?? [] as ch}<label class="toggle-row"><input type="checkbox" checked={o.pages?.includes(ch.id)} onchange={(e) => { const on = (e.target as HTMLInputElement).checked; o.pages = on ? [...(o.pages ?? []), ch.id] : (o.pages ?? []).filter((x) => x !== ch.id); }}>{ch.label}</label>{/each}</div>
      <div class="nav"><button class="btn" onclick={() => step--}>Retour</button><span style="flex:1"></span><button class="btn primary" onclick={() => step++}>Continuer</button></div>
    </section>
  {:else}
    <section class="card st1">
      <h2>Récapitulatif</h2>
      <table class="tbl rec"><thead><tr><th>Document</th><th>Format</th><th>Contenu</th><th></th></tr></thead><tbody>
        {#each included as o (o.key)}
          {@const pr = probes[o.key]}{@const f = fmt(o)}
          <tr class:muted={pr?.state !== 'ok'}><td><b>{o.title ?? def(o.key).title}</b><span class="muted small">{' · '}{GROUPS[def(o.key).group]}</span></td><td>{o.key === 'cover' ? 'A4 portrait' : `${f[0]} ${f[1] ? 'paysage' : 'portrait'}`}</td>
            <td class="small">{pr?.state === 'ok' ? (o.pages ? `${o.pages.length} page${o.pages.length > 1 ? 's' : ''}` : pr.message) : pr?.state === 'empty' ? 'Vide : ignoré' : `En échec : ${pr?.message ?? ''}`}</td>
            <td>{#if o.key !== 'cover' && pr?.state === 'ok'}<button class="btn sm ghost" disabled={!!busy} onclick={() => download(o.key)}><Icon name="download" size="sm" />Seul</button>{/if}</td></tr>
        {/each}
      </tbody></table>
      {#if done}<p class="ok-t">Dossier exporté : {done.pages} pages.{#if done.skipped.length} Ignorés : {done.skipped.join(' ; ')}.{/if}</p>{/if}
      <div class="nav">
        <button class="btn" onclick={() => step--}>Retour</button><span style="flex:1"></span>
        {#if writable}<button class="btn ghost" onclick={() => { saveTpl = { name: title, description: '' }; tplErr = null; }}>Enregistrer comme modèle</button>{/if}
        <button class="btn primary" disabled={!!busy} onclick={() => download()}>{#if busy}<span class="spin sm"></span>{busy}{:else}<Icon name="download" size="sm" />Télécharger le dossier{/if}</button>
      </div>
    </section>
  {/if}
</div>

{#if saveTpl}
  <Dialog title="Enregistrer comme modèle de dossier" eyebrow="Dossier" onclose={() => (saveTpl = null)}>
    <label class="field">Nom<input class="input" bind:value={saveTpl.name} maxlength="100">{#if tplErr?.field('name')}<span class="errt small">{tplErr.field('name')}</span>{/if}</label>
    <label class="field">Description<textarea class="input" rows="3" maxlength="500" bind:value={saveTpl.description}></textarea></label>
    <p class="muted small">Le modèle garde la composition et les réglages, sans aucune donnée du projet.</p>
    {#snippet footer()}<button class="btn" onclick={() => (saveTpl = null)}>Annuler</button><button class="btn primary" disabled={!saveTpl?.name.trim()} onclick={saveAsTemplate}>Enregistrer</button>{/snippet}
  </Dialog>
{/if}

<style>
  .dz { display: grid; gap: 14px; }
  /* Étapes : une rangée compacte (pastilles reliées par un trait), qui défile si elle déborde. */
  .dz-steps { display: flex; align-items: center; gap: 0; list-style: none; padding: 0; margin: 0; overflow-x: auto; }
  .dz-steps li { flex: none; display: flex; align-items: center; margin: 0; padding: 0; border: 0; }
  .dz-steps .sep { width: 24px; height: 1px; margin: 0 6px; background: var(--line-2, var(--line)); }
  .dz-steps button { display: inline-flex; gap: 8px; align-items: center; height: 34px; white-space: nowrap; border: 1px solid var(--line); background: var(--surface); border-radius: 99px; padding: 0 12px 0 5px; font: 600 13px var(--font); color: var(--fg-2); cursor: pointer; }
  .dz-steps button:disabled { cursor: default; opacity: .55; }
  .dz-steps .n { flex: none; width: 24px; height: 24px; border-radius: 50%; display: grid; place-items: center; background: var(--surface-2); font: 600 12px/1 var(--font); }
  .dz-steps .n :global(.ic) { width: 14px; height: 14px; }
  .dz-steps li.on button { border-color: var(--accent, #4F6BFF); background: color-mix(in srgb, var(--accent, #4F6BFF) 12%, var(--surface)); color: var(--fg); }
  .dz-steps li.on .n { background: var(--accent, #4F6BFF); color: #fff; }
  .dz-steps li.done .n { background: var(--ok, #0E8550); color: #fff; }
  .st0, .st1 { padding: 20px; display: grid; gap: 10px; }
  .starts { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 12px; }
  .start { padding: 16px; display: grid; gap: 6px; justify-items: start; text-align: left; font: inherit; color: inherit; cursor: pointer; }
  .start { border: 1px solid var(--line); box-shadow: none; }
  button.start:not(:disabled):hover { border-color: var(--fg-3); }
  .start:disabled, .start.off { opacity: .5; cursor: default; }
  .row { display: flex; gap: 10px; }
  .grow { flex: 1; }
  .field { display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
  .grp { margin: 14px 0 2px; font-size: 13px; text-transform: uppercase; letter-spacing: .05em; color: var(--fg-2); }
  .doc { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; padding: 8px 10px; border: 1px solid var(--line); border-radius: 12px; }
  .doc.off { opacity: .6; }
  .doc.empty { background: var(--surface-2); }
  .doc.err { border-color: var(--err); }
  .inc { display: inline-flex; gap: 8px; align-items: center; min-width: 220px; }
  .pr { flex: 1; min-width: 140px; }
  .ttl { width: 160px; }
  .doc select.input.sm { width: auto; height: 32px; }
  .input.sm { height: 32px; }
  .cols { flex-basis: 100%; display: flex; gap: 6px; flex-wrap: wrap; align-items: center; padding-left: 26px; }
  .col { font: 600 12px var(--font); padding: 3px 8px; border-radius: 99px; border: 1px dashed var(--line); background: none; color: var(--fg-2); cursor: pointer; display: inline-flex; gap: 2px; align-items: center; }
  .col.on { border-style: solid; color: var(--fg); background: var(--surface-2); cursor: default; }
  .col .x { border: 0; background: none; cursor: pointer; padding: 0 3px; color: var(--fg-2); font: inherit; }
  .col .x:disabled { opacity: .3; }
  .pick { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 4px; }
  .nav { display: flex; gap: 8px; align-items: center; margin-top: 10px; flex-wrap: wrap; }
  .rec td { vertical-align: middle; }
  .errt { color: var(--err); }
  .ok-t { color: var(--ok, #0E8550); font-weight: 600; }
  .spin.sm { width: 14px; height: 14px; }
  .fmtmode { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
</style>
