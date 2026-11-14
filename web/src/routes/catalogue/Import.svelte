<script lang="ts">
  // Import d'équipements depuis un tableur : fichier, colonnes, valeurs inconnues, essai à blanc, import.
  import Icon from '../../components/Icon.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { loadRefs, refList, type RefPath } from '../../lib/catalog.svelte';
  import { nf, pl } from '../../lib/format';

  type Field = string | null;
  interface Analysis { session_id: string; filename: string; headers: string[]; row_count: number; preview: string[][]; matches: { column: number; header: string; field: Field; score: number }[]; fields: { field: string; label: string }[]; unknown: Unknown }
  type Unknown = Record<string, { value: string; rows: number; suggestion: { id: string; name: string } | null; can_create: boolean }[]>;
  interface Report { dry_run: boolean; created: number; updated: number; skipped: { row: number; name: string; reason: string }[]; errors: { row: number; name: string; messages: string[] }[]; issues: { row: number; field: string; value: string; message: string }[]; refs_created: { name: string }[]; snapshot_id: string | null }

  const LOOKUPS: { key: string; ref: RefPath; label: string }[] = [
    { key: 'brands', ref: 'brands', label: 'Marques' }, { key: 'categories', ref: 'categories', label: 'Catégories' },
    { key: 'types', ref: 'types', label: 'Types' }, { key: 'functions', ref: 'functions', label: 'Fonctions' }, { key: 'phases', ref: 'phases', label: 'Phases' },
  ];

  let step = $state(1);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let a = $state<Analysis | null>(null);
  let mapping = $state<Field[]>([]);
  let unknown = $state<Unknown>({});
  let choice = $state<Record<string, Record<string, string>>>({}); // lookup → valeur → "map:<id>" | "create" | "ignore"
  let strategy = $state<'skip' | 'update' | 'create'>('skip');
  let report = $state<Report | null>(null);
  let dragging = $state(false);
  let input = $state<HTMLInputElement>();
  const canCreateRefs = $derived(can('parametres', 'ecriture'));

  async function send(f: File) {
    busy = true; error = null;
    try {
      a = await api.upload<Analysis>(`/catalog/import/analyze?filename=${encodeURIComponent(f.name)}`, f);
      mapping = a.matches.map((m) => m.field);
      setUnknown(a.unknown);
      step = 2;
    } catch (e) { error = e instanceof ApiError ? e.message : 'Lecture impossible.'; } finally { busy = false; }
  }
  function setUnknown(u: Unknown) {
    unknown = u;
    const c: typeof choice = {};
    for (const l of LOOKUPS) {
      c[l.key] = {};
      for (const v of u[l.key] ?? []) c[l.key]![v.value] = choice[l.key]?.[v.value] ?? (v.suggestion ? `map:${v.suggestion.id}` : v.can_create && canCreateRefs ? 'create' : 'ignore');
    }
    choice = c;
  }
  const body = () => ({
    mapping: mapping.map((f, column) => ({ column, field: f })),
    values: Object.fromEntries(LOOKUPS.map((l) => [l.key, Object.fromEntries(Object.entries(choice[l.key] ?? {}).map(([v, c]) => [v, c.startsWith('map:') ? { action: 'map', id: c.slice(4) } : { action: c }]))])),
    duplicates: strategy,
  });
  async function toValues() {
    busy = true;
    try { setUnknown((await api.post<{ unknown: Unknown }>(`/catalog/import/${a!.session_id}/values`, { mapping: body().mapping })).unknown); step = 3; } catch (e) { fail(e); } finally { busy = false; }
  }
  async function run(dry: boolean) {
    busy = true; error = null;
    try {
      report = await api.post<Report>(`/catalog/import/${a!.session_id}/run`, { ...body(), dry_run: dry });
      step = dry ? 4 : 5;
      if (!dry) { toast(`Import terminé : ${report.created} fiche${pl(report.created)} créée${pl(report.created)}, ${report.updated} mise${pl(report.updated)} à jour.`); loadRefs(true); }
    } catch (e) { error = e instanceof ApiError ? e.message : 'Import impossible.'; } finally { busy = false; }
  }
  async function restart() {
    if (a && step < 5) api.del(`/catalog/import/${a.session_id}`).catch(() => {});
    a = null; report = null; step = 1; error = null;
  }
  const dupMap = $derived.by(() => { const s = new Map<string, number>(); mapping.forEach((f) => f && s.set(f, (s.get(f) ?? 0) + 1)); return s; });
  const hasName = $derived(mapping.includes('name'));
  const unknownCount = $derived(LOOKUPS.reduce((s, l) => s + (unknown[l.key]?.length ?? 0), 0));
  const STEPS = ['Fichier', 'Colonnes', 'Valeurs', 'Essai', 'Terminé'];
