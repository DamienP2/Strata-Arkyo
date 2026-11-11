<script lang="ts">
  // Bibliothèque des panels de brassage (section 6.3) : liste, assistant de création,
  // duplication, suppression ; l'édition ouvre /catalogue/panels/{id}.
  import Icon from '../../components/Icon.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import PanelFace from '../../components/PanelFace.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { relative } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { PER_U, blankPanel, type PanelData, type PanelStats } from '../../lib/panel';

  interface Lib { id: string; panel: PanelData; rev: number; stats: PanelStats; updated_at: string }
  let list = $state<Lib[]>([]);
  let loaded = $state(false);
  let q = $state('');
  let wiz = $state<PanelData | null>(null);
  let wizErr = $state('');
  let deleting = $state<Lib | null>(null);
  const writable = $derived(can('catalogue', 'ecriture'));

  async function load() { try { list = (await api.get<{ panels: Lib[] }>('/panels')).panels; loaded = true; } catch (e) { fail(e); } }
  load();
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const shown = $derived(list.filter((t) => !q || norm(t.panel.name).includes(norm(q))));

  async function create(e: SubmitEvent) {
    e.preventDefault();
    wizErr = '';
    try {
      const r = await api.post<Lib>('/panels', { panel: wiz });
      toast(`Panel « ${r.panel.name} » créé.`);
      navigate(`/catalogue/panels/${r.id}`);
    } catch (x) { if (x instanceof ApiError && x.fields.length) wizErr = x.fields[0]!.message; else fail(x); }
  }
  async function dup(t: Lib) { try { const r = await api.post<Lib>(`/panels/${t.id}/duplicate`, {}); toast(`« ${r.panel.name} » créé.`); await load(); } catch (e) { fail(e); } }
  async function remove() { try { await api.del(`/panels/${deleting!.id}`); toast('Panel supprimé.'); deleting = null; await load(); } catch (e) { fail(e); } }
</script>

