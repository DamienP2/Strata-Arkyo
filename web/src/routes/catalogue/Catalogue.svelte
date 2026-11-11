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
  import Templates from './Templates.svelte';
  import Panels from './Panels.svelte';
  import RegieTemplates from './RegieTemplates.svelte';
  import PanelPage from './PanelPage.svelte';

  loadRefs();
  const seg = $derived(route.path.split('/').slice(2));
  const sub = $derived(seg[0] ?? '');
  const kind = $derived(ITEM_KINDS.find((k) => k.path === sub));
  const groups = $derived.by(() => {
    const g: SecGroup[] = [{ label: 'Fiches', items: ITEM_KINDS.map((k) => ({ key: k.path, label: k.label, href: `/catalogue/${k.path}` })) }];
    g.push({ label: 'Bibliothèque', items: [{ key: 'modeles', label: 'Modèles d’éléments', href: '/catalogue/modeles' }, { key: 'regies', label: 'Modèles de régies', href: '/catalogue/regies' }, { key: 'panels', label: 'Panels', href: '/catalogue/panels' }] });
    const tools = [{ key: 'referentiels', label: 'Référentiels', href: '/catalogue/referentiels' }];
    if (can('catalogue', 'ecriture')) tools.push({ key: 'import', label: 'Import', href: '/catalogue/import' });
    tools.push({ key: 'echange', label: 'Export et échange', href: '/catalogue/echange' });
    g.push({ label: 'Outils', items: tools });
    return g;
  });
  $effect(() => { if (sub === '') navigate('/catalogue/equipment', true); });
</script>

{#if sub === 'panels' && seg[1]}
  {#if refs.loaded}{#key seg[1]}<PanelPage id={seg[1]} />{/key}{:else}<span class="spin"></span>{/if}
{:else if seg.length >= 2 && kind}
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
    {:else if sub === 'modeles'}<Templates />
    {:else if sub === 'panels'}<Panels />
    {:else if sub === 'regies'}<RegieTemplates />
    {:else}<p class="lead">Section introuvable.</p>{/if}
  </div>
{/if}
