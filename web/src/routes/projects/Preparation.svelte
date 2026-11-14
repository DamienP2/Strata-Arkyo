<script lang="ts">
  // Préparation et livraison (section 11.4) : une ligne par élément, modèle d'équipement,
  // modèle d'adaptateur, panel ou accessoire, type de câble ; statut, approvisionnement,
  // réserve et note ; état « câblé » de chaque liaison ; colisage par élément.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import { api, download } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { fail } from '../../lib/toast.svelte';
  import { relative } from '../../lib/format';
  import type { ProjectView } from '../../lib/projects';

  interface CableRow { id: string; number: string | null; from: string; to: string; done_at: string | null; done_by: string | null; note: string | null }
  interface Line { key: string; label: string; detail: string; min: number; reserve: number; total: number; status: string; supply: string | null; note: string | null; updated_by: string | null; updated_at: string | null; model_id: string | null; missing_model: boolean; cables: CableRow[]; cabled: number }
  interface Fam { key: string; label: string; statuses: { key: string; label: string }[]; lines: Line[]; done: number }
  interface Pack { id: string; name: string; location: string; kind: string; content: { label: string; quantity: number }[] }

  let { view }: { view: ProjectView } = $props();
  const pid = $derived(view.project.id);
  const writable = $derived(can('projets', 'ecriture'));
  let data = $state<{ families: Fam[]; packing: Pack[] } | null>(null);
  let tab = $state('elements');
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

  async function set(l: Line, body: Record<string, unknown>) {
    try { await api.patch(`/projects/${pid}/prep/line`, { key: l.key, ...body }); await load(); } catch (e) { fail(e); await load(); }
  }
  async function model(l: Line, id: string) { try { await api.post(`/projects/${pid}/prep/cable-model`, { key: l.key, model_id: id || null }); await load(); } catch (e) { fail(e); } }
  async function cabled(c: CableRow, done: boolean, note: string | null = c.note) { try { await api.post(`/projects/${pid}/prep/cabled`, { cable_id: c.id, done, note }); await load(); } catch (e) { fail(e); } }
  function toggle(k: string) { const s = new Set(open); if (s.has(k)) s.delete(k); else s.add(k); open = s; }
  const pct = (d: number, t: number) => (t ? Math.round((100 * d) / t) : 0);
</script>

