<script lang="ts">
  // Vue 3D d'un mobilier de régie et de ses casquettes équipées (lecture seule).
  import { onDestroy } from 'svelte';
  import Icon from './Icon.svelte';
  import { deskSpec, toEl3D } from '../lib/el3d';
  import type { RackScene, Hit, Casquette3D } from '../lib/rack3d';

  let { desk, casquettes }: { desk: any; casquettes: { c: any; elements: any[] }[] } = $props();
  let host = $state<HTMLDivElement>(null!);
  let scene: RackScene | null = null;
  let loading = $state(true);
  let error = $state<string | null>(null);
  let sides = $state(true);
  let hover = $state<{ h: Hit; x: number; y: number } | null>(null);
  $effect(() => {
    const d = deskSpec(desk);
    const cs = casquettes;
    (async () => {
      try {
        if (!scene) { const { RackScene } = await import('../lib/rack3d'); scene = new RackScene(host); scene.onHover = (h, x, y) => (hover = h ? { h, x, y } : null); }
        const list: Casquette3D[] = await Promise.all(cs.map(async ({ c, elements }) => ({ id: c.id, name: c.name, offset_mm: c.offset_mm ?? 0, units: c.height_u ?? 6, depth_mm: c.depth_mm ?? 350, tilt_deg: c.tilt_deg ?? 0, els: await Promise.all(elements.map(toEl3D)) })));
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
