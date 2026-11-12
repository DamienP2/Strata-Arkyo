<script lang="ts">
  // Vue 3D d'un mobilier de régie et de ses casquettes équipées (lecture seule).
  import { onDestroy } from 'svelte';
  import Icon from './Icon.svelte';
  import { api } from '../lib/api';
  import { fnColor, refById } from '../lib/catalog.svelte';
  import { ALU, accessoryFace, visualOf } from '../lib/faceplate';
  import type { RackScene, El3D, Hit, Casquette3D } from '../lib/rack3d';

  let { desk, casquettes }: { desk: any; casquettes: { c: any; elements: any[] }[] } = $props();
  let host = $state<HTMLDivElement>(null!);
  let scene: RackScene | null = null;
  let loading = $state(true);
  let error = $state<string | null>(null);
  let sides = $state(true);
  let hover = $state<{ h: Hit; x: number; y: number } | null>(null);
  const portsCache = new Map<string, any[]>();
  async function ports(itemId: string | null): Promise<any[]> {
    if (!itemId) return [];
    if (!portsCache.has(itemId)) { try { const r = await api.get<{ item: { data: { ports?: any[] } } }>(`/catalog/items/${itemId}`); portsCache.set(itemId, r.item.data.ports ?? []); } catch { portsCache.set(itemId, []); } }
    return portsCache.get(itemId)!;
  }
  const toEl = async (e: any): Promise<El3D> => {
    const kind = e.nature === 'accessory' ? (e.accessory?.kind === 'panel' ? 'panel' : e.accessory?.kind === 'plate' ? (e.accessory.variant === 'brush' ? 'brush' : e.accessory.variant === 'vented' ? 'vent' : 'blank') : accessoryFace(e.label)) : visualOf(e);
    const ps = (await ports(e.item_id)).map((p: any) => ({ face: p.face ?? 'rear', connector: p.connector_id ?? '', quantity: p.quantity ?? 1, color: p.signal_id ? (refById('signals', p.signal_id)?.data?.color ?? null) : null, label: p.label ?? null }));
    return { id: e.placement.id, label: e.label, tag: e.tag, nature: e.nature, height_u: e.height_u, depth_mm: e.depth_mm, function: e.function, brand: e.brand ?? null, face: e.placement.face, u_start: e.placement.u_start, width: e.placement.width, col: e.placement.col, kind, alu: ALU.has(kind), accessory: e.accessory, ports: ps, fnColor: fnColor(e.function), shelf_id: e.placement.shelf_id ?? null } as El3D;
  };
  $effect(() => {
    const d = { name: desk.name, length_mm: desk.length_mm ?? 2400, depth_mm: desk.width_mm ?? 900, height_mm: desk.outer_height_mm ?? 750 };
    const cs = casquettes;
    (async () => {
      try {
        if (!scene) { const { RackScene } = await import('../lib/rack3d'); scene = new RackScene(host); scene.onHover = (h, x, y) => (hover = h ? { h, x, y } : null); }
        const list: Casquette3D[] = await Promise.all(cs.map(async ({ c, elements }) => ({ id: c.id, name: c.name, offset_mm: c.offset_mm ?? 0, units: c.height_u ?? 6, depth_mm: c.depth_mm ?? 350, tilt_deg: c.tilt_deg ?? 0, els: await Promise.all(elements.map(toEl)) })));
        scene.buildDesk(d, list); scene.setSides(sides); error = null;
      } catch (e) { error = (e as Error).message; } finally { loading = false; }
    })();
  });
  $effect(() => { scene?.setSides(sides); });
  const ro = new ResizeObserver(() => scene?.resize());
  $effect(() => { if (host) { ro.observe(host); return () => ro.unobserve(host); } });
  onDestroy(() => { ro.disconnect(); scene?.dispose(); scene = null; });
  function shot() { if (!scene) return; const a = document.createElement('a'); a.href = scene.snapshot(); a.download = `${String(desk.name).replace(/[^\p{L}\p{N}-]+/gu, '_')}_3D.png`; a.click(); }
</script>

<div class="d3">
  <div class="d3-bar">
    <div class="seg" role="group" aria-label="Cadrage">{#each [['quarter', '3/4'], ['front', 'Opérateur'], ['rear', 'Arrière'], ['side', 'Côté'], ['top', 'Dessus']] as [k, l]}<button onclick={() => scene?.frame(k as any)}>{l}</button>{/each}</div>
    <span class="d3-l">Joues</span><div class="seg" role="group" aria-label="Joues"><button class:on={sides} onclick={() => (sides = true)}>Avec</button><button class:on={!sides} onclick={() => (sides = false)}>Sans</button></div>
    <span style="flex:1"></span>
    <span class="muted small">Glisser : tourner · molette : zoom · clic droit : déplacer</span>
    <button class="btn sm ghost" onclick={shot}><Icon name="download" size="sm" />PNG</button>
  </div>
  <div class="d3-view" bind:this={host}>
    {#if loading}<span class="spin d3-spin"></span>{/if}
    {#if error}<p class="d3-err">Vue 3D indisponible : {error}</p>{/if}
    {#if hover}<div class="d3-tip" style="left:{hover.x + 14}px;top:{hover.y + 10}px"><b>{hover.h.label}</b>{#if hover.h.tag}<span class="mono"> {hover.h.tag}</span>{/if}<br><span>{hover.h.where ? `${hover.h.where} · ` : ''}U{hover.h.u}</span></div>{/if}
  </div>
</div>

<style>
  .d3 { display: flex; flex-direction: column; height: 100%; }
  .d3-bar { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; padding: 8px 12px; border-bottom: 1px solid var(--line); }
  .d3-view { position: relative; flex: 1; min-height: 520px; background: #2b2e33; overflow: hidden; }
  .d3-view :global(canvas) { position: absolute; inset: 0; display: block; width: 100% !important; height: 100% !important; cursor: grab; }
  .d3-spin { position: absolute; left: 50%; top: 50%; }
  .d3-err { position: absolute; inset: 0; display: grid; place-items: center; color: #fff; }
  .d3-tip { position: absolute; pointer-events: none; background: rgba(15, 16, 18, .9); color: #fff; padding: 6px 9px; border-radius: 8px; font-size: 12.5px; line-height: 1.35; max-width: 280px; }
  .d3-l { font: 600 10px var(--mono); letter-spacing: .08em; text-transform: uppercase; color: var(--fg-3); }
</style>
