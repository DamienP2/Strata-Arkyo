<script lang="ts">
  // Édition d'un panel de la bibliothèque.
  import Icon from '../../components/Icon.svelte';
  import PanelEditor from '../../components/PanelEditor.svelte';
  import PageHead from '../../components/PageHead.svelte';
  import Loading from '../../components/Loading.svelte';
  import { registerLeaveGuard } from '../../lib/router.svelte';
  import { untrack } from 'svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import type { PanelData } from '../../lib/panel';

  let { id }: { id: string } = $props();
  let lib = $state<{ id: string; panel: PanelData; rev: number } | null>(null);
  let editor = $state<{ isDirty: () => boolean } | null>(null);
  // Quitter la page avec un panel modifié non enregistré : on demande.
  $effect(() => registerLeaveGuard(() => editor?.isDirty() ?? false));
  untrack(() => api.get<{ id: string; panel: PanelData; rev: number }>(`/panels/${id}`)).then((r) => (lib = r), fail);

  async function save(p: PanelData) {
    const r = await api.put<{ panel: PanelData; rev: number }>(`/panels/${id}`, { rev: lib!.rev, panel: p });
    lib!.rev = r.rev;
    toast('Panel enregistré.');
    return r.panel;
  }
</script>

<div class="page">
  <PageHead crumbs={[{ label: 'Catalogue', href: '/catalogue' }, { label: 'Panels de brassage', href: '/catalogue/panels' }, { label: lib?.panel.name ?? 'Panel' }]} title={lib?.panel.name ?? 'Panel'} />
  {#if !lib}<Loading label="Chargement du panel" />
  {:else}
    {#key lib.id}<PanelEditor bind:this={editor} initial={lib.panel} readonly={!can('catalogue', 'ecriture')} onsave={save}>
      {#snippet actions()}<a class="btn ghost" href="/catalogue/panels"><Icon name="chevr" size="sm" />Retour à la bibliothèque</a>{/snippet}
    </PanelEditor>{/key}
  {/if}
</div>
