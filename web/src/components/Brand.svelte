<script lang="ts">
  // « Strata Arkyo » comme sur Strata Multiview : le F à plume devant le nom dans la barre du
  // haut ; centré et plus grand, avec la signature F AudioVisuel dessous, sur les pages d'accès.
  import { F_PATH, AUDIO_PATHS, VISUEL_PATHS, MODULE } from '../lib/brand';
  let { stacked = false }: { stacked?: boolean } = $props();
  const k = $derived(stacked ? 1.15 : 0.62);
  const sh = $derived(11 * k);
  // F de la signature : sa barre et son pied au niveau des lettres, sa plume au-dessus.
  const s = 38.15 / 97.3, vh = 50.95 + 5.1 * (38.15 / 97.3);
  const fh = $derived((sh * vh) / s / 104);
</script>

<span class="brand" class:stacked aria-label="Strata {MODULE}, Le Figaro AudioVisuel">
  <span class="brand-lockup">
    {#if !stacked}<svg class="brand-f-lead" viewBox="93.8 -0.2 52.3 52.95" width="22" height="22.3" aria-hidden="true"><path fill="currentColor" d={F_PATH} /></svg>{/if}
    <span class="brand-name">Strata</span>
    <span class="brand-module">{MODULE}</span>
  </span>
  {#if stacked}
    <span class="brand-sign">
      <svg viewBox="93.8 -0.2 52.3 {vh}" width={(fh * 52.3) / vh} height={fh} aria-hidden="true"><path fill="currentColor" d={F_PATH} /></svg>
      <svg viewBox="0 65.5 756 104" width={(sh * 756) / 104} height={sh} aria-hidden="true">
        <g fill="currentColor">{#each AUDIO_PATHS as d}<path {d} />{/each}</g>
        <g fill="currentColor" transform="translate(383 -110.84)">{#each VISUEL_PATHS as d}<path {d} />{/each}</g>
      </svg>
    </span>
  {/if}
</span>

<style>
  .brand { display: inline-flex; flex-direction: column; align-items: flex-start; gap: 3px; flex: none; color: var(--fg); }
  .brand-lockup { display: flex; align-items: flex-end; gap: 6px; }
  .brand-name, .brand-module { font: 800 17px/1 'Inter Variable', Inter, system-ui, sans-serif; letter-spacing: -0.035em; color: var(--fg); }
  .brand-module { font-weight: 400; letter-spacing: -0.02em; color: var(--accent); }
  .brand-f-lead { display: block; flex: none; overflow: visible; color: var(--fg); margin-right: 2px; margin-bottom: -1px; }
  .brand-sign { display: flex; align-items: flex-end; gap: 6px; color: var(--fg-2); }
  .brand-sign svg { display: block; flex: none; overflow: visible; }
  .stacked { align-items: center; gap: 12px; }
  .stacked .brand-lockup { gap: 10px; align-items: baseline; }
  .stacked .brand-name, .stacked .brand-module { font-size: 40px; }
</style>