<p class="muted" style="max-width:70ch">Panels de brassage du site : hauteur, densité, connecteurs, groupes de ports et libellés. Ajouté à un contenant de projet, un panel y est recopié et se modifie sans toucher la bibliothèque.</p>
<div class="toolbar" style="display:flex;gap:10px;flex-wrap:wrap;align-items:center">
  <label class="search" style="flex:1;min-width:220px"><Icon name="search" /><input class="input" bind:value={q} placeholder="Nom du panel…" aria-label="Rechercher un panel"></label>
  {#if writable}<button class="btn primary" onclick={() => { wiz = blankPanel(); wizErr = ''; }}><Icon name="plus" />Nouveau panel</button>{/if}
</div>

{#if !loaded}<span class="spin"></span>
{:else if !shown.length}
  <section class="card empty-card"><p class="lead">{q ? 'Aucun panel ne correspond.' : 'La bibliothèque ne compte encore aucun panel.'}</p>{#if writable && !q}<button class="btn primary" onclick={() => { wiz = blankPanel(); }}><Icon name="plus" />Créer le premier panel</button>{/if}</section>
{:else}
  <div class="pgrid">
    {#each shown as t (t.id)}
      <article class="card pcard">
        <a class="pprev" href="/catalogue/panels/{t.id}" aria-label="Ouvrir {t.panel.name}" style="--h:{t.panel.height_u}">
          <span class="ear"></span><span class="pf"><PanelFace panel={t.panel} /></span><span class="ear"></span>
        </a>
        <div class="pmeta">
          <div><a class="pn" href="/catalogue/panels/{t.id}">{t.panel.name}</a><br><span class="muted small">{t.panel.height_u} U · {t.panel.per_u} par U · {t.panel.groups.length} groupe{t.panel.groups.length > 1 ? 's' : ''} · modifié {relative(t.updated_at)}</span></div>
          <div class="fillv"><span class="mono">{t.stats.used}/{t.stats.total}</span><span class="bar"><i style="width:{t.stats.fill_pct}%"></i></span></div>
          {#if writable}<div class="dl-actions"><button class="btn sm ghost" title="Dupliquer" aria-label="Dupliquer {t.panel.name}" onclick={() => dup(t)}><Icon name="copy" size="sm" /></button><button class="btn sm ghost danger" title="Supprimer" aria-label="Supprimer {t.panel.name}" onclick={() => (deleting = t)}><Icon name="trash" size="sm" /></button></div>{/if}
        </div>
      </article>
    {/each}
  </div>
{/if}

{#if wiz}
  <Dialog title="Nouveau panel" eyebrow="Bibliothèque" onclose={() => (wiz = null)}>
    <form id="pwiz" class="stack" onsubmit={create}>
      <div class="field"><label for="pw-n">Nom</label><input class="input" id="pw-n" bind:value={wiz.name} required maxlength="100" placeholder="Patch vidéo 16 BNC">{#if wizErr}<span class="ferr">{wizErr}</span>{/if}</div>
      <div class="field"><span class="flabel">Hauteur</span><div class="seg">{#each [1, 2, 3, 4] as u}<button type="button" class:on={wiz.height_u === u} onclick={() => (wiz!.height_u = u)}>{u} U</button>{/each}</div></div>
      <div class="field"><span class="flabel">Emplacements par U</span><div class="seg">{#each PER_U as n}<button type="button" class:on={wiz.per_u === n} onclick={() => (wiz!.per_u = n)}>{n}</button>{/each}</div><span class="muted small">{wiz.height_u * wiz.per_u} emplacements · format D possible jusqu’à 16 par U</span></div>
      <div class="field" style="max-width:220px"><label for="pw-r">Profondeur de câblage arrière (mm)</label><input class="input mono" id="pw-r" type="number" min="30" max="300" step="5" bind:value={wiz.rear_depth_mm}></div>
      <div class="pprev big" style="--h:{wiz.height_u}"><span class="ear"></span><span class="pf"><PanelFace panel={wiz} /></span><span class="ear"></span></div>
    </form>
    {#snippet footer()}<button class="btn ghost" onclick={() => (wiz = null)}>Annuler</button><button class="btn primary" type="submit" form="pwiz">Créer et poser les connecteurs</button>{/snippet}
  </Dialog>
{/if}

{#if deleting}
  <Confirm title="Supprimer « {deleting.panel.name} » ?" confirmLabel="Supprimer" danger onconfirm={remove} onclose={() => (deleting = null)}>
    <p>Le panel quitte la bibliothèque. Les copies déjà posées dans les projets restent en place.</p>
  </Confirm>
{/if}

<style>
  .pgrid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 420px), 1fr)); gap: 16px; }
  .pcard { padding: 14px; display: grid; gap: 12px; }
  .pprev { display: flex; align-items: stretch; height: calc(var(--h) * 30px); background: linear-gradient(180deg, #363B42, #1D2025); border-radius: 6px; padding: 0 2px; box-shadow: 0 4px 12px rgba(0, 0, 0, .18); }
  .pprev.big { height: calc(var(--h) * 40px); }
  .pprev .pf { flex: 1; min-width: 0; padding: 3px 4px; }
  .ear { width: 14px; flex: none; background: radial-gradient(circle at 50% 50%, #C8CDD2 0 1.6px, #5A6068 2px 2.8px, transparent 3.2px) center / 14px 30px repeat-y; }
  .pmeta { display: flex; gap: 12px; align-items: center; }
  .pmeta > div:first-child { flex: 1; min-width: 0; }
  .pn { font-weight: 650; color: var(--fg); text-decoration: none; }
  .pn:hover { text-decoration: underline; }
  .fillv { display: flex; flex-direction: column; gap: 4px; align-items: flex-end; font-size: 12.5px; }
  .bar { width: 70px; height: 6px; border-radius: 99px; background: color-mix(in srgb, var(--fg) 10%, transparent); overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--ok, #22A06B); }
  .empty-card { padding: 28px; display: grid; gap: 12px; justify-items: start; }
  .ferr { color: var(--err); font-size: 13px; }
</style>