</script>

<section class="card stack">
  <div class="card-head">
    <div><p class="eyebrow">Import d’équipements · étape {step} sur 5</p><h2>{STEPS[step - 1]}</h2></div>
    {#if step > 1}<button class="btn sm ghost" onclick={restart}>Recommencer</button>{/if}
  </div>
  <div class="bar"><i style="width:{step * 20}%"></i></div>

  {#if step === 1}
    <p class="muted">Déposez un tableur : CSV (UTF-8 ou Windows-1252, séparateur deviné), Excel (.xlsx, .xls) ou OpenDocument (.ods). La première ligne doit porter les en-têtes ; les colonnes sont reconnues même avec d’autres noms (« Désignation », « Fabricant », « Conso (W) »…). Rien n’est écrit avant la dernière étape.</p>
    <button type="button" class="drop" class:on={dragging} onclick={() => input?.click()}
      ondragover={(e) => { e.preventDefault(); dragging = true; }} ondragleave={() => (dragging = false)}
      ondrop={(e) => { e.preventDefault(); dragging = false; const f = e.dataTransfer?.files[0]; if (f) send(f); }}>
      {#if busy}<span class="spin"></span>Lecture du fichier…{:else}<Icon name="upload" /><b>Choisir un fichier</b><span class="muted">ou le glisser ici · 20 Mo et 5 000 lignes au plus</span>{/if}
    </button>
    <input type="file" accept=".csv,.txt,.tsv,.xlsx,.xlsm,.xls,.ods" bind:this={input} hidden onchange={() => { const f = input?.files?.[0]; if (f) send(f); }}>
    <p class="muted small">Astuce : un export du catalogue (« Export et échange ») se réimporte tel quel.</p>
  {:else if step === 2 && a}
    <p class="muted">{a.filename} · {nf(a.row_count)} ligne{a.row_count > 1 ? 's' : ''}. Vérifiez à quel champ correspond chaque colonne ; une colonne ignorée n’est pas lue.</p>
    <div class="tbl-wrap">
      <table class="tbl">
        <thead><tr><th>Colonne du fichier</th><th>Exemples</th><th>Champ Strata Arkyo</th></tr></thead>
        <tbody>
          {#each a.headers as h, i}
            <tr>
              <td><b>{h || `Colonne ${i + 1}`}</b></td>
              <td class="small muted">{a.preview.slice(0, 3).map((r) => r[i]).filter(Boolean).join(' · ') || '—'}</td>
              <td>
                <select class="input" class:err={!!mapping[i] && (dupMap.get(mapping[i]!) ?? 0) > 1} style="height:38px" value={mapping[i] ?? ''} onchange={(e) => (mapping[i] = (e.target as HTMLSelectElement).value || null)} aria-label="Champ pour {h}">
                  <option value="">Ignorer</option>{#each a.fields as f}<option value={f.field}>{f.label}</option>{/each}
                </select>
                {#if a.matches[i]?.field && a.matches[i]?.field === mapping[i]}<span class="help">Reconnu à {Math.round(a.matches[i]!.score * 100)} %</span>{/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if !hasName}<div class="alert warn"><Icon name="alert" /><span>Indiquez la colonne qui porte le nom des équipements.</span></div>{/if}
    {#if [...dupMap.values()].some((n) => n > 1)}<div class="alert warn"><Icon name="alert" /><span>Deux colonnes visent le même champ.</span></div>{/if}
    <div class="row-actions"><button class="btn primary" onclick={toValues} disabled={busy || !hasName || [...dupMap.values()].some((n) => n > 1)}>{#if busy}<span class="spin"></span>{/if}Continuer</button></div>
  {:else if step === 3 && a}
    {#if !unknownCount}<div class="alert info"><Icon name="ok" /><span>Toutes les marques, catégories, types, fonctions et phases du fichier existent déjà dans les référentiels.</span></div>
    {:else}
      <p class="muted">Ces valeurs du fichier ne correspondent à aucun élément des référentiels. Associez-les à un élément existant, créez-les{canCreateRefs ? '' : ' (demande le droit « Paramètres et site : écriture »)'}, ou ignorez-les.</p>
      {#each LOOKUPS as l}
        {#if unknown[l.key]?.length}
          <h3 style="font-size:17px;margin-top:6px">{l.label}</h3>
          <div class="tbl-wrap"><table class="tbl"><tbody>
            {#each unknown[l.key]! as v}
              <tr>
                <td><b>« {v.value} »</b><br><span class="muted small">{v.rows} ligne{v.rows > 1 ? 's' : ''}</span></td>
                <td style="width:50%">
                  <select class="input" style="height:38px" bind:value={choice[l.key]![v.value]} aria-label="Que faire de {v.value}">
                    <option value="ignore">Ignorer la valeur</option>
                    {#if v.can_create && canCreateRefs}<option value="create">Créer « {v.value} »</option>{/if}
                    <optgroup label="Associer à">{#each refList(l.ref) as r}<option value="map:{r.id}">{r.name}{v.suggestion?.id === r.id ? ' (suggéré)' : ''}</option>{/each}</optgroup>
                  </select>
                </td>
              </tr>
            {/each}
          </tbody></table></div>
        {/if}
      {/each}
    {/if}
    <div class="field"><span class="flabel">Si un équipement existe déjà (même référence, ou même nom et même marque)</span>
      <div class="seg">{#each [['skip', 'Garder l’existant'], ['update', 'Le mettre à jour'], ['create', 'Créer quand même']] as [k, l]}<button class:on={strategy === k} onclick={() => (strategy = k as typeof strategy)}>{l}</button>{/each}</div>
      <span class="help">« Mettre à jour » ne remplace que les colonnes importées ; les ports et le reste de la fiche sont gardés.</span>
    </div>
    {#if error}<div class="alert err">{error}</div>{/if}
    <div class="row-actions"><button class="btn ghost" onclick={() => (step = 2)}>Retour</button><button class="btn primary" onclick={() => run(true)} disabled={busy}>{#if busy}<span class="spin"></span>{/if}Faire un essai à blanc</button></div>
  {:else if (step === 4 || step === 5) && report}
    <div class="figs" style="grid-template-columns:repeat(4,minmax(0,1fr))">
      <div class="fig"><span class="lbl-t">{report.dry_run ? 'À créer' : 'Créées'}</span><b>{report.created}</b></div>
      <div class="fig"><span class="lbl-t">{report.dry_run ? 'À mettre à jour' : 'Mises à jour'}</span><b>{report.updated}</b></div>
      <div class="fig"><span class="lbl-t">Ignorées</span><b>{report.skipped.length}</b></div>
      <div class="fig"><span class="lbl-t">En erreur</span><b style:color={report.errors.length ? 'var(--err)' : undefined}>{report.errors.length}</b></div>
    </div>
    {#if report.refs_created.length}<p class="muted small">{report.dry_run ? 'Seront créés' : 'Créés'} dans les référentiels : {report.refs_created.map((r) => r.name).join(', ')}.</p>{/if}
    {#if report.errors.length}
      <details open><summary><b>Lignes en erreur</b> ({report.errors.length})</summary>
        <ul class="notes">{#each report.errors as e}<li>Ligne {e.row}{e.name ? ` · ${e.name}` : ''} : {e.messages.join(' ')}</li>{/each}</ul></details>
    {/if}
    {#if report.issues.length}
      <details><summary><b>Cellules corrigées ou vidées</b> ({report.issues.length})</summary>
        <ul class="notes">{#each report.issues as i}<li>Ligne {i.row} : « {i.value} » · {i.message}</li>{/each}</ul></details>
    {/if}
    {#if report.skipped.length}
      <details><summary><b>Lignes ignorées</b> ({report.skipped.length})</summary>
        <ul class="notes">{#each report.skipped as s}<li>Ligne {s.row} · {s.name} : {s.reason}</li>{/each}</ul></details>
    {/if}
    {#if error}<div class="alert err">{error}</div>{/if}
    {#if step === 4}
      <div class="row-actions"><button class="btn ghost" onclick={() => (step = 3)}>Retour</button><button class="btn primary" onclick={() => run(false)} disabled={busy || (!report.created && !report.updated)}>{#if busy}<span class="spin"></span>{/if}Importer {report.created + report.updated} fiche{report.created + report.updated > 1 ? 's' : ''}</button></div>
    {:else}
      <div class="alert info"><Icon name="ok" /><span>Import terminé. Un instantané du catalogue a été pris juste avant : en cas d’erreur, revenez en arrière depuis « Export et échange ».</span></div>
      <div class="row-actions"><button class="btn" onclick={restart}>Importer un autre fichier</button><a class="btn primary" href="/catalogue/equipment">Voir les équipements</a></div>
    {/if}
  {/if}
  {#if error && step === 1}<div class="alert err">{error}</div>{/if}
</section>

<style>
  .drop { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; min-height: 180px; border: 2px dashed var(--line-2); border-radius: 18px; background: var(--surface-2); color: var(--fg); font-size: 16px; }
  .drop.on, .drop:hover { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 6%, var(--surface-2)); }
  .drop :global(.ic) { width: 28px; height: 28px; }
  details summary { cursor: pointer; padding: 6px 0; }
  .fig b { font-size: 44px; }
</style>
