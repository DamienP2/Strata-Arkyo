<script lang="ts">
  // Vue 3D réaliste d'une baie ou d'un flight-case (lecture seule) : orbite, cadrages,
  // portes, panneaux latéraux, survol des équipements, capture PNG.
  import { onDestroy } from 'svelte';
  import Icon from './Icon.svelte';
  import { api } from '../lib/api';
  import { fnColor, refById } from '../lib/catalog.svelte';
  import { ALU, accessoryFace, visualOf } from '../lib/faceplate';
  import type { RackScene, El3D, Hit } from '../lib/rack3d';

  let { container, elements, title }: { container: any; elements: any[]; title: string } = $props();
  let host = $state<HTMLDivElement>(null!);
  let scene: RackScene | null = null;
  let loading = $state(true);
  let error = $state<string | null>(null);
  let doors = $state<'closed' | 'open' | 'removed'>('open'), sides = $state(true);
  let hover = $state<{ h: Hit; x: number; y: number } | null>(null);
  const portsCache = new Map<string, any[]>();

  async function ports(itemId: string | null): Promise<any[]> {
    if (!itemId) return [];
    if (!portsCache.has(itemId)) { try { const r = await api.get<{ item: { data: { ports?: any[] } } }>(`/catalog/items/${itemId}`); portsCache.set(itemId, r.item.data.ports ?? []); } catch { portsCache.set(itemId, []); } }
    return portsCache.get(itemId)!;
  }
  async function toEls(): Promise<El3D[]> {
    return Promise.all(elements.map(async (e) => {
      const kind = e.nature === 'accessory' ? (e.accessory?.kind === 'panel' ? 'panel' : e.accessory?.kind === 'plate' ? (e.accessory.variant === 'brush' ? 'brush' : e.accessory.variant === 'vented' ? 'vent' : 'blank') : accessoryFace(e.label)) : visualOf(e);
      const ps = (await ports(e.item_id)).map((p: any) => ({ face: p.face ?? 'rear', connector: p.connector_id ?? '', quantity: p.quantity ?? 1, color: p.signal_id ? (refById('signals', p.signal_id)?.data?.color ?? null) : null, label: p.label ?? null }));
      return { id: e.placement.id, label: e.label, tag: e.tag, nature: e.nature, height_u: e.height_u, depth_mm: e.depth_mm, function: e.function, brand: e.brand ?? null, face: e.placement.face, u_start: e.placement.u_start, width: e.placement.width, col: e.placement.col, kind, alu: ALU.has(kind), accessory: e.accessory, ports: ps, fnColor: fnColor(e.function), shelf_id: e.placement.shelf_id ?? null } as El3D;
    }));
  }
  const spec = $derived({ kind: container.kind ?? 'rack', name: container.name, units: container.height_u ?? 42, width_mm: container.width_mm ?? null, depth_mm: container.depth_mm ?? container.length_mm ?? null, height_mm: container.outer_height_mm ?? null, rear_rails_mm: container.rear_rails_mm ?? null, wheels: !!container.wheels });

  $effect(() => {
    const s = spec, els = elements; void els;
    (async () => {
      try {
        if (!scene) { const { RackScene } = await import('../lib/rack3d'); scene = new RackScene(host); scene.onHover = (h, x, y) => (hover = h ? { h, x, y } : null); }
        scene.build(s, await toEls()); scene.setDoors(doors); scene.setSides(sides); error = null;
      } catch (e) { error = (e as Error).message; } finally { loading = false; }
    })();
  });
  $effect(() => { scene?.setDoors(doors); });
  $effect(() => { scene?.setSides(sides); });
  const ro = new ResizeObserver(() => scene?.resize());
  $effect(() => { if (host) { ro.observe(host); return () => ro.unobserve(host); } });
  onDestroy(() => { ro.disconnect(); scene?.dispose(); scene = null; });
  function shot() { if (!scene) return; const a = document.createElement('a'); a.href = scene.snapshot(); a.download = `${title.replace(/[^\p{L}\p{N}-]+/gu, '_')}_3D.png`; a.click(); }
  const isCase = $derived(/case/.test(spec.kind));
</script>

<div class="r3">
  <div class="r3-bar">
    <div class="seg" role="group" aria-label="Cadrage">{#each [['quarter', '3/4'], ['front', 'Avant'], ['rear', 'Arrière'], ['side', 'Côté'], ['top', 'Dessus']] as [k, l]}<button onclick={() => scene?.frame(k as any)}>{l}</button>{/each}</div>
    {#if !isCase}<span class="r3-l">Portes</span><div class="seg" role="group" aria-label="Portes">{#each [['closed', 'Fermées'], ['open', 'Ouvertes'], ['removed', 'Retirées']] as [k, l]}<button class:on={doors === k} onclick={() => (doors = k as typeof doors)}>{l}</button>{/each}</div>{/if}
    <span class="r3-l">{isCase ? 'Parois' : 'Flancs'}</span><div class="seg" role="group" aria-label="Panneaux latéraux"><button class:on={sides} onclick={() => (sides = true)}>Avec</button><button class:on={!sides} onclick={() => (sides = false)}>Sans</button></div>
    <span style="flex:1"></span>
    <span class="muted small">Glisser : tourner · molette : zoom · clic droit : déplacer</span>
    <button class="btn sm ghost" onclick={shot}><Icon name="download" size="sm" />PNG</button>
  </div>
  <div class="r3-view" bind:this={host}>
    {#if loading}<span class="spin r3-spin"></span>{/if}
    {#if error}<p class="r3-err">Vue 3D indisponible : {error}</p>{/if}
    {#if hover}<div class="r3-tip" style="left:{hover.x + 14}px;top:{hover.y + 10}px"><b>{hover.h.label}</b>{#if hover.h.tag}<span class="mono"> {hover.h.tag}</span>{/if}<br><span>U{hover.h.u}{hover.h.height > 1 ? `–${hover.h.u + Math.ceil(hover.h.height) - 1}` : ''} · {hover.h.face === 'front' ? 'avant' : 'arrière'}</span></div>{/if}
  </div>
</div>

<style>
  .r3 { display: flex; flex-direction: column; height: 100%; min-height: 560px; }
  .r3-bar { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; padding: 8px 12px; border-bottom: 1px solid var(--line); }
  .r3-view { position: relative; flex: none; height: max(560px, calc(100vh - 300px)); background: #2b2e33; overflow: hidden; border-radius: 0 0 16px 16px; }
  .r3-view :global(canvas) { position: absolute; inset: 0; display: block; width: 100% !important; height: 100% !important; cursor: grab; }
  .r3-spin { position: absolute; left: 50%; top: 50%; }
  .r3-err { position: absolute; inset: 0; display: grid; place-items: center; color: #fff; }
  .r3-tip { position: absolute; pointer-events: none; background: rgba(15, 16, 18, .9); color: #fff; padding: 6px 9px; border-radius: 8px; font-size: 12.5px; line-height: 1.35; max-width: 280px; }
  .r3-l { font: 600 10px var(--mono); letter-spacing: .08em; text-transform: uppercase; color: var(--fg-3); }
</style>
