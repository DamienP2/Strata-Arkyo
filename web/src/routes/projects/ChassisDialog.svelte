<script lang="ts">
  // Châssis d'un projet : ses emplacements, les modules qui y sont glissés (avec leur repère),
  // ajout d'un module du catalogue dans un emplacement libre, retrait d'un module.
  import Dialog from '../../components/Dialog.svelte';
  import Icon from '../../components/Icon.svelte';
  import { api } from '../../lib/api';
  import { toast, fail } from '../../lib/toast.svelte';
  import type { Article, ProjectView } from '../../lib/projects';

  let { view, host, writable, onclose, ondone }: { view: ProjectView; host: Article; writable: boolean; onclose: () => void; ondone: () => Promise<void> } = $props();
  const slots = $derived(view.items[host.item_id ?? '']?.host_slots ?? 0);
  const modules = $derived(view.articles.filter((a) => a.host?.article_id === host.id).sort((a, b) => a.host!.slot - b.host!.slot));
  const widthOf = (a: Article) => view.items[a.item_id ?? '']?.module_slots ?? 1;
  const tagOf = (a: Article) => view.instances.find((i) => i.article_id === a.id)?.tag ?? '';
  const occ = $derived.by(() => { const m = new Map<number, { a: Article; first: boolean; w: number }>(); for (const a of modules) { const w = widthOf(a); for (let k = 0; k < w; k++) m.set(a.host!.slot + k, { a, first: k === 0, w }); } return m; });
  const used = $derived(modules.reduce((n, a) => n + widthOf(a), 0));

  interface Comp { id: string; name: string; reference: string | null; slots: number }
  let comps = $state<Comp[] | null>(null);
  let pick = $state<number | null>(null);
  let q = $state('');
  let busy = $state(false);
  async function openPick(slot: number) {
    pick = pick === slot ? null : slot;
    if (comps) return;
    try { comps = (await api.get<{ items: { id: string; name: string; reference: string | null; data: any }[] }>('/catalog/items?kind=equipment&modular=component&active=true&limit=1000')).items.map((i) => ({ id: i.id, name: i.name, reference: i.reference, slots: i.data.modular?.slots ?? 1 })); }
    catch { comps = []; }
  }
  const fits = (slot: number, w: number) => { if (slot + w - 1 > slots) return false; for (let k = 0; k < w; k++) if (occ.has(slot + k)) return false; return true; };
  async function add(c: Comp) {
    if (pick == null) return;
    busy = true;
    try { await api.post(`/articles/${host.id}/modules`, { item_id: c.id, slot: pick }); toast(`${c.name} glissé dans l’emplacement ${pick}.`); pick = null; q = ''; await ondone(); }
    catch (e) { fail(e); } finally { busy = false; }
  }
  async function remove(a: Article) {
    busy = true;
    try { await api.del(`/articles/${a.id}`); toast('Module retiré du châssis.'); await ondone(); }
    catch (e) { fail(e); } finally { busy = false; }
  }
</script>

