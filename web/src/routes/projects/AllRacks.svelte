<script lang="ts">
  // Toutes les baies côte à côte. Un équipement glissé d'une baie à une autre change
  // d'élément dans l'inventaire (un exemplaire à la fois) et y est posé à l'endroit lâché ;
  // dans la même baie, il change simplement de position. La main est prise le temps du geste.
  import Icon from '../../components/Icon.svelte';
  import PanelFace from '../../components/PanelFace.svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { navigate } from '../../lib/router.svelte';
  import { refById } from '../../lib/catalog.svelte';
  import { ALU, decor, accFace, visualOf } from '../../lib/faceplate';
  import { rackLines } from '../../lib/racktext.svelte';
  import type { Accessory, ProjectView } from '../../lib/projects';

  import RackPicker from '../../components/RackPicker.svelte';
  let { view, reload, loc = null }: { view: ProjectView; reload?: () => Promise<void>; loc?: string | null } = $props();
  type Face = 'front' | 'rear';
  interface El { placement: { id: string; article_id: string; instance_id: string | null; face: string; u_start: number; width: string; col: number; shelf_id: string | null; rev: number }; label: string; tag: string | null; custom?: string | null; brand?: string | null; nature: string; height_u: number; function: string | null; accessory: Accessory | null; article_rev: number; type_id?: string | null; visual?: string | null }
  interface R { container: any; elements: El[]; lock: { mine: boolean; account_name: string } | null }

  // Un emplacement choisi (ou tous), rangés par emplacement.
  const racks = $derived(view.locations.flatMap((l) => (loc && l.id !== loc ? [] : view.containers.filter((c) => c.rackable && c.kind !== 'casquette' && c.location_id === l.id))));
  let data = $state<Record<string, R>>({});
  let face = $state<Face>('front');
  let scale = $state(24);
  let busy = $state(false);
  const canDesign = $derived(can('conception', 'ecriture'));

  async function loadAll() {
    const out: Record<string, R> = {};
    await Promise.all(racks.map(async (c) => { try { out[c.id] = await api.get<R>(`/containers/${c.id}/rack`); } catch (e) { fail(e); } }));
    data = out;
  }
  $effect(() => { racks.length; loadAll(); });

  const cols = (w: string) => ({ full: 1, half: 2, third: 3, quarter: 4 } as Record<string, number>)[w] ?? 1;
  const fnColor = (f: string | null) => (f ? (refById('functions', f)?.data?.color ?? 'var(--fg-3)') : 'var(--fg-3)');
  const typeOf = (e: El) => (e.nature === 'accessory' ? accFace(e as any) : visualOf(e));
  const panelOf = (e: El) => (e.accessory?.kind === 'panel' ? e.accessory.panel : null);
  function style(e: El) {
    const n = cols(e.placement.width), l = (100 * e.placement.col) / n, w = 100 / n;
    return `--c:${fnColor(e.function)};bottom:calc(${e.placement.u_start - 1} * var(--u) + 1px);height:calc(${e.height_u} * var(--u) - 2px);left:calc(${l}% + 2px);width:calc(${w}% - 4px)`;
  }

  // ----- Glisser-déposer -----
  let drag = $state<{ from: string; e: El } | null>(null);
  let over = $state<{ cid: string; u: number } | null>(null);
  function uAt(ev: DragEvent, el: HTMLElement, units: number) {
    const r = el.getBoundingClientRect();
    const h = drag?.e.height_u ?? 1;
    const u = Math.floor((r.bottom - ev.clientY) / scale) + 1 - Math.floor(h / 2);
    return Math.max(1, Math.min(units - Math.ceil(h) + 1, u));
  }
  async function lock(cid: string): Promise<boolean> {
    const r = await api.post<{ mine: boolean; account_name: string }>(`/containers/${cid}/rack/lock`);
    if (!r.mine) toast(`${r.account_name} modifie « ${data[cid]?.container.name} » : réessayez plus tard.`, 'warn');
    return r.mine;
  }
  async function unlock(cid: string) { try { await api.del(`/containers/${cid}/rack/lock`); } catch { /* déjà libre */ } }
  async function dropOn(cid: string, u: number) {
    const d = drag; drag = null; over = null;
    if (!d || busy) return;
    const e = d.e;
    if (d.from === cid && e.placement.u_start === u && e.placement.face === face) return;
    busy = true;
    const locked: string[] = [];
    try {
      for (const id of new Set([d.from, cid])) { if (!(await lock(id))) return; locked.push(id); }
      if (d.from === cid) {
        await api.patch(`/placements/${e.placement.id}`, { rev: e.placement.rev, face, u_start: u, width: e.placement.width, col: e.placement.col, circuit_id: null });
      } else {
        // Retirer de la baie d'origine, changer d'élément dans l'inventaire, poser dans la nouvelle.
        await api.del(`/placements/${e.placement.id}`);
        const dest = view.containers.find((c) => c.id === cid)!;
        const moved = await api.post<{ id: string }>(`/articles/${e.placement.article_id}/move`, { rev: e.article_rev, container_id: cid, location_id: dest.location_id, instance_id: e.placement.instance_id });
        try {
          const r = await api.post<{ adjusted: boolean; placement: { u_start: number } }>(`/containers/${cid}/placements`, { article_id: moved.id, instance_id: e.placement.instance_id, face, u_start: u, col: 0 });
          toast(`${e.label} déplacé dans « ${dest.name} »${r.adjusted ? `, en U${r.placement.u_start}` : ''}.`);
        } catch (x) {
          toast(`${e.label} est dans « ${dest.name} », à placer : ${(x as Error).message}`, 'warn');
        }
      }
    } catch (x) { fail(x); }
    finally {
      for (const id of locked) await unlock(id);
      busy = false;
      await loadAll();
      await reload?.();
    }
  }
