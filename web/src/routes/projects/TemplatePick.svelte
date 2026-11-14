<script lang="ts">
  // Ajouter un élément depuis un modèle actif de la bibliothèque.
  import Dialog from '../../components/Dialog.svelte';
  import Vu from '../../components/Vu.svelte';
  import { api } from '../../lib/api';
  import { toast, fail } from '../../lib/toast.svelte';
  import { nf, pl } from '../../lib/format';
  import { CONTAINER_KINDS, type ProjectView } from '../../lib/projects';

  let { view, locationId, onclose, ondone }: { view: ProjectView; locationId: string; onclose: () => void; ondone: (id: string) => void } = $props();
  let list = $state<any[] | null>(null);
  let busy = $state('');
  api.get<{ templates: any[] }>('/templates?status=active').then((r) => (list = r.templates)).catch((e) => { fail(e); list = []; });

  async function pick(t: any) {
    busy = t.id;
    try {
      const r = await api.post<{ container: { id: string; name: string }; skipped_items: number }>(`/projects/${view.project.id}/containers/from-template`, { template_id: t.id, location_id: locationId });
      toast(`« ${r.container.name} » ajouté depuis le modèle.`);
      if (r.skipped_items) toast(`${r.skipped_items} fiche${pl(r.skipped_items)} du modèle ${pl(r.skipped_items, 'n’existent', 'n’existe')} plus au catalogue : ${pl(r.skipped_items, 'ignorées', 'ignorée')}.`, 'warn');
      ondone(r.container.id);
    } catch (e) { fail(e); } finally { busy = ''; }
  }
</script>

<Dialog title="Ajouter depuis un modèle" eyebrow={view.locations.find((l) => l.id === locationId)?.name ?? ''} {onclose} wide>
  {#if !list}<span class="spin"></span>
  {:else if !list.length}<p class="muted">Aucun modèle actif. Enregistrez un élément comme modèle depuis l’inventaire d’un projet, puis activez-le dans Catalogue › Modèles.</p>
  {:else}
    <div class="tbl-wrap"><table class="tbl"><tbody>
      {#each list as t (t.id)}
        <tr>
          <td><b>{t.name}</b><br><span class="muted small">{CONTAINER_KINDS[t.container.kind as keyof typeof CONTAINER_KINDS]?.label}{t.container.rackable ? ` · ${t.container.height_u} U` : ''} · {t.summary.articles} article{t.summary.articles > 1 ? 's' : ''}</span></td>
          <td style="width:180px">{#if t.container.rackable}<span class="small muted">Avant {t.summary.front_pct} %</span><Vu value={t.summary.front_pct} mode="load" n={16} />{/if}</td>
          <td class="num small">{nf(t.summary.weight_kg, 1)} kg<br>{nf(t.summary.power_w)} W</td>
          <td style="width:120px"><button class="btn sm primary" onclick={() => pick(t)} disabled={!!busy}>{#if busy === t.id}<span class="spin"></span>{/if}Ajouter</button></td>
        </tr>
      {/each}
    </tbody></table></div>
  {/if}
</Dialog>
