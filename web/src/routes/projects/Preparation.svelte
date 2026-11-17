<script lang="ts">
  // Préparation et livraison (section 11.4) : une ligne par élément, modèle d'équipement,
  // modèle d'adaptateur, panel ou accessoire, type de câble ; statut, approvisionnement,
  // réserve et note ; état « câblé » de chaque liaison ; colisage par élément.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Loading from '../../components/Loading.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import SaveState from '../../components/SaveState.svelte';
  import { api, download } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { fail } from '../../lib/toast.svelte';
  import { relative } from '../../lib/format';
  import { route } from '../../lib/router.svelte';
  import type { ProjectView } from '../../lib/projects';

  interface CableRow { id: string; number: string | null; from: string; to: string; done_at: string | null; done_by: string | null; note: string | null }
  interface Line { key: string; label: string; detail: string; min: number; reserve: number; total: number; status: string; supply: string | null; note: string | null; updated_by: string | null; updated_at: string | null; model_id: string | null; missing_model: boolean; cables: CableRow[]; cabled: number }
  interface Fam { key: string; label: string; statuses: { key: string; label: string }[]; lines: Line[]; done: number }
  interface Pack { id: string; name: string; location: string; kind: string; content: { label: string; quantity: number }[] }

  let { view }: { view: ProjectView } = $props();
  const pid = $derived(view.project.id);
  const writable = $derived(can('projets', 'ecriture'));
  let data = $state<{ families: Fam[]; packing: Pack[] } | null>(null);
  // L'onglet choisi est dans l'adresse (?onglet=) : un lien ou un rechargement y ramène.
  let tab = $state(route.query.get('onglet') ?? 'elements');
  $effect(() => { const t = route.query.get('onglet'); if (t && t !== untrack(() => tab)) tab = t; });
  function pick(t: string) {
    tab = t; status = '';
    const qs = new URLSearchParams(location.search); qs.set('onglet', t);
    history.replaceState(null, '', `${location.pathname}?${qs}`);
    route.query = qs;
  }
  let q = $state('');
  let status = $state('');
  let open = $state<Set<string>>(new Set());
  let models = $state<{ id: string; name: string }[]>([]);

  async function load() { try { data = await api.get(`/projects/${pid}/prep`); } catch (e) { fail(e); } }
  $effect(() => { view; untrack(load); });
  api.get<{ items: { id: string; name: string }[] }>('/catalog/items?kind=cable-models&active=true&limit=200').then((r) => (models = r.items), () => {});

  const fam = $derived(data?.families.find((f) => f.key === tab) ?? null);
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const shown = $derived((fam?.lines ?? []).filter((l) => (!status || l.status === status) && (!q || norm(`${l.label} ${l.detail} ${l.note ?? ''}`).includes(norm(q)))));
  const rank = (f: Fam, s: string) => f.statuses.findIndex((x) => x.key === s);
  /** Colisage : les articles identiques d'un même élément sont regroupés (« 5 × 24x BNC12G »). */
  const packed = (p: Pack) => {
    const m = new Map<string, number>();
    for (const c of p.content) m.set(c.label, (m.get(c.label) ?? 0) + c.quantity);
    return [...m].map(([label, quantity]) => ({ label, quantity }));
  };

  async function set(l: Line, body: Record<string, unknown>) {
    try { await api.patch(`/projects/${pid}/prep/line`, { key: l.key, ...body }); await load(); } catch (e) { fail(e); await load(); }
  }
  async function model(l: Line, id: string) { try { await api.post(`/projects/${pid}/prep/cable-model`, { key: l.key, model_id: id || null }); await load(); } catch (e) { fail(e); } }
  async function cabled(c: CableRow, done: boolean, note: string | null = c.note) { try { await api.post(`/projects/${pid}/prep/cabled`, { cable_id: c.id, done, note }); await load(); } catch (e) { fail(e); } }
  function toggle(k: string) { const s = new Set(open); if (s.has(k)) s.delete(k); else s.add(k); open = s; }
  const pct = (d: number, t: number) => (t ? Math.round((100 * d) / t) : 0);
</script>

