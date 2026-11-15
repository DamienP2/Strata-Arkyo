<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import SortTh, { type Sort } from '../../components/SortTh.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Loading from '../../components/Loading.svelte';
  import Seg from '../../components/Seg.svelte';
  import HeadActions from '../../components/HeadActions.svelte';
  import { pageHead, plural } from '../../lib/pagehead.svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { navigate } from '../../lib/router.svelte';
  import { ITEM_KINDS, refList, refName, familyColor, summary, type CatalogItem, type ItemPath } from '../../lib/catalog.svelte';
  import { pl } from '../../lib/format';

  let { kind }: { kind: ItemPath } = $props();
  const meta = $derived(ITEM_KINDS.find((k) => k.path === kind)!);
  const PAGE = 100;

  let q = $state('');
  let active = $state<'' | 'true' | 'false'>('true');
  let brand = $state('');
  let items = $state<CatalogItem[]>([]);
  let total = $state(0);
  let loading = $state(true);
  let selected = $state<Set<string>>(new Set());
  let confirmDelete = $state(false);
  let timer: ReturnType<typeof setTimeout>;

  async function load(append = false) {
    loading = true;
    const p = new URLSearchParams({ kind, limit: String(PAGE), offset: String(append ? items.length : 0) });
    if (q.trim()) p.set('q', q.trim());
    if (active) p.set('active', active);
    if (brand) p.set('brand_id', brand);
    try {
      const r = await api.get<{ items: CatalogItem[]; total: number }>(`/catalog/items?${p}`);
      items = append ? [...items, ...r.items] : r.items; total = r.total;
    } catch (e) { fail(e); } finally { loading = false; }
  }
  $effect(() => { active; brand; load(); });
  // Tri des colonnes : sur la liste complète (on charge le reste si besoin, 1000 fiches au plus).
  let sort = $state<Sort>({ key: 'name', dir: 1 });
  $effect(() => { if ((sort.key !== 'name' || sort.dir !== 1) && !loading && items.length < total) loadAll(); });
  async function loadAll() {
    loading = true;
    const p = new URLSearchParams({ kind, limit: '1000', offset: '0' });
    if (q.trim()) p.set('q', q.trim());
    if (active) p.set('active', active);
    if (brand) p.set('brand_id', brand);
    try { const r = await api.get<{ items: CatalogItem[]; total: number }>(`/catalog/items?${p}`); items = r.items; total = r.total; } catch (e) { fail(e); } finally { loading = false; }
  }
  const sortVal = (i: CatalogItem): string => sort.key === 'brand' ? refName('brands', i.brand_id) || '' : sort.key === 'fn' ? fns(i).map((f) => refName('functions', f)).join(', ') : i.name;
  // Valeurs vides toujours en fin de liste, quel que soit le sens.
  const cmp = (a: CatalogItem, b: CatalogItem) => { const x = sortVal(a), y = sortVal(b); return !x !== !y ? (x ? -1 : 1) : sort.dir * x.localeCompare(y, 'fr', { numeric: true, sensitivity: 'base' }); };
  const sorted = $derived(sort.key === 'name' && sort.dir === 1 ? items : [...items].sort(cmp));
  const noun = $derived(meta.label.toLowerCase());
  const countLabel = $derived(loading && !items.length ? '' : plural(total, meta.one.toLowerCase(), noun) + (q.trim() || brand ? (total > 1 ? ' trouvés' : ' trouvé') : ''));
  function search() { clearTimeout(timer); timer = setTimeout(() => load(), 250); }

  function toggle(id: string) { const s = new Set(selected); s.has(id) ? s.delete(id) : s.add(id); selected = s; }
  const allOn = $derived(items.length > 0 && items.every((i) => selected.has(i.id)));
  function toggleAll() { selected = allOn ? new Set() : new Set(items.map((i) => i.id)); }

  // Désactiver ou réactiver la sélection : chaque fiche est réenregistrée avec son nouvel état.
  let bulkBusy = $state(false);
  const selItems = $derived(items.filter((i) => selected.has(i.id)));
  async function bulkActive(on: boolean) {
    bulkBusy = true;
    const list = selItems.filter((i) => i.active !== on);
    let done = 0;
    for (const i of list) {
      try { await api.put(`/catalog/items/${i.id}`, { rev: i.rev, data: { ...i.data, active: on } }); done++; } catch { /* compté plus bas */ }
    }
    bulkBusy = false;
    const failed = list.length - done;
    if (done) toast(`${done} fiche${pl(done)} ${on ? 'réactivée' : 'désactivée'}${pl(done)}.`);
    if (failed) toast(`${failed} fiche${pl(failed)} non modifiée${pl(failed)} : ouvrez-la${failed > 1 ? 's' : ''} pour corriger.`, 'warn');
    if (!list.length) toast(on ? 'Ces fiches sont déjà actives.' : 'Ces fiches sont déjà inactives.', 'info');
    selected = new Set(); load();
  }
  async function bulkDelete() {
    try {
      const r = await api.post<{ deleted: number; kept: { name: string; used_by: { name: string }[] }[] }>('/catalog/items/delete', { ids: [...selected] });
      toast(`${r.deleted} fiche${r.deleted > 1 ? 's' : ''} supprimée${r.deleted > 1 ? 's' : ''}.`);
      if (r.kept.length) toast(`${r.kept.length} fiche${pl(r.kept.length)} gardée${pl(r.kept.length)} car utilisée${pl(r.kept.length)} : ${r.kept.map((k) => k.name).join(', ')}.`, 'warn');
      selected = new Set(); confirmDelete = false; load();
    } catch (e) { fail(e); }
  }
  async function duplicate(i: CatalogItem) {
    try { const r = await api.post<{ item: CatalogItem }>(`/catalog/items/${i.id}/duplicate`); toast('Fiche dupliquée.'); navigate(`/catalogue/${kind}/${r.item.id}`); } catch (e) { fail(e); }
  }
  const fns = (i: CatalogItem): string[] => i.kind === 'equipment' ? (i.data.functions ?? []) : i.data.function ? [i.data.function] : [];
  const writable = $derived(can('catalogue', 'ecriture'));
  /** Catalogue vide (sans recherche ni filtre) : la seule action est celle de l’état vide. */
  const bare = $derived(total === 0 && !loading && !q.trim() && !brand && active === 'true');
  pageHead(() => ({ count: bare ? '' : countLabel }));