<Dialog title={host.label} eyebrow="Châssis · {used} / {slots} emplacements occupés" wide {onclose}>
  <div class="chassis" style="--n:{Math.max(1, slots)}">
    {#each Array(Math.max(1, slots)) as _, k}
      {@const slot = k + 1}
      {@const o = occ.get(slot)}
      {#if o?.first}
        <div class="cs full" style="grid-column:span {Math.min(o.w, slots - k)}" title="{o.a.label} · {tagOf(o.a)}">
          <span class="n">{slot}{o.w > 1 ? `–${slot + o.w - 1}` : ''}</span>
          <span class="nm">{o.a.label}</span>
          <span class="tg mono">{tagOf(o.a)}</span>
          {#if writable}<button class="x" aria-label="Retirer {o.a.label}" disabled={busy} onclick={() => remove(o.a)}><Icon name="x" size="sm" /></button>{/if}
        </div>
      {:else if !o}
        <button class="cs" class:on={pick === slot} disabled={!writable} onclick={() => openPick(slot)} title="Emplacement {slot} libre"><span class="n">{slot}</span>{#if writable}<span class="plus">+</span>{/if}</button>
      {/if}
    {/each}
  </div>
  <p class="muted small" style="margin:10px 0 0">Les ports des modules s’ajoutent au châssis dans le synoptique (« S3 · SDI IN »). Chaque module garde son repère, son numéro de série et son poids.</p>
  {#if pick != null}
    <div class="pickr">
      <div class="ph"><b>Emplacement {pick}</b><input class="input" placeholder="Rechercher un module…" bind:value={q} aria-label="Rechercher un module"></div>
      {#if comps === null}<span class="spin"></span>
      {:else}
        {#each comps.filter((c) => !q.trim() || `${c.name} ${c.reference ?? ''}`.toLowerCase().includes(q.trim().toLowerCase())).slice(0, 80) as c (c.id)}
          {@const ok = fits(pick, c.slots)}
          <button class="pr" disabled={!ok || busy} onclick={() => add(c)}><b>{c.name}</b><span class="muted small">{c.reference ?? ''}</span><span class="w mono">{c.slots} empl.{ok ? '' : ' · ne tient pas'}</span></button>
        {:else}<p class="muted small">Aucun module au catalogue : dans la fiche d’une carte, onglet Modules, choisissez « Module ».</p>{/each}
      {/if}
    </div>
  {/if}
  {#snippet footer()}<button class="btn primary" onclick={onclose}>Fermer</button>{/snippet}
</Dialog>

<style>
  .chassis { display: grid; grid-template-columns: repeat(var(--n), minmax(24px, 1fr)); gap: 3px; padding: 10px; border-radius: 8px; background: linear-gradient(180deg, #2b3036, #1c2025); box-shadow: inset 0 0 0 1px #000; overflow-x: auto; }
  .cs { position: relative; height: 170px; display: flex; flex-direction: column; align-items: center; justify-content: space-between; gap: 4px; padding: 6px 2px; border: 0; border-radius: 3px; background: repeating-linear-gradient(180deg, #121519 0 6px, #171b20 6px 12px); box-shadow: inset 0 0 0 1px #000; color: #8f98a3; font: inherit; cursor: pointer; min-width: 0; }
  .cs:hover:not(:disabled), .cs.on { box-shadow: inset 0 0 0 2px var(--accent); }
  .cs:disabled { cursor: default; }
  .cs.full { cursor: default; background: linear-gradient(90deg, #4a515a, #5d6570 40%, #444b54); color: #fff; box-shadow: inset 0 0 0 1px #000, inset 0 1px 0 rgba(255, 255, 255, .25); }
  .n { font: 600 10px var(--mono); }
  .plus { font-size: 18px; line-height: 1; opacity: .7; }
  .nm { flex: 1; min-height: 0; writing-mode: vertical-rl; transform: rotate(180deg); font-size: 11.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tg { font-size: 9px; padding: 1px 3px; border-radius: 2px; background: #f4f4f0; color: #15171a; max-width: 100%; overflow: hidden; }
  .x { border: 0; background: rgba(0, 0, 0, .35); color: #fff; border-radius: 4px; width: 22px; height: 22px; display: grid; place-items: center; cursor: pointer; }
  .pickr { margin-top: 12px; display: grid; gap: 6px; padding: 12px; border-radius: 12px; background: var(--surface-2); max-height: 320px; overflow: auto; }
  .ph { display: flex; gap: 10px; align-items: center; }
  .ph .input { flex: 1; height: 34px; }
  .pr { display: grid; grid-template-columns: minmax(0, 1fr) auto auto; gap: 10px; align-items: center; padding: 8px 10px; border: 0; border-radius: 9px; background: var(--surface); text-align: left; font: inherit; color: inherit; cursor: pointer; }
  .pr:hover:not(:disabled) { box-shadow: inset 0 0 0 1.5px var(--accent); }
  .pr:disabled { opacity: .5; cursor: default; }
  .w { font-size: 11.5px; color: var(--fg-3); }
</style>
