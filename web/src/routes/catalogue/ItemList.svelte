<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import SortTh, { type Sort } from '../../components/SortTh.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import HeadActions from '../../components/HeadActions.svelte';
  import { pageHead, plural } from '../../lib/pagehead.svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { navigate } from '../../lib/router.svelte';
  import { ITEM_KINDS, refList, refName, fnColor, summary, type CatalogItem, type ItemPath } from '../../lib/catalog.svelte';

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

  async function bulkDelete() {
    try {
      const r = await api.post<{ deleted: number; kept: { name: string; used_by: { name: string }[] }[] }>('/catalog/items/delete', { ids: [...selected] });
      toast(`${r.deleted} fiche${r.deleted > 1 ? 's' : ''} supprimée${r.deleted > 1 ? 's' : ''}.`);
      if (r.kept.length) toast(`${r.kept.length} fiche(s) gardée(s) car utilisée(s) : ${r.kept.map((k) => k.name).join(', ')}.`, 'warn');
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
<div class="toolbar" style="display:flex;gap:10px;flex-wrap:wrap;align-items:center">
  <label class="search" style="flex:1;min-width:220px"><Icon name="search" /><input class="input" bind:value={q} oninput={search} placeholder="Nom ou référence, sans se soucier des accents…" aria-label="Rechercher"></label>
  {#if kind !== 'adapters'}
    <select class="input" style="width:auto" bind:value={brand} aria-label="Marque"><option value="">Toutes les marques</option>{#each refList('brands') as b}<option value={b.id}>{b.name}</option>{/each}</select>
  {/if}
  <div class="seg" role="group" aria-label="État">{#each [['true', 'Actives'], ['false', 'Inactives'], ['', 'Toutes']] as [v, l]}<button class:on={active === v} onclick={() => (active = v as typeof active)}>{l}</button>{/each}</div>
</div>
{/if}

{#if selected.size}
  <div class="alert info" style="align-items:center"><b>{selected.size} sélectionnée{selected.size > 1 ? 's' : ''}</b><span style="flex:1"></span><button class="btn sm" onclick={() => (selected = new Set())}>Désélectionner</button><button class="btn sm danger" onclick={() => (confirmDelete = true)}><Icon name="trash" size="sm" />Supprimer</button></div>
{/if}

{#if !items.length}
  {#if loading}<span class="spin"></span>
  {:else if q.trim() || brand || active !== 'true'}<EmptyState icon="search" message="Aucune fiche ne correspond à ces critères." hint="Modifiez la recherche ou les filtres." />
  {:else}<EmptyState icon="box" message="Aucune fiche pour l’instant." hint={writable ? 'Créez-en une, ou importez un tableur.' : ''}>{#snippet action()}{#if writable}<a class="btn primary" href="/catalogue/{kind}/nouveau"><Icon name="plus" />{meta.newLabel}</a>{/if}{/snippet}</EmptyState>{/if}
{:else}
<section class="card" style="padding:6px 18px 10px">
  <div class="tbl-wrap">
    <table class="tbl">
      <thead><tr>
        {#if writable}<th style="width:28px"><label class="cb-hit"><input type="checkbox" checked={allOn} onchange={toggleAll} aria-label="Tout sélectionner"></label></th>{/if}
        <SortTh label={meta.one} key="name" bind:sort />
        {#if kind !== 'adapters'}<SortTh label="Marque" key="brand" bind:sort cls="hide-m" />{:else}<th class="hide-m">Marque</th>{/if}
        <th class="hide-m">Caractéristiques</th><SortTh label="Fonction" key="fn" bind:sort cls="hide-m" /><th><span class="sr-only">Actions</span></th>
      </tr></thead>
      <tbody>
        {#each sorted as i (i.id)}
          <tr class="click" onclick={(e) => { if (!(e.target as Element).closest('input,button,a,label')) navigate(`/catalogue/${kind}/${i.id}`); }}>
            {#if writable}<td><label class="cb-hit"><input type="checkbox" checked={selected.has(i.id)} onchange={() => toggle(i.id)} aria-label="Sélectionner {i.name}"></label></td>{/if}
            <td><a href="/catalogue/{kind}/{i.id}" style="text-decoration:none"><b>{i.name}</b></a>{#if !i.active} <span class="chip">inactive</span>{/if}{#if i.reference}<br><span class="mono small muted">{i.reference}</span>{/if}<span class="show-m small muted">{[refName('brands', i.brand_id), summary(i)].filter(Boolean).join(' · ')}</span>{#if fns(i).length}<span class="show-m fn-m">{#each fns(i) as f}<span class="chip" style="background:color-mix(in srgb,{fnColor(f)} 14%,transparent);color:{fnColor(f)}">{refName('functions', f)}</span>{/each}</span>{/if}</td>
            <td class="hide-m">{refName('brands', i.brand_id) || '—'}</td>
            <td class="small hide-m">{summary(i)}</td>
            <td class="hide-m"><div style="display:flex;gap:6px;flex-wrap:wrap">{#each fns(i) as f}<span class="chip" style="background:color-mix(in srgb,{fnColor(f)} 14%,transparent);color:{fnColor(f)}">{refName('functions', f)}</span>{/each}</div></td>
            <td class="num">{#if writable}<button class="btn sm ghost dup row-act" title="Dupliquer la fiche" aria-label="Dupliquer {i.name}" onclick={() => duplicate(i)}><Icon name="copy" size="sm" /></button>{/if}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  {#if items.length < total}<div style="display:flex;justify-content:center;padding:12px"><button class="btn" onclick={() => load(true)} disabled={loading}>Afficher plus ({items.length} sur {total})</button></div>{/if}
</section>
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
  /* Téléphone : cases et icônes à 40 px au moins (cible tactile). */
  @media (max-width: 640px) {
    .cb-hit { width: 40px; height: 40px; margin: -8px -6px; }
    .cb-hit input { width: 20px; height: 20px; }
    .dup { min-width: 40px; min-height: 40px; }
    .fn-m.show-m { display: flex; flex-wrap: wrap; }
  }
</style>
