<script lang="ts">
  // Réglage de l'angle d'une vue 3D de la planche : la scène sans fond, au format exact du
  // cadre sur la feuille ; chaque geste (orbite, zoom, déplacement) est enregistré.
  import { onDestroy } from 'svelte';
  import type { RackScene } from '../lib/rack3d';
  import { build3D } from '../lib/sheetctx';
  import type { Ctx, SheetItem } from '../lib/sheet';

  let { it, ctx, aspect, oncamera }: { it: SheetItem; ctx: Ctx; aspect: number; oncamera: (pose: number[]) => void } = $props();
  let host = $state<HTMLDivElement>(null!);
  let scene: RackScene | null = null;
  let error = $state<string | null>(null);
  const key = $derived(JSON.stringify([it.id, it.container_id, it.doors, it.sides]));

  $effect(() => {
    void key;
    (async () => {
      try {
        if (!scene) { const { RackScene } = await import('../lib/rack3d'); scene = new RackScene(host, { transparent: true }); scene.onCamera = () => scene && oncamera(scene.pose()); }
        scene.resize();
        await build3D(scene, it, ctx);
        error = null;
      } catch (e) { error = (e as Error).message; }
    })();
  });
  export function frame(v: 'front' | 'rear' | 'quarter' | 'side' | 'top') { if (!scene) return; scene.frame(v); oncamera(scene.pose()); }
  const ro = new ResizeObserver(() => scene?.resize());
  $effect(() => { if (host) { ro.observe(host); return () => ro.unobserve(host); } });
  onDestroy(() => { ro.disconnect(); scene?.dispose(); scene = null; });
</script>

<div class="s3" bind:this={host} style="aspect-ratio:{aspect};width:min(100%, {Math.round(420 * aspect)}px)">
  {#if error}<p class="s3-err">Vue 3D indisponible : {error}</p>{/if}
</div>

<style>
  .s3 { position: relative; margin: 0 auto; border-radius: 10px; overflow: hidden;
    background: conic-gradient(#f1f3f6 25%, #fff 0 50%, #f1f3f6 0 75%, #fff 0) 0 0 / 16px 16px; box-shadow: inset 0 0 0 1px var(--line-2); }
  .s3 :global(canvas) { position: absolute; inset: 0; width: 100% !important; height: 100% !important; cursor: grab; }
  .s3-err { position: absolute; inset: 0; display: grid; place-items: center; color: #333; font-size: 12px; padding: 10px; text-align: center; }
</style>
