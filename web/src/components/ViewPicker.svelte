<script lang="ts">
  // Choix de la vue du synoptique, rangé par emplacement : vue générale, puis pour chaque
  // emplacement « Tout le lieu » et ses éléments. Valeur : overview, location:{id}, container:{id}.
  import Icon from './Icon.svelte';
  import { CONTAINER_KINDS, type ContainerKind } from '../lib/projects';

  interface Views { containers: { id: string; name: string; location_id: string; kind: string }[]; locations: { id: string; name: string; code: string }[] }
  let { views, value, onpick }: { views: Views; value: string; onpick: (v: string) => void } = $props();
  let open = $state(false);
  let root = $state<HTMLDivElement>(null!);
  const groups = $derived(views.locations.map((l) => ({ l, list: views.containers.filter((c) => c.location_id === l.id) })));
  const [kind, id] = $derived(value.split(':') as [string, string | undefined]);
  const cur = $derived(kind === 'container' ? views.containers.find((c) => c.id === id) : null);
  const curLoc = $derived(kind === 'location' ? views.locations.find((l) => l.id === id) : cur ? views.locations.find((l) => l.id === cur.location_id) : null);
  const label = $derived(kind === 'overview' ? 'Vue générale' : cur ? cur.name : `${curLoc?.name ?? ''} · tout le lieu`);
  const sub = $derived(kind === 'overview' ? 'Liaisons entre les emplacements' : cur ? `${curLoc?.name ?? ''} · ${CONTAINER_KINDS[cur.kind as ContainerKind]?.label ?? ''}` : `${groups.find((g) => g.l.id === id)?.list.length ?? 0} élément(s) de l’emplacement`);
  function pick(v: string) { open = false; onpick(v); }
</script>

<svelte:window onpointerdown={(e) => { if (open && root && !root.contains(e.target as Node)) open = false; }} onkeydown={(e) => { if (e.key === 'Escape') open = false; }} />

<div class="vp" bind:this={root}>
  <button class="vp-btn" aria-haspopup="listbox" aria-expanded={open} onclick={() => (open = !open)}>
    <span class="vp-ic"><Icon name={kind === 'overview' ? 'map' : kind === 'location' ? 'layers' : 'server'} size="sm" /></span>
    <span class="vp-t"><b>{label}</b><small>{sub}</small></span>
    <Icon name="chevd" size="sm" />
  </button>
  {#if open}
    <div class="vp-pop" role="listbox" aria-label="Vues du synoptique">
      <button role="option" aria-selected={kind === 'overview'} class="vp-it top" class:on={kind === 'overview'} onclick={() => pick('overview')}><Icon name="map" size="sm" /><span><b>Vue générale</b><small>Liaisons entre les emplacements</small></span></button>
      {#each groups as g (g.l.id)}
        <div class="vp-g">
          <div class="vp-gh"><span class="code mono">{g.l.code || '—'}</span><b>{g.l.name}</b></div>
          <button role="option" aria-selected={value === `location:${g.l.id}`} class="vp-it" class:on={value === `location:${g.l.id}`} onclick={() => pick(`location:${g.l.id}`)}><Icon name="layers" size="sm" /><span><b>Tout le lieu</b><small>{g.list.length} élément{g.list.length > 1 ? 's' : ''}</small></span></button>
          {#each g.list as c (c.id)}
            <button role="option" aria-selected={value === `container:${c.id}`} class="vp-it sub" class:on={value === `container:${c.id}`} onclick={() => pick(`container:${c.id}`)}><span><b>{c.name}</b><small>{CONTAINER_KINDS[c.kind as ContainerKind]?.label ?? ''}</small></span></button>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .vp { position: relative; }
  .vp-btn { display: flex; align-items: center; gap: 10px; min-width: 260px; max-width: 380px; height: 40px; padding: 0 12px 0 5px; border: 0; border-radius: 9px; background: var(--btn-bg, var(--surface)); box-shadow: inset 0 0 0 1px var(--btn-edge, var(--line-2)); color: var(--fg); font: inherit; text-align: left; cursor: pointer; }
  .vp-btn:hover { background: var(--btn-bg-h, var(--surface-2)); }
  .vp-ic { width: 30px; height: 30px; display: grid; place-items: center; border-radius: 7px; background: var(--surface-3); color: var(--fg-2); flex: none; }
  .vp-t { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.15; }
  .vp-t b { font-size: 14px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .vp-t small { font-size: 12px; color: var(--fg-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .vp-pop { position: absolute; left: 0; top: calc(100% + 6px); z-index: 50; width: 360px; max-height: min(70vh, 560px); overflow: auto; padding: 6px; border-radius: 12px; background: var(--surface); box-shadow: 0 0 0 1px var(--line-2), 0 24px 48px -16px rgba(0, 0, 0, .55); }
  .vp-it { display: flex; align-items: center; gap: 10px; width: 100%; padding: 7px 10px; border: 0; border-radius: 8px; background: none; color: var(--fg); font: inherit; text-align: left; cursor: pointer; }
  .vp-it.sub { padding-left: 38px; }
  .vp-it span { display: flex; flex-direction: column; line-height: 1.2; min-width: 0; }
  .vp-it b { font-size: 14px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .vp-it small { font-size: 12px; color: var(--fg-3); }
  .vp-it:hover { background: var(--surface-2); }
  .vp-it.on { background: color-mix(in srgb, var(--accent) 16%, transparent); box-shadow: inset 2px 0 0 var(--accent); }
  .vp-g { margin-top: 6px; padding-top: 6px; border-top: 1px solid var(--line); }
  .vp-gh { display: flex; align-items: center; gap: 8px; padding: 4px 10px 4px; }
  .vp-gh b { font-size: 12px; letter-spacing: .06em; text-transform: uppercase; color: var(--fg-2); }
  .vp-gh .code { font-size: 11px; padding: 2px 6px; border-radius: 5px; background: var(--surface-3); color: var(--fg-2); }
</style>
