<script lang="ts">
  // Bibliothèque des modèles d’éléments (section 6.1).
  import Icon from '../../components/Icon.svelte';
  import Vu from '../../components/Vu.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { nf, relative } from '../../lib/format';
  import { CONTAINER_KINDS } from '../../lib/projects';

  const STATUS: Record<string, { label: string; chip: string }> = { draft: { label: 'Brouillon', chip: 'info' }, inactive: { label: 'Inactif', chip: '' }, active: { label: 'Actif', chip: 'ok' } };
  let list = $state<any[]>([]);
  let q = $state('');
  let f = $state<'' | 'draft' | 'inactive' | 'active'>('');
  let sort = $state<'name' | 'date'>('name');
  let detail = $state<any | null>(null);
  let renaming = $state<any | null>(null);
  let newName = $state('');
  let deleting = $state<any | null>(null);
  const writable = $derived(can('catalogue', 'ecriture'));

  async function load() { try { list = (await api.get<{ templates: any[] }>('/templates')).templates; } catch (e) { fail(e); } }
  load();
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const shown = $derived(list.filter((t) => (!f || t.status === f) && (!q || norm(t.name).includes(norm(q)))).sort((a, b) => (sort === 'name' ? a.name.localeCompare(b.name, 'fr') : b.updated_at.localeCompare(a.updated_at))));

  async function setStatus(t: any, status: string) {
    try { await api.patch(`/templates/${t.id}`, { rev: t.rev, status }); toast(`« ${t.name} » : ${STATUS[status]!.label.toLowerCase()}.`); await load(); } catch (e) { fail(e); await load(); }
  }
  async function rename(e: SubmitEvent) {
    e.preventDefault();
    try { await api.patch(`/templates/${renaming.id}`, { rev: renaming.rev, name: newName }); renaming = null; await load(); } catch (x) { fail(x); }
  }
  async function remove() { try { await api.del(`/templates/${deleting.id}`); toast('Modèle supprimé.'); deleting = null; await load(); } catch (e) { fail(e); } }
  async function open(t: any) { try { detail = await api.get(`/templates/${t.id}`); } catch (e) { fail(e); } }
</script>

<p class="muted" style="max-width:70ch">Un modèle garde un élément et tout son contenu : matériel, accessoires, circuits et disposition dans la baie. Créez-en un depuis l’inventaire d’un projet (« Enregistrer comme modèle »), activez-le ici, puis ajoutez-le à n’importe quel projet. Seuls les modèles actifs sont proposés.</p>
<div class="toolbar" style="display:flex;gap:10px;flex-wrap:wrap;align-items:center">
  <label class="search" style="flex:1;min-width:220px"><Icon name="search" /><input class="input" bind:value={q} placeholder="Nom du modèle…" aria-label="Rechercher un modèle"></label>
  <div class="seg">{#each [['', 'Tous'], ['active', 'Actifs'], ['draft', 'Brouillons'], ['inactive', 'Inactifs']] as [k, l]}<button class:on={f === k} onclick={() => (f = k as typeof f)}>{l}</button>{/each}</div>
  <div class="seg"><button class:on={sort === 'name'} onclick={() => (sort = 'name')}>A → Z</button><button class:on={sort === 'date'} onclick={() => (sort = 'date')}>Récents</button></div>
</div>

<section class="card" style="padding:6px 18px 10px">
  <div class="tbl-wrap"><table class="tbl">
    <thead><tr><th>Modèle</th><th>Statut</th><th>Occupation</th><th class="num">Poids</th><th class="num">Conso</th><th></th></tr></thead>
    <tbody>
      {#each shown as t (t.id)}
        {@const s = t.summary}
        <tr class="click" onclick={(e) => { if (!(e.target as Element).closest('button,select')) open(t); }}>
          <td><b>{t.name}</b>{#if t.cable_digit != null} <span class="chip mono" title="Identifiant câble">#{t.cable_digit}</span>{/if}<br><span class="muted small">{CONTAINER_KINDS[t.container.kind as keyof typeof CONTAINER_KINDS]?.label}{t.container.rackable ? ` · ${t.container.height_u} U · ${t.container.depth_mm} mm` : ''} · modifié {relative(t.updated_at)}</span></td>
          <td>{#if writable}<select class="input" style="height:36px;width:auto" value={t.status} onchange={(e) => setStatus(t, (e.target as HTMLSelectElement).value)} aria-label="Statut de {t.name}">{#each Object.entries(STATUS) as [k, v]}<option value={k}>{v.label}</option>{/each}</select>{:else}<span class="chip {STATUS[t.status]?.chip}">{STATUS[t.status]?.label}</span>{/if}</td>
          <td style="min-width:170px">{#if t.container.rackable}<div class="small muted">Avant {s.front_pct} % · arrière {s.rear_pct} %</div><Vu value={s.front_pct} mode="load" n={18} />{:else}<span class="muted">—</span>{/if}</td>
          <td class="num" class:urg={s.over_weight}>{nf(s.weight_kg, 1)} kg</td>
          <td class="num" class:urg={s.over_power}>{nf(s.power_w)} W</td>
          <td>{#if writable}<div class="dl-actions"><button class="btn sm ghost" title="Renommer" onclick={() => { renaming = t; newName = t.name; }}><Icon name="pen" size="sm" /></button><button class="btn sm ghost danger" title="Supprimer" onclick={() => (deleting = t)}><Icon name="trash" size="sm" /></button></div>{/if}</td>
        </tr>
      {:else}<tr><td colspan="6" class="empty">Aucun modèle. Enregistrez un élément comme modèle depuis l’inventaire d’un projet.</td></tr>{/each}
    </tbody>
  </table></div>
</section>

{#if detail}
  <Dialog title={detail.name} eyebrow="Modèle d’élément" onclose={() => (detail = null)}>
    <dl class="kv"><dt>Type</dt><dd>{CONTAINER_KINDS[detail.container.kind as keyof typeof CONTAINER_KINDS]?.label}</dd><dt>Format</dt><dd>{detail.container.rackable ? `${detail.container.height_u} U · ${detail.container.depth_mm} mm` : 'Non rackable'}</dd><dt>Poids</dt><dd>{nf(detail.summary.weight_kg, 1)} kg</dd><dt>Consommation</dt><dd>{nf(detail.summary.power_w)} W</dd></dl>
    {#if detail.summary.missing_items}<div class="alert warn"><Icon name="alert" /><span>{detail.summary.missing_items} fiche(s) n’existent plus au catalogue : elles seront ignorées à la copie.</span></div>{/if}
    <ul class="notes">{#each detail.articles as a}<li>{a.quantity} × {a.label}</li>{:else}<li>Élément vide.</li>{/each}</ul>
  </Dialog>
{/if}
{#if renaming}
  <Dialog title="Renommer le modèle" eyebrow={renaming.name} onclose={() => (renaming = null)}>
    <form id="tplf" onsubmit={rename}><div class="field"><label for="tpl-n">Nom</label><input class="input" id="tpl-n" bind:value={newName} required maxlength="120"></div></form>
    {#snippet footer()}<button class="btn ghost" onclick={() => (renaming = null)}>Annuler</button><button class="btn primary" type="submit" form="tplf">Enregistrer</button>{/snippet}
  </Dialog>
{/if}
{#if deleting}
  <Confirm title="Supprimer « {deleting.name} » ?" eyebrow="Modèle" confirmLabel="Supprimer" danger onconfirm={remove} onclose={() => (deleting = null)}>
    <p>Les éléments déjà copiés dans des projets ne changent pas.</p>
  </Confirm>
{/if}
