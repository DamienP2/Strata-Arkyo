<script lang="ts">
  // Bibliothèque des modèles d’éléments (section 6.1).
  import Icon from '../../components/Icon.svelte';
  import Vu from '../../components/Vu.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Loading from '../../components/Loading.svelte';
  import Seg from '../../components/Seg.svelte';
  import SortTh, { type Sort } from '../../components/SortTh.svelte';
  import { ask } from '../../lib/ask.svelte';
  import { pageHead, plural } from '../../lib/pagehead.svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { nf, relative, pl } from '../../lib/format';
  import { CONTAINER_KINDS } from '../../lib/projects';

  const STATUS: Record<string, { label: string; chip: string }> = { draft: { label: 'Brouillon', chip: 'info' }, inactive: { label: 'Inactif', chip: '' }, active: { label: 'Actif', chip: 'ok' } };
  let list = $state<any[]>([]);
  let loaded = $state(false);
  let q = $state('');
  let f = $state<'' | 'draft' | 'inactive' | 'active'>('');
  let sort = $state<Sort>({ key: 'name', dir: 1 });
  let detail = $state<any | null>(null);
  let renaming = $state<any | null>(null);
  let newName = $state('');
  let renameOrig = '';
  const writable = $derived(can('catalogue', 'ecriture'));

  async function load() { try { list = (await api.get<{ templates: any[] }>('/templates')).templates; loaded = true; } catch (e) { fail(e); } }
  load();
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  // Tri par colonne : Modèle, Poids, Conso.
  const sortVal = (t: any): number | string => sort.key === 'weight' ? t.summary.weight_kg ?? 0 : sort.key === 'power' ? t.summary.power_w ?? 0 : t.name;
  const cmp = (a: any, b: any) => { const x = sortVal(a), y = sortVal(b); return sort.dir * (typeof x === 'number' && typeof y === 'number' ? x - y : String(x).localeCompare(String(y), 'fr', { numeric: true })); };
  const shown = $derived(list.filter((t) => (!f || t.status === f) && (!q || norm(t.name).includes(norm(q)))).sort(cmp));

  pageHead(() => ({ count: list.length ? plural(list.length, 'modèle d’élément', 'modèles d’élément') : '' }));
  async function setStatus(t: any, status: string) {
    try { await api.patch(`/templates/${t.id}`, { rev: t.rev, status }); await load(); } catch (e) { fail(e); await load(); }
  }
  function startRename(t: any) { renaming = t; newName = t.name; renameOrig = t.name; }
  async function rename() {
    await api.patch(`/templates/${renaming.id}`, { rev: renaming.rev, name: newName });
    toast(`« ${newName.trim()} » renommé.`); renaming = null; await load();
  }
  async function remove(t: any) {
    if (!(await ask({ title: `Supprimer « ${t.name} » ?`, eyebrow: 'Modèle d’élément', text: 'Les éléments déjà copiés dans des projets ne changent pas.', confirmLabel: 'Supprimer', danger: true, run: () => api.del(`/templates/${t.id}`) }))) return;
    toast(`« ${t.name} » supprimé.`); await load();
  }
  async function open(t: any) { try { detail = await api.get(`/templates/${t.id}`); } catch (e) { fail(e); } }
</script>

