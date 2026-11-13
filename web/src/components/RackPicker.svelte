<script lang="ts">
  // Sélecteur des éléments rackables d'un projet, rangés par emplacement : chaque baie avec son
  // type, sa hauteur et son remplissage ; en tête, « Toutes les baies » (du projet ou d'un
  // emplacement). Valeur : un id d'élément, « tout » ou « tout:{emplacement} ».
  import Icon from './Icon.svelte';
  import { CONTAINER_KINDS, type ProjectView } from '../lib/projects';
  import { nf } from '../lib/format';

  let { view, value, onpick }: { view: ProjectView; value: string; onpick: (v: string) => void } = $props();
  let open = $state(false);
  let root = $state<HTMLDivElement>(null!);
  const racks = $derived(view.containers.filter((c) => c.rackable && c.kind !== 'casquette'));
  const groups = $derived(view.locations.map((l) => ({ l, list: racks.filter((c) => c.location_id === l.id) })).filter((g) => g.list.length));
  const cur = $derived(racks.find((c) => c.id === value) ?? null);
  const locOf = (v: string) => view.locations.find((l) => l.id === v.split(':')[1]);
  const label = $derived(cur ? cur.name : value.startsWith('tout:') ? `Toutes les baies · ${locOf(value)?.name ?? ''}` : 'Toutes les baies');
  const sub = $derived(cur ? `${view.locations.find((l) => l.id === cur.location_id)?.name ?? ''} · ${CONTAINER_KINDS[cur.kind].label} · ${cur.height_u} U` : `${value.startsWith('tout:') ? groups.find((g) => g.l.id === value.split(':')[1])?.list.length ?? 0 : racks.length} élément(s) côte à côte`);
  const pct = (c: (typeof racks)[number]) => (c.height_u ? Math.min(100, (c.totals.used_u / c.height_u) * 100) : 0);
  function pick(v: string) { open = false; onpick(v); }
</script>

<svelte:window onpointerdown={(e) => { if (open && root && !root.contains(e.target as Node)) open = false; }} onkeydown={(e) => { if (e.key === 'Escape') open = false; }} />

<div class="rp" bind:this={root}>
  <button class="rp-btn" aria-haspopup="listbox" aria-expanded={open} onclick={() => (open = !open)}>
    <span class="rp-ic"><Icon name={cur ? 'server' : 'grid'} size="sm" /></span>
    <span class="rp-t"><b>{label}</b><small>{sub}</small></span>
    <Icon name="chevd" size="sm" />
  </button>
  {#if open}
    <div class="rp-pop" role="listbox" aria-label="Éléments rackables">
      {#if racks.length > 1}
        <button role="option" aria-selected={value === 'tout'} class="rp-all" class:on={value === 'tout'} onclick={() => pick('tout')}><Icon name="grid" size="sm" /><span><b>Toutes les baies côte à côte</b><small>{racks.length} éléments, tous emplacements</small></span></button>
      {/if}
      {#each groups as g (g.l.id)}
        <div class="rp-g">
          <div class="rp-gh"><span class="code mono">{g.l.code || '—'}</span><b>{g.l.name}</b>{#if g.list.length > 1}<button class="rp-gall" class:on={value === `tout:${g.l.id}`} onclick={() => pick(`tout:${g.l.id}`)}>Toutes ({g.list.length})</button>{/if}</div>
          {#each g.list as c (c.id)}
            <button role="option" aria-selected={value === c.id} class="rp-it" class:on={value === c.id} onclick={() => pick(c.id)}>
              <span class="rp-n"><b>{c.name}</b><small>{CONTAINER_KINDS[c.kind].label}</small></span>
              <span class="rp-fill" class:over={c.totals.over_u}><i style="width:{pct(c)}%"></i></span>
              <span class="rp-u mono">{nf(c.totals.used_u, c.totals.used_u % 1 ? 1 : 0)} / {c.height_u} U</span>
            </button>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .rp { position: relative; }
  .rp-btn { display: flex; align-items: center; gap: 10px; min-width: 280px; max-width: 420px; height: 44px; padding: 0 12px 0 6px; border: 0; border-radius: 10px; background: var(--btn-bg, var(--surface)); box-shadow: inset 0 0 0 1px var(--btn-edge, var(--line-2)); color: var(--fg); font: inherit; text-align: left; cursor: pointer; }
  .rp-btn:hover { background: var(--btn-bg-h, var(--surface-2)); }
  .rp-ic { width: 32px; height: 32px; display: grid; place-items: center; border-radius: 8px; background: var(--surface-3); color: var(--fg-2); flex: none; }
  .rp-t { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.15; }
  .rp-t b { font-size: 14px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rp-t small { font-size: 12px; color: var(--fg-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rp-pop { position: absolute; left: 0; top: calc(100% + 6px); z-index: 50; width: 420px; max-height: min(70vh, 560px); overflow: auto; padding: 6px; border-radius: 12px; background: var(--surface); box-shadow: 0 0 0 1px var(--line-2), 0 24px 48px -16px rgba(0, 0, 0, .55); }
  .rp-all, .rp-it { display: flex; align-items: center; gap: 10px; width: 100%; padding: 8px 10px; border: 0; border-radius: 8px; background: none; color: var(--fg); font: inherit; text-align: left; cursor: pointer; }
  .rp-all:hover, .rp-it:hover { background: var(--surface-2); }
  .rp-all.on, .rp-it.on { background: color-mix(in srgb, var(--accent) 16%, transparent); box-shadow: inset 2px 0 0 var(--accent); }
  .rp-all span { display: flex; flex-direction: column; line-height: 1.2; }
  .rp-all small, .rp-n small { font-size: 12px; color: var(--fg-3); }
  .rp-g { margin-top: 6px; padding-top: 6px; border-top: 1px solid var(--line); }
  .rp-gh { display: flex; align-items: center; gap: 8px; padding: 4px 10px 6px; }
  .rp-gh b { font-size: 12px; letter-spacing: .06em; text-transform: uppercase; color: var(--fg-2); }
  .rp-gh .code { font-size: 11px; padding: 2px 6px; border-radius: 5px; background: var(--surface-3); color: var(--fg-2); }
  .rp-gall { margin-left: auto; height: 24px; padding: 0 8px; border: 0; border-radius: 6px; background: var(--surface-2); color: var(--fg-2); font: 600 12px var(--font); cursor: pointer; }
  .rp-gall:hover, .rp-gall.on { background: color-mix(in srgb, var(--accent) 18%, transparent); color: var(--fg); }
  .rp-n { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.2; }
  .rp-n b { font-size: 14px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rp-fill { flex: none; width: 70px; height: 6px; border-radius: 3px; background: var(--surface-3); overflow: hidden; }
  .rp-fill i { display: block; height: 100%; background: var(--ok); }
  .rp-fill.over i { background: var(--err); }
  .rp-u { flex: none; width: 74px; text-align: right; font-size: 12px; color: var(--fg-2); }
</style>
