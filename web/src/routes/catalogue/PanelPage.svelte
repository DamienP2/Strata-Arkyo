<script lang="ts">
  // Édition d'un panel de la bibliothèque.
  import Icon from '../../components/Icon.svelte';
  import PanelEditor from '../../components/PanelEditor.svelte';
  import { untrack } from 'svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import type { PanelData } from '../../lib/panel';

  let { id }: { id: string } = $props();
  let lib = $state<{ id: string; panel: PanelData; rev: number } | null>(null);
  untrack(() => api.get<{ id: string; panel: PanelData; rev: number }>(`/panels/${id}`)).then((r) => (lib = r), fail);

  async function save(p: PanelData) {
    const r = await api.put<{ panel: PanelData; rev: number }>(`/panels/${id}`, { rev: lib!.rev, panel: p });
    lib!.rev = r.rev;
    toast('Panel enregistré.');
    return r.panel;
  }
</script>

<div class="page">
  <div class="page-head"><div><p class="eyebrow"><a class="crumb" href="/catalogue/panels">Catalogue › Panels</a></p><h1 class="display">{lib?.panel.name ?? 'Panel'}</h1></div></div>
  {#if !lib}<span class="spin"></span>
  {:else}
    {#key lib.id}<PanelEditor initial={lib.panel} readonly={!can('catalogue', 'ecriture')} onsave={save}>
      {#snippet actions()}<a class="btn ghost" href="/catalogue/panels"><Icon name="chevr" size="sm" />Retour à la bibliothèque</a>{/snippet}
    </PanelEditor>{/key}
  {/if}
</div>
