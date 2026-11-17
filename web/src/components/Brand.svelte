<script lang="ts" module>
  /*
   * The Strata name and the Le Figaro AudioVisuel signature (the F with its
   * quill, then "Audio" bold and "Visuel" light, as on the Le Figaro
   * Audiovisuel logo, the two words joined). Composant commun de la famille
   * Strata (annexe A du design commun), en Svelte.
   */
  import { F_PATH as F, AUDIO_PATHS as AUDIO, VISUEL_PATHS as VISUEL, MODULE } from '../lib/brand';
  /** F units per AudioVisuel unit: the F (bar 12.6 → foot 50.75) over the letters (67.1 → 164.4). */
  const S = 38.15 / 97.3;
  /** From the quill's top to the AudioVisuel box's bottom (5.1 below its baseline). */
  const VH = 50.95 + 5.1 * S;
</script>

<script lang="ts">
  /**
   * « Strata Arkyo » with the F AudioVisuel signature, small, under it (top
   * bar). `stacked`: centred and larger (login page).
   */
  let { stacked = false }: { stacked?: boolean } = $props();
  // The signature stays well below the name.
  const k = $derived(stacked ? 1.15 : 0.62);
  const height = $derived(11 * k);
  // The F for AudioVisuel `height` high: its bar and foot level with the letters' top and baseline, its quill rising above.
  const fh = $derived((height * VH) / S / 104);
</script>

<div class="brand" class:stacked aria-label="Strata {MODULE}, Le Figaro AudioVisuel">
  <span class="brand-lockup">
    <!-- The F with its quill before the name (the top bar): as tall as the name, its quill above. -->
    {#if !stacked}
      <svg class="brand-f-lead" viewBox="93.8 -0.2 52.3 52.95" width="23.7" height="24" aria-hidden="true"><path fill="currentColor" d={F} /></svg>
    {/if}
    <span class="brand-name">Strata</span>
    <span class="brand-module">{MODULE}</span>
  </span>
  <!-- The signature on the login page only: too small in the top bar. -->
  {#if stacked}
    <span class="brand-sign">
      <svg class="brand-f" viewBox="93.8 -0.2 52.3 {VH}" width={(fh * 52.3) / VH} height={fh} aria-hidden="true"><path fill="currentColor" d={F} /></svg>
      <svg class="brand-av" viewBox="0 65.5 756 104" width={(height * 756) / 104} height={height} aria-hidden="true">
        <g fill="currentColor">{#each AUDIO as d}<path {d} />{/each}</g>
        <g fill="currentColor" transform="translate(383 -110.84)">{#each VISUEL as d}<path {d} />{/each}</g>
      </svg>
    </span>
  {/if}
</div>