{#if !data}<span class="spin"></span>
{:else}
  <div class="pp-head" role="tablist" aria-label="Familles à préparer">
    {#each data.families as f (f.key)}
      <button class="pp-fam" role="tab" aria-selected={tab === f.key} class:on={tab === f.key} onclick={() => { tab = f.key; status = ''; }}>
        <span class="flabel">{f.label}</span>
        <b class="mono">{f.done} / {f.lines.length}</b>
        <span class="bar"><i style="width:{pct(f.done, f.lines.length)}%"></i></span>
      </button>
    {/each}
    <button class="pp-fam" role="tab" aria-selected={tab === 'packing'} class:on={tab === 'packing'} onclick={() => (tab = 'packing')}><span class="flabel">Colisage</span><b class="mono">{data.packing.length}</b><span class="muted small">élément{data.packing.length > 1 ? 's' : ''}</span></button>
  </div>

  {#if tab === 'packing'}
    <div class="pack">
      {#each data.packing as p (p.id)}
        <section class="card pk">
          <div class="pk-h"><b>{p.name}</b><span class="muted small">{p.kind} · {p.location}</span></div>
          <ul>{#each p.content as c}<li><span class="mono">{c.quantity} ×</span> {c.label}</li>{:else}<li class="muted">Vide</li>{/each}</ul>
        </section>
      {/each}
    </div>
  {:else if fam}
    <div class="toolbar pp-tools">
      <label class="search" style="flex:1;min-width:200px"><Icon name="search" /><input class="input" bind:value={q} placeholder="Désignation, détail, note…" aria-label="Rechercher"></label>
      <select class="input" style="width:auto;height:40px" bind:value={status} aria-label="Statut"><option value="">Tous les statuts</option>{#each fam.statuses as s}<option value={s.key}>{s.label}</option>{/each}</select>
      <button class="btn sm ghost" onclick={() => download(`/projects/${pid}/prep/export?format=csv`)}><Icon name="download" size="sm" />CSV</button>
      <button class="btn sm ghost" onclick={() => download(`/projects/${pid}/prep/export?format=xlsx`)}><Icon name="download" size="sm" />XLSX</button>
    </div>
    <section class="card pp">
      <div class="tbl-wrap"><table class="tbl">
        <thead><tr><th>Désignation</th><th class="num qh">Quantité<span>Besoin + Réserve = Total</span></th><th>Approvisionnement</th><th>Statut</th><th>Note</th><th></th></tr></thead>
        <tbody>
          {#each shown as l (l.key)}
            {@const r = rank(fam, l.status)}
            <tr>
              <td class="des">
                <b>{l.label}</b>{#if l.missing_model}<span class="chip warn">Modèle manquant</span>{/if}
                <span class="muted small">{l.detail}</span>
                {#if tab === 'cables'}<select class="input mdl" disabled={!writable} value={l.model_id ?? ''} onchange={(e) => model(l, (e.target as HTMLSelectElement).value)} aria-label="Modèle de câble"><option value="">Modèle à choisir…</option>{#each models as m}<option value={m.id}>{m.name}</option>{/each}</select>{/if}
              </td>
              <td class="num">
                <span class="qty"><span class="mono" title="Besoin calculé depuis l’inventaire et le câblage">{l.min}</span> + <input class="input mono rsv" type="number" min="0" max="9999" value={l.reserve} disabled={!writable} onchange={(e) => set(l, { reserve: Math.max(0, Number((e.target as HTMLInputElement).value) || 0) })} aria-label="Réserve" title="Réserve : quantité en plus du besoin"> = <b class="mono" title="Total à préparer">{l.total}</b></span>
              </td>
              <td><select class="input" disabled={!writable} value={l.supply ?? 'stock'} onchange={(e) => set(l, { supply: (e.target as HTMLSelectElement).value })} aria-label="Approvisionnement"><option value="stock">Stock</option><option value="purchase">Achat</option><option value="rental">Location</option></select></td>
              <td><select class="input st st{r}" class:done={r === fam.statuses.length - 1} disabled={!writable} value={l.status} onchange={(e) => set(l, { status: (e.target as HTMLSelectElement).value })} aria-label="Statut">{#each fam.statuses as s}<option value={s.key}>{s.label}</option>{/each}</select></td>
              <td><input class="input note" value={l.note ?? ''} disabled={!writable} maxlength="1000" placeholder="" onchange={(e) => set(l, { note: (e.target as HTMLInputElement).value })} aria-label="Note">{#if l.updated_at}<span class="muted small">{l.updated_by ?? ''} · {relative(l.updated_at)}</span>{/if}</td>
              <td>{#if tab === 'cables'}<button class="btn sm ghost" onclick={() => toggle(l.key)} aria-expanded={open.has(l.key)}>{l.cabled}/{l.cables.length} câblé{l.cabled > 1 ? 's' : ''}<Icon name="chevd" size="sm" /></button>{/if}</td>
            </tr>
            {#if open.has(l.key)}
              <tr class="sub"><td colspan="6">
                {#each l.cables as c (c.id)}
                  <div class="cb">
                    <label class="toggle-row"><input type="checkbox" checked={!!c.done_at} disabled={!writable} onchange={(e) => cabled(c, (e.target as HTMLInputElement).checked)}><b class="mono">{c.number ?? 'Câble non numéroté'}</b></label>
                    <span class="small">{c.from} → {c.to}</span>
                    <span class="muted small">{c.done_at ? `Câblé ${relative(c.done_at)}${c.done_by ? ` par ${c.done_by}` : ''}` : 'À câbler'}</span>
                    {#if c.done_at}<input class="input note" value={c.note ?? ''} disabled={!writable} placeholder="Note" onchange={(e) => cabled(c, true, (e.target as HTMLInputElement).value)} aria-label="Note de câblage">{/if}
                  </div>
                {/each}
              </td></tr>
            {/if}
          {:else}<tr><td colspan="6" class="empty">{fam.lines.length ? 'Aucune ligne ne correspond.' : tab === 'cables' ? 'Aucun câble : tracez-les dans le synoptique.' : 'Rien à préparer dans cette famille.'}</td></tr>{/each}
        </tbody>
      </table></div>
    </section>
    <p class="muted small">Quantité : le besoin, calculé depuis l’inventaire et le câblage, plus la réserve que vous saisissez, donne le total à préparer. Un câble sans modèle compte comme manquant ; choisir un modèle l’affecte à toute la ligne.</p>
  {/if}
{/if}

<style>
  .pp-head { display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr)); gap: 10px; margin-bottom: 14px; }
  .pp-fam { display: grid; gap: 4px; padding: 12px 14px; border-radius: 14px; border: 1.5px solid var(--line); background: var(--surface); text-align: left; cursor: pointer; font: inherit; color: inherit; }
  .pp-fam:hover { border-color: var(--fg-3); }
  /* Les cartes sont des onglets : celui qui filtre la liste est marqué comme une sélection (fond teinté, liseré). */
  .pp-fam.on { border-color: var(--accent, #4F6BFF); background: color-mix(in srgb, var(--accent, #4F6BFF) 12%, var(--surface)); box-shadow: inset 0 -3px 0 var(--accent, #4F6BFF); }
  .pp-fam.on .flabel { color: var(--fg); }
  .pp-fam b { font-size: 20px; }
  .bar { display: block; height: 6px; border-radius: 99px; background: color-mix(in srgb, var(--fg) 10%, transparent); overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--ok, #22A06B); }
  .pp-tools { margin-bottom: 10px; }
  .pp { padding: 4px 8px 8px; }
  .des { display: flex; flex-direction: column; gap: 3px; min-width: 240px; }
  .des .chip { align-self: flex-start; }
  .mdl { height: 34px; max-width: 260px; font-size: 13.5px; }
  .qh span { display: block; font: 500 11px var(--font); letter-spacing: 0; text-transform: none; color: var(--fg-3); white-space: nowrap; }
  .qty { display: inline-flex; gap: 6px; align-items: center; white-space: nowrap; }
  .rsv { width: 64px; height: 34px; text-align: right; }
  td select.input { height: 36px; width: auto; min-width: 130px; font-size: 14px; }
  .st.done { background: color-mix(in srgb, var(--ok, #22A06B) 16%, var(--surface)); color: var(--ok, #22A06B); font-weight: 600; }
  .st.st0 { color: var(--fg-2); }
  .note { height: 34px; min-width: 160px; font-size: 14px; }
  tr.sub td { background: var(--surface-2); }
  .cb { display: grid; grid-template-columns: 180px minmax(0, 1fr) 200px 220px; gap: 10px; align-items: center; padding: 4px 0; }
  .pack { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; }
  .pk { padding: 14px; display: grid; gap: 8px; }
  .pk-h { display: flex; flex-direction: column; }
  .pk ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; font-size: 14px; }
  @media (max-width: 760px) { .cb { grid-template-columns: 1fr; } }
</style>