{#if !data}<Loading label="Chargement de la préparation" />
{:else}
  <div class="pp-head" role="tablist" aria-label="Familles à préparer">
    {#each data.families as f (f.key)}
      <button class="card link pp-fam" role="tab" aria-selected={tab === f.key} class:on={tab === f.key} onclick={() => pick(f.key)}>
        <span class="flabel">{f.label}</span>
        <b class="mono">{f.done} / {f.lines.length}</b>
        <span class="bar"><i style="width:{pct(f.done, f.lines.length)}%"></i></span>
      </button>
    {/each}
    <button class="card link pp-fam" role="tab" aria-selected={tab === 'packing'} class:on={tab === 'packing'} onclick={() => pick('packing')}><span class="flabel">Colisage</span><b class="mono">{data.packing.length}</b><span class="muted small">élément{data.packing.length > 1 ? 's' : ''}</span></button>
  </div>

  {#if tab === 'packing'}
    {#if data.packing.length}
      <div class="pack">
        {#each data.packing as p (p.id)}
          <section class="card compact pk">
            <div class="pk-h"><b>{p.name}</b><span class="muted small">{p.kind} · {p.location}</span></div>
            <ul>{#each packed(p) as c}<li><span class="mono">{c.quantity} ×</span> {c.label}</li>{:else}<li class="muted">Vide</li>{/each}</ul>
          </section>
        {/each}
      </div>
    {:else}<EmptyState icon="box" message="Aucun élément" hint="Le colisage liste le contenu de chaque élément de l’inventaire : ajoutez-en depuis l’Inventaire." />{/if}
  {:else if fam}
    <div class="toolbar pp-tools">
      <label class="search"><Icon name="search" /><input class="input" type="search" bind:value={q} placeholder="Rechercher une désignation, un détail, une note" aria-label="Rechercher"></label>
      <select class="input sm" bind:value={status} aria-label="Statut"><option value="">Tous les statuts</option>{#each fam.statuses as s}<option value={s.key}>{s.label}</option>{/each}</select>
      <span class="grow"></span>
      <SaveState />
      <button class="btn sm ghost" onclick={() => download(`/projects/${pid}/prep/export?format=csv`)}><Icon name="download" size="sm" />CSV</button>
      <button class="btn sm ghost" onclick={() => download(`/projects/${pid}/prep/export?format=xlsx`)}><Icon name="download" size="sm" />Excel</button>
    </div>
    <!-- Téléphone : .tbl.stack présente chaque ligne en carte (désignation, quantité, approvisionnement et statut, note). -->
    <section class="card tbl-card dense">
      <div class="tbl-wrap"><table class="tbl stack pp">
        <thead><tr><th>Désignation</th><th class="num qh">Quantité<span>Besoin + Réserve = Total</span></th><th>Approvisionnement</th><th>Statut</th><th>Note</th><th><span class="sr-only">Actions</span></th></tr></thead>
        <tbody>
          {#each shown as l (l.key)}
            {@const r = rank(fam, l.status)}
            {@const done = r === fam.statuses.length - 1}
            <tr>
              <td class="first des">
                <b>{l.label}</b>{#if l.missing_model}<span class="chip warn">Type manquant</span>{/if}
                <span class="muted small">{l.detail}</span>
                {#if tab === 'cables'}<select class="input sm mdl" disabled={!writable} value={l.model_id ?? ''} onchange={(e) => model(l, (e.target as HTMLSelectElement).value)} aria-label="Type de câble de {l.label}"><option value="">Type de câble à choisir</option>{#each models as m}<option value={m.id}>{m.name}</option>{/each}</select>{/if}
              </td>
              <td class="num" data-l="Besoin + Réserve = Total">
                <span class="qty"><span class="mono" title="Besoin calculé depuis l’inventaire et le câblage">{l.min}</span> + <input class="input sm mono rsv" type="number" min="0" max="9999" value={l.reserve} disabled={!writable} onchange={(e) => set(l, { reserve: Math.max(0, Number((e.target as HTMLInputElement).value) || 0) })} aria-label="Réserve de {l.label}" title="Réserve : quantité en plus du besoin"> = <b class="mono" title="Total à préparer">{l.total}</b></span>
              </td>
              <td data-l="Approvisionnement"><select class="input sm" disabled={!writable} value={l.supply ?? 'stock'} onchange={(e) => set(l, { supply: (e.target as HTMLSelectElement).value })} aria-label="Approvisionnement de {l.label}"><option value="stock">Stock</option><option value="purchase">Achat</option><option value="rental">Location</option></select></td>
              <td data-l="Statut">
                {#if writable}<label class="chip pp-st" class:ok={done} class:first-st={r === 0}>{#if done}<Icon name="ok" size="sm" />{/if}<select value={l.status} onchange={(e) => set(l, { status: (e.target as HTMLSelectElement).value })} aria-label="Statut de {l.label}">{#each fam.statuses as s}<option value={s.key}>{s.label}</option>{/each}</select><Icon name="chevd" size="sm" /></label>
                {:else}<span class="chip" class:ok={done}>{#if done}<Icon name="ok" size="sm" />{/if}{fam.statuses[r]?.label ?? l.status}</span>{/if}
              </td>
              <td class="pp-note" data-l="Note"><input class="input sm note" value={l.note ?? ''} disabled={!writable} maxlength="1000" onchange={(e) => set(l, { note: (e.target as HTMLInputElement).value })} aria-label="Note de {l.label}">{#if l.updated_at}<span class="muted small">{l.updated_by ?? ''} · {relative(l.updated_at)}</span>{/if}</td>
              <td class="acts">{#if tab === 'cables'}<button class="btn sm ghost" onclick={() => toggle(l.key)} aria-expanded={open.has(l.key)}>{l.cabled}/{l.cables.length} câblé{l.cabled > 1 ? 's' : ''}<Icon name="chevd" size="sm" /></button>{/if}</td>
            </tr>
            {#if open.has(l.key)}
              <tr class="sub"><td colspan="6">
                {#each l.cables as c (c.id)}
                  <div class="cb">
                    <label class="toggle-row"><input type="checkbox" checked={!!c.done_at} disabled={!writable} onchange={(e) => cabled(c, (e.target as HTMLInputElement).checked)}>{#if c.number}<b class="mono">{c.number}</b>{:else}<span class="muted">non numéroté</span>{/if}</label>
                    <span class="small">{c.from} → {c.to}</span>
                    <span class="muted small">{c.done_at ? `Câblé ${relative(c.done_at)}${c.done_by ? ` par ${c.done_by}` : ''}` : 'À câbler'}</span>
                    {#if c.done_at}<input class="input sm note" value={c.note ?? ''} disabled={!writable} placeholder="Note" onchange={(e) => cabled(c, true, (e.target as HTMLInputElement).value)} aria-label="Note de câblage">{/if}
                  </div>
                {/each}
              </td></tr>
            {/if}
          {:else}<tr><td colspan="6"><EmptyState card={false} icon={fam.lines.length ? 'search' : 'box'} message={fam.lines.length ? 'Aucune ligne ne correspond' : tab === 'cables' ? 'Aucun câble' : 'Rien à préparer'} hint={fam.lines.length ? 'Modifiez la recherche ou le statut.' : tab === 'cables' ? 'Tracez les câbles dans le synoptique.' : 'Rien à préparer dans cette famille.'} /></td></tr>{/each}
        </tbody>
      </table></div>
    </section>
    <button type="button" class="ux3-info" title="Quantité : le besoin, calculé depuis l’inventaire et le câblage, plus la réserve que vous saisissez, donne le total à préparer. Un câble sans modèle compte comme manquant ; choisir un modèle l’affecte à toute la ligne." aria-label="Quantité : le besoin, calculé depuis l’inventaire et le câblage, plus la réserve que vous saisissez, donne le total à préparer. Un câble sans modèle compte comme manquant ; choisir un modèle l’affecte à toute la ligne."><Icon name="info" size="sm" /></button>
  {/if}
{/if}

<style>
  .pp-head { display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr)); gap: 10px; margin-bottom: 14px; }
  /* Cartes-onglets (variante légitime .pp-fam) : l'onglet actif suit .card.link.on. */
  .pp-fam { display: grid; gap: 4px; padding: 12px 14px; cursor: pointer; font: inherit; }
  .pp-fam:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .pp-fam.on .flabel { color: var(--fg); }
  .pp-fam b { font-size: 20px; }
  .bar { display: block; height: 6px; border-radius: 99px; background: color-mix(in srgb, var(--fg) 10%, transparent); overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--ok); }
  .pp-tools { margin-bottom: 10px; }
  .pp-tools .search { flex: 1; min-width: 200px; }
  .pp-tools select { width: auto; }
  .grow { flex: 1; }
  .des { display: flex; flex-direction: column; gap: 3px; min-width: 240px; }
  .des .chip { align-self: flex-start; }
  .mdl { max-width: 260px; }
  .qh span { display: block; font: 500 11px var(--font); letter-spacing: 0; text-transform: none; color: var(--fg-3); white-space: nowrap; }
  .qty { display: inline-flex; gap: 6px; align-items: center; white-space: nowrap; }
  .rsv { width: 64px; text-align: right; }
  td select.input { width: auto; min-width: 130px; }
  /* Statut modifiable : une pastille qui contient la liste de choix (verte quand la ligne est prête). */
  .pp-st { position: relative; height: 30px; padding-right: 8px; cursor: pointer; }
  .pp-st.first-st { color: var(--fg-3); }
  .pp-st select { appearance: none; border: 0; background: none; color: inherit; font: inherit; padding: 0 2px 0 0; height: 100%; cursor: pointer; min-width: 0; }
  .pp-st select:focus-visible { outline: none; }
  .pp-st:focus-within { outline: 2px solid var(--accent); outline-offset: 2px; }
  .note { min-width: 160px; }
  .pp-note { display: flex; flex-direction: column; align-items: stretch; gap: 2px; }
  tr.sub td { background: var(--surface-2); }
  .cb { display: grid; grid-template-columns: 180px minmax(0, 1fr) 200px 220px; gap: 10px; align-items: center; padding: 4px 0; }
  .pack { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; }
  .pk { display: grid; gap: 8px; }
  .pk-h { display: flex; flex-direction: column; }
  .pk ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; font-size: 14px; }
  @media (max-width: 760px) { .cb { grid-template-columns: 1fr; } }
  /* Cartes (.tbl.stack) : la note occupe toute la largeur, les listes s'étirent. */
  @media (max-width: 860px) {
    .des { min-width: 0; }
    .mdl { max-width: none; width: 100%; }
    .pp td.pp-note { grid-column: 1 / -1; flex-direction: column; align-items: stretch; }
    .pp td.pp-note::before { margin-bottom: 2px; }
    .pp td[data-l] { flex-wrap: wrap; }
    .pp td select.input { min-width: 0; }
    .pp tr.sub { display: block; }
    .pp tr.sub td { display: block; padding: 8px 10px; border-radius: 10px; }
    .pp td.acts:empty { display: none; }
  }
</style>
