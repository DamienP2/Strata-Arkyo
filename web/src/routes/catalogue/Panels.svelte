<script lang="ts">
  // Bibliothèque des panels de brassage (section 6.3) : liste, assistant de création,
  // duplication, suppression ; l'édition ouvre /catalogue/panels/{id}.
  import Icon from '../../components/Icon.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Loading from '../../components/Loading.svelte';
  import Seg from '../../components/Seg.svelte';
  import NumField from '../../components/NumField.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import { ask } from '../../lib/ask.svelte';
  import PanelFace from '../../components/PanelFace.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import HeadActions from '../../components/HeadActions.svelte';
  import { pageHead, plural } from '../../lib/pagehead.svelte';
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
  const writable = $derived(can('catalogue', 'ecriture'));

  async function load() { try { list = (await api.get<{ panels: Lib[] }>('/panels')).panels; loaded = true; } catch (e) { fail(e); } }
  load();
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const shown = $derived(list.filter((t) => !q || norm(t.panel.name).includes(norm(q))));
  function start() { wiz = blankPanel(); wizErr = ''; }
  // Bibliothèque vide : la seule action est celle de l'état vide.
  pageHead(() => ({ count: loaded && list.length ? plural(list.length, 'panel') : '' }));

  async function create() {
    wizErr = '';
    try {
      const r = await api.post<Lib>('/panels', { panel: wiz });
      toast(`Panel « ${r.panel.name} » créé.`);
      navigate(`/catalogue/panels/${r.id}`);
    } catch (x) { if (x instanceof ApiError && x.fields.length) wizErr = x.fields[0]!.message; else fail(x); }
  }
  async function dup(t: Lib) { try { const r = await api.post<Lib>(`/panels/${t.id}/duplicate`, {}); toast(`« ${r.panel.name} » créé.`); await load(); } catch (e) { fail(e); } }
  async function remove(t: Lib) {
    if (!(await ask({ title: `Supprimer « ${t.panel.name} » ?`, eyebrow: 'Panel de brassage', text: 'Le panel quitte la bibliothèque. Les copies déjà posées dans les projets restent en place.', confirmLabel: 'Supprimer', danger: true, run: () => api.del(`/panels/${t.id}`) }))) return;
    toast(`« ${t.panel.name} » supprimé.`); await load();
  }
</script>
{#if writable && list.length}<HeadActions><button class="btn primary" onclick={start}><Icon name="plus" />Nouveau panel</button></HeadActions>{/if}

{#if !loaded}<Loading label="Chargement des panels" />
{:else if !list.length}
  <EmptyState icon="panel" message="La bibliothèque ne compte encore aucun panel.">{#snippet action()}{#if writable}<button class="btn primary" onclick={start}><Icon name="plus" />Nouveau panel</button>{/if}{/snippet}</EmptyState>
{:else}
<div class="toolbar">
  <label class="search" style="flex:1;min-width:220px;max-width:480px"><Icon name="search" /><input class="input" type="search" bind:value={q} placeholder="Nom du panel" aria-label="Rechercher un panel"></label>
</div>
{#if !shown.length}
  <EmptyState icon="search" message="Aucun panel ne correspond à cette recherche." />
{:else}
  <div class="pgrid">
    {#each shown as t (t.id)}
      <article class="card pn-card">
        <a class="pprev" href="/catalogue/panels/{t.id}" aria-label="Ouvrir {t.panel.name}" style="--h:{t.panel.height_u}">
          <span class="ear"></span><span class="pf"><PanelFace panel={t.panel} /></span><span class="ear"></span>
        </a>
        <div class="pmeta">
          <div><a class="pn" href="/catalogue/panels/{t.id}">{t.panel.name}</a><br><span class="muted small">{t.panel.height_u} U · {t.panel.per_u} par U · {t.panel.groups.length} groupe{t.panel.groups.length > 1 ? 's' : ''} · modifié {relative(t.updated_at)}</span></div>
          <div class="fillv"><span class="mono">{t.stats.used}/{t.stats.total}</span><span class="bar"><i style="width:{t.stats.fill_pct}%"></i></span></div>
          {#if writable}<div class="dl-actions"><button class="icon-btn sm" title="Dupliquer {t.panel.name}" aria-label="Dupliquer {t.panel.name}" onclick={() => dup(t)}><Icon name="copy" size="sm" /></button><button class="icon-btn sm danger" title="Supprimer {t.panel.name}" aria-label="Supprimer {t.panel.name}" onclick={() => remove(t)}><Icon name="trash" size="sm" /></button></div>{/if}
        </div>
      </article>
    {/each}
  </div>
{/if}
{/if}

{#if wiz}
  <Dialog title="Nouveau panel" eyebrow="Bibliothèque" onclose={() => (wiz = null)} onsubmit={create} submitLabel="Créer et poser les connecteurs">
    <div class="stack">
      <div class="field"><label for="pw-n">Nom</label><input class="input" class:err={!!wizErr} id="pw-n" bind:value={wiz.name} required maxlength="100" placeholder="Patch vidéo 16 BNC" aria-invalid={wizErr ? 'true' : undefined} aria-describedby={wizErr ? 'pw-n-err' : undefined}><FieldError message={wizErr} for="pw-n" /></div>
      <div class="field"><span class="flabel">Hauteur</span><Seg label="Hauteur" bind:value={wiz.height_u} options={[1, 2, 3, 4].map((u) => ({ value: u, label: `${u} U` }))} /></div>
      <div class="field"><span class="flabel">Emplacements par U</span><Seg label="Emplacements par U" bind:value={wiz.per_u} options={PER_U.map((n) => ({ value: n, label: String(n) }))} /><span class="help">{wiz.height_u * wiz.per_u} emplacements · format D possible jusqu’à 16 par U</span></div>
      <div style="max-width:240px"><NumField id="pw-r" label="Profondeur de câblage arrière" unit="mm" min={30} max={300} step={5} bind:value={() => wiz!.rear_depth_mm, (v) => (wiz!.rear_depth_mm = v ?? 80)} /></div>
      <div class="pprev big" style="--h:{wiz.height_u}"><span class="ear"></span><span class="pf"><PanelFace panel={wiz} /></span><span class="ear"></span></div>
    </div>
  </Dialog>
{/if}

<style>
  .pgrid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 420px), 1fr)); gap: 16px; }
  .pn-card { display: grid; gap: 12px; }
  .pprev { display: flex; align-items: stretch; height: calc(var(--h) * 30px); background: linear-gradient(180deg, #363B42, #1D2025); border-radius: 6px; padding: 0 2px; box-shadow: 0 4px 12px rgba(0, 0, 0, .18); }
  .pprev.big { height: calc(var(--h) * 40px); }
  .pprev .pf { flex: 1; min-width: 0; padding: 3px 4px; }
  .ear { width: 14px; flex: none; background: radial-gradient(circle at 50% 50%, #C8CDD2 0 1.6px, #5A6068 2px 2.8px, transparent 3.2px) center / 14px 30px repeat-y; }
  .pmeta { display: flex; gap: 12px; align-items: center; }
  .pmeta > div:first-child { flex: 1; min-width: 0; }
  .pn { font-weight: 650; color: var(--fg); text-decoration: none; }
  .pn:hover { text-decoration: underline; }
  .fillv { display: flex; flex-direction: column; gap: 4px; align-items: flex-end; font-size: var(--fs-sm); }
  .bar { width: 70px; height: 6px; border-radius: 99px; background: color-mix(in srgb, var(--fg) 10%, transparent); overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--ok); }
</style>
