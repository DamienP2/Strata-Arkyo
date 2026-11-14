<script lang="ts">
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import PageHead from '../../components/PageHead.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { navigate, registerLeaveGuard } from '../../lib/router.svelte';
  import { relative } from '../../lib/format';
  import { ITEM_KINDS, blank, type CatalogItem, type Duplicate, type ItemPath } from '../../lib/catalog.svelte';
  import EquipmentForm from './forms/EquipmentForm.svelte';
  import CableForm from './forms/CableForm.svelte';
  import CableModelForm from './forms/CableModelForm.svelte';
  import AdapterForm from './forms/AdapterForm.svelte';

  let { kind, id }: { kind: ItemPath; id: string | null } = $props();
  const meta = $derived(ITEM_KINDS.find((k) => k.path === kind)!);
  const writable = $derived(can('catalogue', 'ecriture'));

  let item = $state<CatalogItem | null>(null);
  let data = $state<any>(null);
  let usedBy = $state<{ id: string; kind: CatalogItem['kind']; name: string }[]>([]);
  let errors = $state<Record<string, string>>({});
  let general = $state<string | null>(null);
  let duplicates = $state<Duplicate[]>([]);
  let saving = $state(false);
  let stale = $state(false);
  let deleting = $state(false);
  let saved = $state('');

  async function load() {
    errors = {}; general = null; stale = false;
    if (!id) { item = null; data = blank(meta.kind); saved = JSON.stringify(data); return; }
    try {
      const r = await api.get<{ item: CatalogItem; used_by: typeof usedBy }>(`/catalog/items/${id}`);
      item = r.item; usedBy = r.used_by; data = structuredClone(r.item.data); saved = JSON.stringify(data);
    } catch (e) { fail(e); navigate(`/catalogue/${kind}`, true); }
  }
  // Recharger seulement quand l'adresse change, pas quand le formulaire bouge.
  $effect(() => { id; untrack(load); });
  const dirty = $derived(data !== null && JSON.stringify(data) !== saved);

  /** Message d'erreur d'un champ (ou du premier sous-champ qui commence par ce nom). */
  function err(name: string): string | undefined {
    if (errors[name]) return errors[name];
    const k = Object.keys(errors).find((f) => f.startsWith(name + '.') || f.startsWith(name + '['));
    return k ? errors[k] : undefined;
  }

  async function save() {
    saving = true; errors = {}; general = null;
    try {
      const r = id
        ? await api.put<{ item: CatalogItem; duplicates: Duplicate[] }>(`/catalog/items/${id}`, { rev: item!.rev, data })
        : await api.post<{ item: CatalogItem; duplicates: Duplicate[] }>('/catalog/items', { kind, data });
      duplicates = r.duplicates;
      toast(id ? 'Fiche enregistrée.' : 'Fiche créée.');
      if (!id) { saved = JSON.stringify(data); navigate(`/catalogue/${kind}/${r.item.id}`, true); return; }
      item = r.item; data = structuredClone(r.item.data); saved = JSON.stringify(data);
    } catch (e) {
      if (e instanceof ApiError && e.code === 'stale') stale = true;
      else if (e instanceof ApiError && e.fields.length) { errors = Object.fromEntries(e.fields.map((f) => [f.field, f.message])); general = e.message; }
      else fail(e);
    } finally { saving = false; }
  }
  async function remove() {
    try {
      const r = await api.post<{ deleted: number; kept: unknown[] }>('/catalog/items/delete', { ids: [id] });
      if (r.deleted) { toast('Fiche supprimée.'); saved = JSON.stringify(data); navigate(`/catalogue/${kind}`); } else toast('Fiche gardée : elle est encore utilisée.', 'warn');
    } catch (e) { fail(e); } finally { deleting = false; }
  }
  async function duplicate() {
    try { const r = await api.post<{ item: CatalogItem }>(`/catalog/items/${id}/duplicate`); toast('Fiche dupliquée : vous modifiez la copie.'); navigate(`/catalogue/${kind}/${r.item.id}`); } catch (e) { fail(e); }
  }
  function leave(e: BeforeUnloadEvent) { if (dirty) e.preventDefault(); }
  // Changer de page (lien, menu, « Annuler ») avec des modifications : on demande.
  $effect(() => registerLeaveGuard(() => dirty));
</script>

<svelte:window onbeforeunload={leave} />

{#if data}
  <div class="editor">
    <PageHead crumbs={[{ label: 'Catalogue', href: '/catalogue' }, { label: meta.label, href: `/catalogue/${kind}` }, { label: id ? (item?.name ?? 'Fiche') : 'Nouvelle fiche' }]} title={data.name || (id ? item?.name ?? '' : meta.newLabel)}>
      {#snippet info()}{#if item}Modifiée {relative(item.updated_at)} · révision {item.rev}{/if}{/snippet}
      {#snippet actions()}{#if id && writable}<button class="btn" onclick={duplicate}><Icon name="copy" />Dupliquer</button><button class="btn ghost danger act-danger" onclick={() => (deleting = true)}><Icon name="trash" />Supprimer…</button>{/if}{/snippet}
    </PageHead>

    {#if stale}<div class="alert warn"><Icon name="alert" /><span style="flex:1">Quelqu’un a modifié cette fiche pendant que vous l’éditiez. Rechargez-la pour voir sa version ; vos changements non enregistrés seront perdus.</span><button class="btn sm" onclick={load}>Recharger</button></div>{/if}
    {#if duplicates.length}
      <div class="alert warn"><Icon name="alert" /><div><b>Doublon possible</b><div class="dup-list">{#each duplicates as d}<a class="ilink" href="/catalogue/{kind}/{d.id}">{d.name}{d.reference ? ` (${d.reference})` : ''} · {d.reason}</a>{/each}</div></div></div>
    {/if}
    {#if usedBy.length}<div class="alert info"><Icon name="info" /><span>Utilisée par {usedBy.map((u) => u.name).join(', ')}.</span></div>{/if}

    <fieldset disabled={!writable} style="border:0;padding:0;margin:0;display:contents">
      {#if kind === 'equipment'}<EquipmentForm bind:data {err} />
      {:else if kind === 'cables'}<CableForm bind:data {err} />
      {:else if kind === 'cable-models'}<CableModelForm bind:data {err} />
      {:else}<AdapterForm bind:data {err} />{/if}
    </fieldset>

    {#if writable}
      <div class="savebar">
        <span class="grow" class:has-err={!!general}>{#if general}<span style="color:var(--err)">{general}</span>{:else if dirty}Modifications non enregistrées.{:else}Rien à enregistrer.{/if}</span>
        <a class="btn ghost" href="/catalogue/{kind}">{dirty ? 'Annuler' : 'Retour à la liste'}</a>
        <button class="btn primary" onclick={save} disabled={saving || (!dirty && !!id)}>{#if saving}<span class="spin"></span>{/if}{id ? 'Enregistrer' : 'Créer la fiche'}</button>
      </div>
    {/if}
  </div>
{:else}<span class="spin"></span>{/if}

{#if deleting}
  <Confirm title="Supprimer cette fiche ?" eyebrow={meta.one} confirmLabel="Supprimer" danger onconfirm={remove} onclose={() => (deleting = false)}>
    <p>{usedBy.length ? 'Elle est encore utilisée : elle sera gardée.' : 'La suppression est définitive.'}</p>
  </Confirm>
{/if}
