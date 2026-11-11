<script lang="ts">
  // Ajouter du matériel : une fiche du catalogue (équipement, adaptateur) ou un accessoire.
  import Icon from '../../components/Icon.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import { api, ApiError } from '../../lib/api';
  import { toast, fail } from '../../lib/toast.svelte';
  import { loadRefs, refName, summary, type CatalogItem } from '../../lib/catalog.svelte';
  import type { Container, ProjectView } from '../../lib/projects';

  let { view, container, locationId, onclose, ondone }: { view: ProjectView; container: Container | null; locationId: string; onclose: () => void; ondone: () => void } = $props();
  type Tab = 'equipment' | 'adapters' | 'accessory';
  let tab = $state<Tab>('equipment');
  let q = $state('');
  let items = $state<CatalogItem[]>([]);
  let qty = $state<Record<string, number>>({});
  let busy = $state('');
  let acc = $state<{ kind: 'plate' | 'shelf' | 'drawer'; variant: 'blank' | 'brush' | 'vented'; depth_mm: number; height_u: number }>({ kind: 'plate', variant: 'blank', depth_mm: 450, height_u: 1 });
  let timer: ReturnType<typeof setTimeout>;
  loadRefs();

  async function search() {
    if (tab === 'accessory') return;
    const p = new URLSearchParams({ kind: tab, active: 'true', limit: '60' });
    if (q.trim()) p.set('q', q.trim());
    try { items = (await api.get<{ items: CatalogItem[] }>(`/catalog/items?${p}`)).items; } catch (e) { fail(e); }
  }
  $effect(() => { tab; search(); });
  const where = $derived(container ? container.name : `${view.locations.find((l) => l.id === locationId)?.name} (équipements libres)`);

  async function add(body: object, key: string) {
    busy = key;
    try {
      const r = await api.post<{ merged: boolean; warnings: string[]; article: { quantity: number } }>(`/projects/${view.project.id}/articles`, { container_id: container?.id ?? null, location_id: locationId, ...body });
      toast(r.merged ? `Quantité portée à ${r.article.quantity}.` : 'Ajouté.');
      for (const w of r.warnings) toast(w, 'warn');
      ondone();
    } catch (e) { fail(e instanceof ApiError ? e : e); } finally { busy = ''; }
  }
  const addItem = (i: CatalogItem) => add({ nature: tab === 'adapters' ? 'adapter' : 'equipment', item_id: i.id, quantity: qty[i.id] ?? 1 }, i.id);
  const addAcc = () => add({ nature: 'accessory', accessory: acc.kind === 'plate' ? { kind: 'plate', variant: acc.variant, height_u: acc.height_u } : { kind: acc.kind, depth_mm: acc.depth_mm, height_u: acc.height_u } }, 'acc');
</script>

<Dialog title="Ajouter du matériel" eyebrow={where} {onclose} wide>
  <div class="seg">{#each [['equipment', 'Équipements'], ['adapters', 'Adaptateurs'], ['accessory', 'Accessoires']] as [k, l]}<button class:on={tab === k} disabled={k === 'accessory' && !container?.rackable} onclick={() => (tab = k as Tab)}>{l}</button>{/each}</div>
  {#if tab !== 'accessory'}
    <label class="search"><Icon name="search" /><input class="input" bind:value={q} oninput={() => { clearTimeout(timer); timer = setTimeout(search, 200); }} placeholder="Nom ou référence…" aria-label="Rechercher dans le catalogue"></label>
    <div class="tbl-wrap" style="max-height:52vh;overflow:auto">
      <table class="tbl"><tbody>
        {#each items as i (i.id)}
          <tr>
            <td><b>{i.name}</b><br><span class="muted small">{[refName('brands', i.brand_id), summary(i)].filter(Boolean).join(' · ')}</span></td>
            <td style="width:90px"><input class="input mono" type="number" min="1" max="999" style="height:38px" value={qty[i.id] ?? 1} oninput={(e) => (qty[i.id] = Math.max(1, Number((e.target as HTMLInputElement).value) || 1))} aria-label="Quantité de {i.name}"></td>
            <td style="width:110px"><button class="btn sm primary" onclick={() => addItem(i)} disabled={!!busy}>{#if busy === i.id}<span class="spin"></span>{:else}<Icon name="plus" size="sm" />{/if}Ajouter</button></td>
          </tr>
        {:else}<tr><td class="empty">{q ? 'Aucune fiche ne correspond.' : 'Le catalogue est vide pour ce genre.'} <a class="ilink" href="/catalogue/{tab}/nouveau">Créer une fiche</a></td></tr>{/each}
      </tbody></table>
    </div>
  {:else}
    <div class="stack">
      <div class="seg">{#each [['plate', 'Plaque'], ['shelf', 'Étagère'], ['drawer', 'Tiroir']] as [k, l]}<button class:on={acc.kind === k} onclick={() => (acc.kind = k as typeof acc.kind)}>{l}</button>{/each}</div>
      <div class="fg3">
        <div class="field"><label for="ac-u">Hauteur</label><select class="input" id="ac-u" bind:value={acc.height_u}>{#each [1, 2, 3, 4, 5, 6, 7, 8] as u}<option value={u}>{u} U</option>{/each}</select></div>
        {#if acc.kind === 'plate'}<div class="field"><label for="ac-v">Variante</label><select class="input" id="ac-v" bind:value={acc.variant}><option value="blank">Pleine</option><option value="brush">Passe-câble</option><option value="vented">Aérée</option></select></div>
        {:else}<div class="field"><label for="ac-d">Profondeur</label><select class="input" id="ac-d" bind:value={acc.depth_mm}>{#each acc.kind === 'drawer' ? [200, 250, 300, 350, 400, 450, 500] : [200, 300, 350, 400, 450, 500, 600, 700, 800, 1000, 1200] as d}<option value={d}>{d} mm</option>{/each}</select></div>{/if}
      </div>
      <div class="row-actions"><button class="btn primary" onclick={addAcc} disabled={!!busy}><Icon name="plus" />Ajouter l’accessoire</button></div>
    </div>
  {/if}
</Dialog>