</script>

<div class="all-tool">
  <RackPicker {view} value={loc ? `tout:${loc}` : 'tout'} onpick={(v) => navigate(`/projets/${view.project.id}/racks/${v.replace(':', '/')}`)} />
  {#if view.locations.filter((l) => view.containers.some((c) => c.rackable && c.kind !== 'casquette' && c.location_id === l.id)).length > 1}
    <div class="seg" role="group" aria-label="Emplacement">
      <button class:on={!loc} onclick={() => navigate(`/projets/${view.project.id}/racks/tout`)}>Tous</button>
      {#each view.locations.filter((l) => view.containers.some((c) => c.rackable && c.kind !== 'casquette' && c.location_id === l.id)) as l (l.id)}<button class:on={loc === l.id} onclick={() => navigate(`/projets/${view.project.id}/racks/tout/${l.id}`)}>{l.name}</button>{/each}
    </div>
  {/if}
  <div class="seg" role="group" aria-label="Face"><button class:on={face === 'front'} onclick={() => (face = 'front')}>Avant</button><button class:on={face === 'rear'} onclick={() => (face = 'rear')}>Arrière</button></div>
  <div class="seg" role="group" aria-label="Taille"><button class:on={scale === 18} onclick={() => (scale = 18)}>S</button><button class:on={scale === 24} onclick={() => (scale = 24)}>M</button><button class:on={scale === 32} onclick={() => (scale = 32)}>L</button></div>
  <span class="muted small">{#if canDesign}Glissez un équipement vers une autre baie : il change d’élément dans l’inventaire.{:else}Lecture seule.{/if}</span>
  {#if busy}<span class="spin"></span>{/if}
</div>

<div class="all-racks">
  {#each racks as c (c.id)}
    {@const r = data[c.id]}
    {@const units = c.height_u ?? 0}
    <div class="ar-col">
      <div class="ar-head"><b>{c.name}</b><span class="muted small">{view.locations.find((l) => l.id === c.location_id)?.name} · {units} U</span>
        {#if r?.lock && !r.lock.mine}<span class="chip warn"><Icon name="lock" />{r.lock.account_name}</span>{/if}
        <a class="btn sm ghost" href="/projets/{view.project.id}/racks/{c.id}" title="Ouvrir cette baie"><Icon name="arrowr" size="sm" /></a></div>
      <div class="rack" style="--units:{units};--u:{scale}px;--bw:{Math.round(scale * 10.86) - 22}px">
        <div class="rack-scale">{#each Array(units) as _, i}<span>{units - i}</span>{/each}</div>
        <div class="rack-body" role="grid" tabindex="-1" aria-label="Élévation de {c.name}" class:target={over?.cid === c.id}
          ondragover={(ev) => { if (drag) { ev.preventDefault(); over = { cid: c.id, u: uAt(ev, ev.currentTarget as HTMLElement, units) }; } }}
          ondragleave={() => { if (over?.cid === c.id) over = null; }}
          ondrop={(ev) => { ev.preventDefault(); if (drag) dropOn(c.id, uAt(ev, ev.currentTarget as HTMLElement, units)); }}>
          {#each (r?.elements ?? []).filter((e) => e.placement.face === face && !e.placement.shelf_id) as e (e.placement.id)}
            <div class="ritem {e.nature === 'accessory' ? 'acc' : ''} {ALU.has(typeOf(e)) ? 'alu' : ''} {e.height_u <= 1 ? 'u1' : ''} {e.placement.width !== 'full' ? `half c${e.placement.col}` : ''}" class:dragged={drag?.e === e}
              style={style(e)} role="button" tabindex="0" draggable={canDesign && !busy} title="{e.custom ? `${e.custom} · ` : ''}{e.tag ? `${e.tag} · ` : ''}{e.label} · U{e.placement.u_start}"
              ondragstart={(ev) => { drag = { from: c.id, e }; ev.dataTransfer?.setData('text/plain', e.placement.id); }} ondragend={() => { drag = null; over = null; }}>
              {#if e.nature !== 'accessory'}<i class="led"></i>{/if}<span class="rl" class:two={!!rackLines(e)[1]}>{#if rackLines(e)[0]}<b class="rn" class:id={!!e.tag && rackLines(e)[0] === e.tag}>{rackLines(e)[0]}</b>{/if}{#if rackLines(e)[1]}<small>{rackLines(e)[1]}</small>{/if}</span>{#if panelOf(e)}<span class="pdec"><PanelFace panel={panelOf(e)!} /></span>{:else}<span class="dec">{@html decor(typeOf(e))}</span>{/if}
            </div>
          {/each}
          {#if drag && over?.cid === c.id}
            <div class="rghost" style="bottom:calc({over.u - 1} * var(--u));height:calc({drag.e.height_u} * var(--u));left:2px;width:calc(100% - 4px)"></div>
          {/if}
        </div>
        <div class="rack-scale">{#each Array(units) as _, i}<span>{units - i}</span>{/each}</div>
      </div>
    </div>
  {/each}
</div>

<style>
  .all-tool { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .all-racks { display: flex; gap: 22px; align-items: flex-start; justify-content: safe center; overflow-x: auto; padding: 18px 6px 24px; border-radius: 12px; background: radial-gradient(120% 90% at 50% 0%, var(--stage-a), var(--stage-b)); box-shadow: inset 0 0 0 1px var(--line); }
  .ar-col { flex: none; display: flex; flex-direction: column; gap: 8px; }
  .ar-col :global(.rack-body) { width: var(--bw) !important; }
  .ar-col :global(.rack) { grid-template-columns: 22px auto 22px; }
  .ar-head { display: flex; align-items: center; gap: 8px; padding: 0 4px; max-width: calc(var(--bw, 300px) + 60px); }
  .ar-head b { font: 650 14px var(--display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ar-head .muted { margin-right: auto; white-space: nowrap; }
  .rack-body.target { outline: 2px dashed var(--accent); outline-offset: 2px; }
  .ritem.dragged { opacity: .35; }
  .ritem[draggable='true'] { cursor: grab; }
  .pdec { flex: 1 1 0; min-width: 0; align-self: stretch; display: block; padding: 2px 0; margin-left: 8px; }
</style>
