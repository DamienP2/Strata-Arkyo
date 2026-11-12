<script lang="ts">
  import Workspace from '../../components/Workspace.svelte';
  import type { SecGroup } from '../../components/SecNav.svelte';
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
  import Printing from './Printing.svelte';

  loadRefs();
  const seg = $derived(route.path.split('/').slice(2));
  const sub = $derived(seg[0] ?? '');
  const kind = $derived(ITEM_KINDS.find((k) => k.path === sub));
  const ICON: Record<string, string> = { equipment: 'cpu', cables: 'cable', 'cable-models': 'radio', adapters: 'plug' };
  const groups = $derived.by(() => {
    const g: SecGroup[] = [{ label: 'Fiches', items: ITEM_KINDS.map((k) => ({ key: k.path, label: k.label, href: `/catalogue/${k.path}`, icon: ICON[k.path] ?? 'box' })) }];
    g.push({ label: 'Modèles', items: [
      { key: 'modeles', label: 'Éléments', href: '/catalogue/modeles', icon: 'case' },
      { key: 'regies', label: 'Régies', href: '/catalogue/regies', icon: 'layers' },
      { key: 'panels', label: 'Panels de brassage', href: '/catalogue/panels', icon: 'panel' },
    ] });
    const tools = [{ key: 'referentiels', label: 'Référentiels', href: '/catalogue/referentiels', icon: 'list' }];
    if (can('catalogue', 'ecriture')) tools.push({ key: 'import', label: 'Import de tableur', href: '/catalogue/import', icon: 'upload' });
    tools.push({ key: 'echange', label: 'Export et échange', href: '/catalogue/echange', icon: 'download' });
    tools.push({ key: 'impression', label: 'Impression', href: '/catalogue/impression', icon: 'printer' });
    g.push({ label: 'Réglages', items: tools });
    return g;
  });
  const section = $derived(groups.flatMap((g) => g.items.map((i) => ({ ...i, group: g.label }))).find((i) => i.key === sub));
  $effect(() => { if (sub === '') navigate('/catalogue/equipment', true); });
</script>

<Workspace {groups} active={sub} label="Sections du catalogue" store="catalogue">
  {#snippet head(folded: boolean)}
    {#if !folded}<div class="ws-proj"><b>Catalogue</b><span>Propre au site</span></div>{/if}
  {/snippet}
  {#if sub === 'panels' && seg[1]}
    {#if refs.loaded}{#key seg[1]}<PanelPage id={seg[1]} />{/key}{:else}<span class="spin"></span>{/if}
  {:else if seg.length >= 2 && kind}
    {#if refs.loaded}<ItemEditor kind={kind.path as ItemPath} id={seg[1] === 'nouveau' ? null : seg[1]!} />{:else}<span class="spin"></span>{/if}
  {:else}
    <header class="ws-top"><div class="ws-top-main"><span class="ws-crumb">{section?.group ?? 'Catalogue'}</span><h1>{section?.label ?? 'Catalogue'}</h1></div></header>
    <div class="page">
      {#if !refs.loaded}<span class="spin"></span>
      {:else if kind}{#key kind.path}<ItemList kind={kind.path as ItemPath} />{/key}
      {:else if sub === 'referentiels'}<Refs />
      {:else if sub === 'import'}<Import />
      {:else if sub === 'echange'}<Exchange />
      {:else if sub === 'modeles'}<Templates />
      {:else if sub === 'panels'}<Panels />
      {:else if sub === 'regies'}<RegieTemplates />
      {:else if sub === 'impression'}<Printing />
      {:else}<p class="lead">Section introuvable.</p>{/if}
    </div>
  {/if}
</Workspace>