</script>

{#if writable && !bare}<HeadActions><a class="btn primary" href="/catalogue/{kind}/nouveau"><Icon name="plus" />{meta.newLabel}</a></HeadActions>{/if}

{#if !bare}
<div class="toolbar">
  <label class="search" style="flex:1;min-width:220px"><Icon name="search" /><input class="input" type="search" bind:value={q} oninput={search} placeholder="Nom ou référence, sans se soucier des accents" aria-label="Rechercher"></label>
  {#if kind !== 'adapters'}
    <select class="input" style="width:auto" bind:value={brand} aria-label="Marque"><option value="">Toutes les marques</option>{#each refList('brands') as b}<option value={b.id}>{b.name}</option>{/each}</select>
  {/if}
  <Seg label="État" bind:value={active} options={[{ value: 'true', label: 'Actifs' }, { value: 'false', label: 'Inactifs' }, { value: '', label: 'Tous' }]} />
</div>
{/if}

{#if !items.length}
  {#if loading}<Loading label="Chargement des fiches" />
  {:else if q.trim() || brand || active !== 'true'}<EmptyState icon="search" message="Aucune fiche ne correspond à ces critères." hint="Modifiez la recherche ou les filtres." />
  {:else}<EmptyState icon="box" message="Aucune fiche pour l’instant." hint={writable ? 'Créez-en une, ou importez un tableur.' : ''}>{#snippet action()}{#if writable}<a class="btn primary" href="/catalogue/{kind}/nouveau"><Icon name="plus" />{meta.newLabel}</a>{/if}{/snippet}</EmptyState>{/if}
{:else}
<section class="card tbl-card">
  <div class="tbl-wrap">
    <table class="tbl">
      <thead><tr>
        {#if writable}<th class="ck"><label class="cb-hit"><input type="checkbox" checked={allOn} onchange={toggleAll} aria-label="Tout sélectionner"></label></th>{/if}
        <SortTh label={meta.one} key="name" bind:sort />
        {#if kind !== 'adapters'}<SortTh label="Marque" key="brand" bind:sort cls="hide-m" />{:else}<th class="hide-m">Marque</th>{/if}
        <th class="hide-m">Caractéristiques</th><SortTh label="Fonction" key="fn" bind:sort cls="hide-m" /><th><span class="sr-only">Actions</span></th>
      </tr></thead>
      <tbody>
        {#each sorted as i (i.id)}
          <tr class="click" class:sel={selected.has(i.id)} onclick={(e) => { if (!(e.target as Element).closest('input,button,a,label')) navigate(`/catalogue/${kind}/${i.id}`); }}>
            {#if writable}<td class="ck"><label class="cb-hit"><input type="checkbox" checked={selected.has(i.id)} onchange={() => toggle(i.id)} aria-label="Sélectionner {i.name}"></label></td>{/if}
            <td><a class="cell-link" href="/catalogue/{kind}/{i.id}">{i.name}</a>{#if !i.active} <span class="chip">inactif</span>{/if}{#if i.reference}<br><span class="mono small muted">{i.reference}</span>{/if}<span class="show-m small muted">{[refName('brands', i.brand_id), summary(i)].filter(Boolean).join(' · ')}</span>{#if fns(i).length}<span class="show-m fn-m">{#each fns(i) as f}<span class="chip fn"><i class="dot" style="--c:{familyColor(f, { screen: true })}"></i>{refName('functions', f)}</span>{/each}</span>{/if}</td>
            <td class="hide-m">{refName('brands', i.brand_id) || '—'}</td>
            <td class="small hide-m">{summary(i)}</td>
            <td class="hide-m"><div style="display:flex;gap:6px;flex-wrap:wrap">{#each fns(i) as f}<span class="chip fn"><i class="dot" style="--c:{familyColor(f, { screen: true })}"></i>{refName('functions', f)}</span>{/each}</div></td>
            <td class="acts">{#if writable}<button class="icon-btn sm dup" title="Dupliquer {i.name}" aria-label="Dupliquer {i.name}" onclick={() => duplicate(i)}><Icon name="copy" size="sm" /></button>{/if}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  {#if items.length < total}<div class="tbl-more"><button class="btn" onclick={() => load(true)} disabled={loading}>{#if loading}<span class="spin" aria-hidden="true"></span>{/if}Afficher plus ({items.length} sur {total})</button></div>{/if}
</section>
{/if}

<!-- Barre de sélection collée en bas de la zone : elle ne décale pas le tableau. -->
{#if selected.size}
  <div class="savebar selbar" role="region" aria-label="Sélection">
    <b class="sel-n">{selected.size} sélectionné{selected.size > 1 ? 's' : ''}</b>
    <button class="btn ghost danger" disabled={bulkBusy} onclick={() => (confirmDelete = true)}><Icon name="trash" size="sm" />Supprimer</button>
    <span class="grow"></span>
    {#if selItems.some((i) => i.active)}<button class="btn" disabled={bulkBusy} onclick={() => bulkActive(false)}>{#if bulkBusy}<span class="spin" aria-hidden="true"></span>{/if}Désactiver</button>{/if}
    {#if selItems.some((i) => !i.active)}<button class="btn" disabled={bulkBusy} onclick={() => bulkActive(true)}>{#if bulkBusy}<span class="spin" aria-hidden="true"></span>{/if}Réactiver</button>{/if}
    <button class="btn ghost" onclick={() => (selected = new Set())}>Désélectionner</button>
  </div>
{/if}

{#if confirmDelete}
  <Confirm title="Supprimer {selected.size} fiche{selected.size > 1 ? 's' : ''} ?" eyebrow={meta.label} confirmLabel="Supprimer" danger onconfirm={bulkDelete} onclose={() => (confirmDelete = false)}>
    <p>Une fiche encore utilisée par une autre (un type de câble cité par un câble, une carte placée dans un châssis) est gardée. Un instantané n’est pas pris : la suppression est définitive.</p>
  </Confirm>
{/if}

<style>
  .cb-hit { display: inline-flex; align-items: center; justify-content: center; cursor: pointer; }
  .fn-m { margin-top: 4px; }
  .fn-m .chip { margin: 0 4px 2px 0; }
  .sel-n { margin-right: 4px; white-space: nowrap; }
  .selbar { flex-wrap: wrap; }
  /* Téléphone : cases et icônes à 40 px au moins (cible tactile). */
  @media (max-width: 640px) {
    .cb-hit { width: 40px; height: 40px; margin: -8px -6px; }
    .cb-hit input { width: 20px; height: 20px; }
    .dup { min-width: 40px; min-height: 40px; }
    .fn-m.show-m { display: flex; flex-wrap: wrap; }
  }
</style>
