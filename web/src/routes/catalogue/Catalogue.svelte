<script lang="ts">
  import SecNav, { type SecGroup } from '../../components/SecNav.svelte';
  import { route, navigate } from '../../lib/router.svelte';
  import { can } from '../../lib/session.svelte';
  import { ITEM_KINDS, loadRefs, refs, type ItemPath } from '../../lib/catalog.svelte';
  import ItemList from './ItemList.svelte';
  import ItemEditor from './ItemEditor.svelte';
  import Refs from './Refs.svelte';
  import Import from './Import.svelte';
  import Exchange from './Exchange.svelte';

  loadRefs();
  const seg = $derived(route.path.split('/').slice(2));
  const sub = $derived(seg[0] ?? '');
  const kind = $derived(ITEM_KINDS.find((k) => k.path === sub));
  const groups = $derived.by(() => {
    const g: SecGroup[] = [{ label: 'Fiches', items: ITEM_KINDS.map((k) => ({ key: k.path, label: k.label, href: `/catalogue/${k.path}` })) }];
    const tools = [{ key: 'referentiels', label: 'Référentiels', href: '/catalogue/referentiels' }];
    if (can('catalogue', 'ecriture')) tools.push({ key: 'import', label: 'Import', href: '/catalogue/import' });
    tools.push({ key: 'echange', label: 'Export et échange', href: '/catalogue/echange' });
    g.push({ label: 'Outils', items: tools });
    return g;
  });
  $effect(() => { if (sub === '') navigate('/catalogue/equipment', true); });
</script>

{#if seg.length >= 2 && kind}
  {#if refs.loaded}<ItemEditor kind={kind.path as ItemPath} id={seg[1] === 'nouveau' ? null : seg[1]!} />{:else}<span class="spin"></span>{/if}
{:else}
  <div class="page">
    <div class="page-head"><div><p class="eyebrow">Propre au site</p><h1 class="display">Catalogue</h1></div></div>
    <SecNav {groups} active={sub} label="Sections du catalogue" />
    {#if !refs.loaded}<span class="spin"></span>
    {:else if kind}{#key kind.path}<ItemList kind={kind.path as ItemPath} />{/key}
    {:else if sub === 'referentiels'}<Refs />
    {:else if sub === 'import'}<Import />
    {:else if sub === 'echange'}<Exchange />
    {:else}<p class="lead">Section introuvable.</p>{/if}
  </div>
{/if}