<p class="muted" style="max-width:70ch">Un modèle garde un élément et tout son contenu : matériel, accessoires, circuits et disposition dans le rack. Créez-en un depuis l’inventaire d’un projet (« Enregistrer comme modèle »), activez-le ici, puis ajoutez-le à n’importe quel projet. Seuls les modèles actifs sont proposés.</p>
{#if !loaded}<Loading label="Chargement des modèles" />
{:else if !list.length}
  <EmptyState icon="case" message="Aucun modèle d’élément pour l’instant." hint="Enregistrez un élément comme modèle depuis l’inventaire d’un projet." />
{:else}
<div class="toolbar">
  <label class="search" style="flex:1;min-width:220px;max-width:480px"><Icon name="search" /><input class="input" type="search" bind:value={q} placeholder="Nom du modèle" aria-label="Rechercher un modèle"></label>
  <Seg label="Statut" bind:value={f} options={[{ value: '', label: 'Tous' }, { value: 'active', label: 'Actifs' }, { value: 'draft', label: 'Brouillons' }, { value: 'inactive', label: 'Inactifs' }]} />
</div>

<section class="card tbl-card">
  <div class="tbl-wrap"><table class="tbl">
    <thead><tr><SortTh label="Modèle" key="name" bind:sort /><th>Statut</th><th>Occupation</th><SortTh label="Poids" key="weight" bind:sort cls="num" /><SortTh label="Conso" key="power" bind:sort cls="num" /><th><span class="sr-only">Actions</span></th></tr></thead>
    <tbody>
      {#each shown as t (t.id)}
        {@const s = t.summary}
        <tr class="click" onclick={(e) => { if (!(e.target as Element).closest('button,select')) open(t); }}>
          <td><button type="button" class="rowlink" onclick={(e) => { e.stopPropagation(); open(t); }}><b>{t.name}</b></button>{#if t.cable_digit != null} <span class="chip mono" title="Identifiant câble">#{t.cable_digit}</span>{/if}<br><span class="muted small">{CONTAINER_KINDS[t.container.kind as keyof typeof CONTAINER_KINDS]?.label}{t.container.rackable ? ` · ${t.container.height_u} U · ${t.container.depth_mm} mm` : ''} · modifié {relative(t.updated_at)}</span></td>
          <td>{#if writable}<select class="input sm" style="width:auto" value={t.status} onchange={(e) => setStatus(t, (e.target as HTMLSelectElement).value)} aria-label="Statut de {t.name}">{#each Object.entries(STATUS) as [k, v]}<option value={k}>{v.label}</option>{/each}</select>{:else}<span class="chip {STATUS[t.status]?.chip}">{STATUS[t.status]?.label}</span>{/if}</td>
          <td style="min-width:170px">{#if t.container.rackable}<div class="small muted">Avant {s.front_pct} % · arrière {s.rear_pct} %</div><Vu value={s.front_pct} mode="load" n={18} />{:else}<span class="muted">—</span>{/if}</td>
          <td class="num" class:urg={s.over_weight}>{nf(s.weight_kg, 1)} kg</td>
          <td class="num" class:urg={s.over_power}>{nf(s.power_w)} W</td>
          <td class="acts">{#if writable}<div class="dl-actions"><button class="icon-btn sm" title="Renommer {t.name}" aria-label="Renommer {t.name}" onclick={() => startRename(t)}><Icon name="pen" size="sm" /></button><button class="icon-btn sm danger" title="Supprimer {t.name}" aria-label="Supprimer {t.name}" onclick={() => remove(t)}><Icon name="trash" size="sm" /></button></div>{/if}</td>
        </tr>
      {:else}<tr><td colspan="6"><EmptyState card={false} icon="search" message="Aucun modèle ne correspond à ces critères." /></td></tr>{/each}
    </tbody>
  </table></div>
</section>
{/if}

{#if detail}
  <Dialog title={detail.name} eyebrow="Modèle d’élément" onclose={() => (detail = null)}>
    <dl class="kv"><dt>Type</dt><dd>{CONTAINER_KINDS[detail.container.kind as keyof typeof CONTAINER_KINDS]?.label}</dd><dt>Format</dt><dd>{detail.container.rackable ? `${detail.container.height_u} U · ${detail.container.depth_mm} mm` : 'Non rackable'}</dd><dt>Poids</dt><dd>{nf(detail.summary.weight_kg, 1)} kg</dd><dt>Consommation</dt><dd>{nf(detail.summary.power_w)} W</dd></dl>
    {#if detail.summary.missing_items}<div class="alert warn"><Icon name="alert" /><span>{detail.summary.missing_items} fiche{pl(detail.summary.missing_items)} {pl(detail.summary.missing_items, 'n’existent', 'n’existe')} plus au catalogue : {pl(detail.summary.missing_items, 'elles seront ignorées', 'elle sera ignorée')} à la copie.</span></div>{/if}
    <ul class="notes">{#each detail.articles as a}<li>{a.quantity} × {a.label}</li>{:else}<li>Élément vide.</li>{/each}</ul>
  </Dialog>
{/if}
{#if renaming}
  <Dialog title="Renommer le modèle" eyebrow={renaming.name} onclose={() => (renaming = null)} onsubmit={rename} submitLabel="Enregistrer" dirty={() => newName !== renameOrig}>
    <div class="field"><label for="tpl-n">Nom</label><input class="input" id="tpl-n" bind:value={newName} required maxlength="120"></div>
  </Dialog>
{/if}
