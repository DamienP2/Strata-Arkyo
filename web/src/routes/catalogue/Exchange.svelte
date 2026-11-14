<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import { api, download, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { loadRefs } from '../../lib/catalog.svelte';
  import { dateTime, pl } from '../../lib/format';

  const SECTIONS: [string, string][] = [['refs', 'Référentiels'], ['equipment', 'Équipements'], ['cables', 'Câbles'], ['cable-models', 'Types de câble'], ['adapters', 'Adaptateurs']];
  let exportSel = $state<string[]>(SECTIONS.map((s) => s[0]));
  const admin = $derived(can('catalogue', 'admin'));

  // Échange : fichier d'un autre site
  let doc = $state<any>(null);
  let docName = $state('');
  let mode = $state<'merge' | 'replace'>('merge');
  let importSel = $state<string[]>([]);
  let result = $state<any>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let fileIn = $state<HTMLInputElement>();

  let snapshots = $state<{ id: string; created_at: string; reason: string; sections: string[] }[]>([]);
  let restoring = $state<(typeof snapshots)[number] | null>(null);
  async function loadSnaps() { if (admin) try { snapshots = (await api.get<{ snapshots: typeof snapshots }>('/catalog/snapshots')).snapshots; } catch (e) { fail(e); } }
  loadSnaps();

  async function pick() {
    const f = fileIn?.files?.[0];
    if (!f) return;
    error = null; result = null;
    try {
      doc = JSON.parse(await f.text()); docName = f.name;
      if (doc?.format !== 'strata-arkyo/catalogue') throw new Error('Ce fichier n’est pas un catalogue Strata Arkyo.');
      importSel = SECTIONS.map((s) => s[0]).filter((s) => (s === 'refs' ? doc.refs && Object.keys(doc.refs).length : doc.items?.[s]));
    } catch (e) { doc = null; error = (e as Error).message; }
  }
  async function exchange(dry: boolean) {
    busy = true; error = null;
    try {
      result = await api.post('/catalog/exchange', { document: doc, mode, sections: importSel.join(','), dry_run: dry });
      if (!dry) { toast('Échange appliqué.'); loadRefs(true); loadSnaps(); }
    } catch (e) { error = e instanceof ApiError ? e.message : 'Échange impossible.'; } finally { busy = false; }
  }
  async function restore() {
    try {
      const r = await api.post<{ restored: { refs: number; items: number } }>(`/catalog/snapshots/${restoring!.id}/restore`);
      toast(`Retour arrière effectué : ${r.restored.items} fiche${pl(r.restored.items)}, ${r.restored.refs} élément${pl(r.restored.refs)} de référentiel.`); restoring = null; loadRefs(true); loadSnaps();
    } catch (e) { fail(e); }
  }
  const REFS: Record<string, string> = { functions: 'fonctions', types: 'types', brands: 'marques', connectors: 'connecteurs', signals: 'signaux', categories: 'catégories', dimensions: 'dimensions' };
  const SEC = (s: string) => s.startsWith('refs/') ? `Référentiel des ${REFS[s.slice(5)] ?? s.slice(5)}` : SECTIONS.find((x) => x[0] === s)?.[1] ?? s;
  /** Sections d'un instantané en clair : les référentiels sont réunis en une mention. */
  const secList = (ss: string[]) => {
    const refs = ss.filter((x) => x.startsWith('refs/')).map((x) => REFS[x.slice(5)] ?? x.slice(5));
    const rest = ss.filter((x) => !x.startsWith('refs/')).map(SEC);
    return [...(refs.length ? [`Référentiels (${refs.join(', ')})`] : []), ...rest].join(' · ');
  };
  const counts = (c: any) => `${c.added} ajouté${pl(c.added)}, ${c.updated} mis à jour, ${c.unchanged} inchangé${pl(c.unchanged)}${c.rejected ? `, ${c.rejected} refusé${pl(c.rejected)}` : ''}`;
</script>

<div class="grid-2">
  <section class="card stack">
    <div class="card-head"><div><p class="eyebrow">Tableur</p><h2>Exporter les équipements</h2></div></div>
    <p class="muted">La version simple se réimporte telle quelle. La version détaillée ajoute une ligne par groupe de ports, pour relire la connectique.</p>
    <div class="row-actions" style="justify-content:flex-start">
      <button class="btn" onclick={() => download('/catalog/export?format=xlsx')}><Icon name="download" />Excel</button>
      <button class="btn" onclick={() => download('/catalog/export?format=csv')}><Icon name="download" />CSV</button>
      <button class="btn ghost" onclick={() => download('/catalog/export?format=xlsx&full=true')}><Icon name="download" />Excel détaillé</button>
    </div>
  </section>
  <section class="card stack">
    <div class="card-head"><div><p class="eyebrow">Entre sites</p><h2>Exporter le catalogue</h2></div></div>
    <p class="muted">Un fichier JSON complet, à importer sur un autre site Strata Arkyo.</p>
    <div style="display:flex;gap:14px;flex-wrap:wrap">{#each SECTIONS as [k, l]}<label class="toggle-row"><input type="checkbox" value={k} bind:group={exportSel}>{l}</label>{/each}</div>
    <div class="row-actions" style="justify-content:flex-start"><button class="btn primary" disabled={!exportSel.length} onclick={() => download(`/catalog/exchange?sections=${exportSel.join(',')}`)}><Icon name="download" />Exporter</button></div>
  </section>
</div>

{#if admin}
  <section class="card stack">
    <div class="card-head"><div><p class="eyebrow">Entre sites</p><h2>Importer le catalogue d’un autre site</h2></div></div>
    <div style="display:flex;gap:10px;align-items:center;flex-wrap:wrap">
      <button class="btn" onclick={() => fileIn?.click()}><Icon name="upload" />Choisir un fichier JSON</button><span class="muted">{docName}</span>
      <input type="file" accept=".json,application/json" bind:this={fileIn} hidden onchange={pick}>
    </div>
    {#if doc}
      <p class="muted small">Exporté {doc.site ? `par ${doc.site} ` : ''}le {dateTime(doc.exported_at)} · version {doc.engine} de l’application.</p>
      <div style="display:flex;gap:14px;flex-wrap:wrap">{#each SECTIONS as [k, l]}<label class="toggle-row"><input type="checkbox" value={k} bind:group={importSel}>{l}</label>{/each}</div>
      <div class="field"><span class="flabel">Mode</span>
        <div class="seg"><button class:on={mode === 'merge'} onclick={() => (mode = 'merge')}>Fusionner</button><button class:on={mode === 'replace'} onclick={() => (mode = 'replace')}>Remplacer</button></div>
        <span class="help">{mode === 'merge' ? 'Ajoute ce qui manque ; pour un élément présent des deux côtés, la version la plus récente l’emporte.' : 'Vide les sections choisies puis reprend le contenu du fichier. Un élément local encore utilisé est gardé.'}</span>
      </div>
      {#if result}
        <div class="alert {result.problems.length ? 'warn' : 'info'}"><Icon name={result.problems.length ? 'alert' : 'ok'} /><div>
          <b>{result.dry_run ? 'Essai à blanc' : 'Échange appliqué'}</b><br>Référentiels : {counts(result.refs)}.<br>Fiches : {counts(result.items)}.
          {#if result.problems.length}<ul class="notes">{#each result.problems.slice(0, 20) as p}<li>{p.name || p.id} ({SEC(p.section)}) : {p.message}</li>{/each}</ul>{/if}
        </div></div>
      {/if}
      {#if error}<div class="alert err">{error}</div>{/if}
      <div class="row-actions">
        <button class="btn" onclick={() => exchange(true)} disabled={busy || !importSel.length}>Essai à blanc</button>
        <button class="btn primary" onclick={() => exchange(false)} disabled={busy || !importSel.length || !result?.dry_run}>{#if busy}<span class="spin"></span>{/if}Appliquer</button>
      </div>
    {:else if error}<div class="alert err">{error}</div>{/if}
  </section>

  <section class="card">
    <div class="card-head"><div><p class="eyebrow">Retour arrière</p><h2>Instantanés du catalogue</h2></div><span class="muted small">Un état est enregistré avant chaque import ou échange ; les 20 derniers sont gardés.</span></div>
    <div class="tbl-wrap"><table class="tbl"><tbody>
      {#each snapshots as s (s.id)}
        <tr><td class="mono" style="white-space:nowrap">{dateTime(s.created_at)}</td><td>{s.reason}<br><span class="muted small">{secList(s.sections)}</span></td><td><div class="dl-actions"><button class="btn sm" onclick={() => (restoring = s)}><Icon name="undo" size="sm" />Restaurer cet état…</button></div></td></tr>
      {:else}<tr><td class="empty">Aucun instantané pour l’instant.</td></tr>{/each}
    </tbody></table></div>
  </section>
{/if}

{#if restoring}
  <Confirm title="Restaurer le catalogue dans cet état ?" eyebrow={dateTime(restoring.created_at)} confirmLabel="Restaurer cet état" danger onconfirm={restore} onclose={() => (restoring = null)}>
    <p>{secList(restoring.sections)} reprennent leur état du {dateTime(restoring.created_at)} ; les fiches ajoutées ou modifiées depuis sont remplacées.</p>
    <p class="muted small">L’état actuel est d’abord enregistré dans un nouvel instantané : cette restauration peut elle aussi être annulée.</p>
  </Confirm>
{/if}
