<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
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
</script>

<div class="toolbar" style="display:flex;gap:10px;flex-wrap:wrap;align-items:center">
  <label class="search" style="flex:1;min-width:220px"><Icon name="search" /><input class="input" bind:value={q} oninput={search} placeholder="Nom ou référence, sans se soucier des accents…" aria-label="Rechercher"></label>
  {#if kind !== 'adapters'}
    <select class="input" style="width:auto" bind:value={brand} aria-label="Marque"><option value="">Toutes les marques</option>{#each refList('brands') as b}<option value={b.id}>{b.name}</option>{/each}</select>
  {/if}
  <div class="seg" role="group" aria-label="État">{#each [['true', 'Actives'], ['false', 'Inactives'], ['', 'Toutes']] as [v, l]}<button class:on={active === v} onclick={() => (active = v as typeof active)}>{l}</button>{/each}</div>
  {#if writable}<a class="btn primary" href="/catalogue/{kind}/nouveau"><Icon name="plus" />{meta.newLabel}</a>{/if}
</div>

{#if selected.size}
  <div class="alert info" style="align-items:center"><b>{selected.size} sélectionnée{selected.size > 1 ? 's' : ''}</b><span style="flex:1"></span><button class="btn sm" onclick={() => (selected = new Set())}>Désélectionner</button><button class="btn sm danger" onclick={() => (confirmDelete = true)}><Icon name="trash" size="sm" />Supprimer</button></div>
{/if}

<section class="card" style="padding:6px 18px 10px">
  <div class="tbl-wrap">
    <table class="tbl">
      <thead><tr>
        {#if writable}<th style="width:28px"><input type="checkbox" checked={allOn} onchange={toggleAll} aria-label="Tout sélectionner"></th>{/if}
        <th>{meta.one}</th><th class="hide-m">Marque</th><th class="hide-m">Caractéristiques</th><th class="hide-m">Fonction</th><th></th>
      </tr></thead>
      <tbody>
        {#each items as i (i.id)}
          <tr class="click" onclick={(e) => { if (!(e.target as Element).closest('input,button,a')) navigate(`/catalogue/${kind}/${i.id}`); }}>
            {#if writable}<td><input type="checkbox" checked={selected.has(i.id)} onchange={() => toggle(i.id)} aria-label="Sélectionner {i.name}"></td>{/if}
            <td><a href="/catalogue/{kind}/{i.id}" style="text-decoration:none"><b>{i.name}</b></a>{#if !i.active} <span class="chip">inactive</span>{/if}{#if i.reference}<br><span class="mono small muted">{i.reference}</span>{/if}<span class="show-m small muted">{[refName('brands', i.brand_id), summary(i)].filter(Boolean).join(' · ')}</span></td>
            <td class="hide-m">{refName('brands', i.brand_id) || '—'}</td>
            <td class="small hide-m">{summary(i)}</td>
            <td class="hide-m"><div style="display:flex;gap:6px;flex-wrap:wrap">{#each fns(i) as f}<span class="chip" style="background:color-mix(in srgb,{fnColor(f)} 14%,transparent);color:{fnColor(f)}">{refName('functions', f)}</span>{/each}</div></td>
            <td>{#if writable}<button class="btn sm ghost" title="Dupliquer" onclick={() => duplicate(i)}><Icon name="copy" size="sm" /></button>{/if}</td>
          </tr>
        {:else}
          <tr><td colspan="6" class="empty">{loading ? 'Chargement…' : q ? 'Aucune fiche ne correspond à cette recherche.' : `Aucune fiche pour l’instant.${writable ? ' Créez-en une, ou importez un tableur.' : ''}`}</td></tr>
        {/each}
      </tbody>
    </table>
  </div>
  {#if items.length < total}<div style="display:flex;justify-content:center;padding:12px"><button class="btn" onclick={() => load(true)} disabled={loading}>Afficher plus ({items.length} sur {total})</button></div>{/if}
</section>
<p class="muted small">{total} fiche{total > 1 ? 's' : ''}{active === 'true' ? ' active' + (total > 1 ? 's' : '') : ''}.</p>

{#if confirmDelete}
  <Confirm title="Supprimer {selected.size} fiche{selected.size > 1 ? 's' : ''} ?" eyebrow={meta.label} confirmLabel="Supprimer" danger onconfirm={bulkDelete} onclose={() => (confirmDelete = false)}>
    <p>Une fiche encore utilisée par une autre (un modèle de câble cité par un câble, une carte placée dans un châssis) est gardée. Un instantané n’est pas pris : la suppression est définitive.</p>
  </Confirm>
{/if}
