<script lang="ts">
  // Panneau d'inspection d'un objet sélectionné dans un canevas (racks, mobilier) : en-tête
  // (vignette, surtitre, nom en grand, ID en mono, ligne secondaire, X « Désélectionner »),
  // fiche en sections de lignes libellé / valeur, actions en pied.
  // Dans le contenu : <section class="insp-sec"><h3>Titre</h3><div class="insp-row">
  // <span class="insp-k">Libellé</span><span class="insp-v">valeur</span></div></section>.
  // Un champ modifiable est un `input.cell` (ou `select.cell`) placé dans `.insp-v`.
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let { eyebrow, title, id = null, sub = null, onclose, thumb, children, actions }: {
    eyebrow?: string; title: string;
    id?: string | null; sub?: string | null; onclose: () => void;
    thumb?: Snippet; children: Snippet; actions?: Snippet;
  } = $props();
</script>

<div class="insp">
  <header class="insp-h">
    <div class="insp-top">
      {#if eyebrow}<span class="eyebrow">{eyebrow}</span>{/if}
      <button class="icon-btn sm insp-x" aria-label="Désélectionner (Échap)" title="Désélectionner (Échap)" onclick={onclose}><Icon name="x" /></button>
    </div>
    {#if thumb}<div class="insp-thumb">{@render thumb()}</div>{/if}
    <h2 class="insp-t">{title}</h2>
    {#if id || sub}<p class="insp-s">{#if id}<span class="insp-id mono">{id}</span>{/if}{#if sub}<span>{sub}</span>{/if}</p>{/if}
  </header>
  {@render children()}
  {#if actions}<footer class="insp-f">{@render actions()}</footer>{/if}
</div>

<style>
  .insp { display: flex; flex-direction: column; gap: 18px; min-width: 0; }
  .insp-h { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .insp-top { display: flex; align-items: center; gap: 8px; min-height: 30px; }
  .insp-top .eyebrow { margin: 0; }
  .insp-x { margin-left: auto; }
  .insp-thumb { display: flex; justify-content: center; align-items: center; padding: 12px 10px; margin-bottom: 6px; border-radius: var(--r-md, 12px); background: var(--sunken); box-shadow: var(--well, inset 0 0 0 1px var(--line)); overflow: hidden; }
  /* Titre : nom en grand (police display), sans le filet des titres de panneau. */
  .insp-h .insp-t { display: block; margin: 0; padding: 0; border: 0; font: 700 var(--fs-xl, 20px)/1.2 var(--display); letter-spacing: -.01em; color: var(--fg); overflow-wrap: anywhere; }
  .insp-s { display: flex; flex-wrap: wrap; align-items: baseline; gap: 2px 10px; margin: 0; font-size: var(--fs-sm, 12.5px); color: var(--fg-3); min-width: 0; }
  .insp-id { font-weight: 700; color: var(--fg-2); font-variant-numeric: tabular-nums; letter-spacing: .03em; }

  /* Sections de la fiche : surtitre, puis lignes libellé / valeur alignées. */
  .insp :global(.insp-sec) { display: flex; flex-direction: column; gap: 0; min-width: 0; }
  .insp :global(.insp-sec > h3) { margin: 0 0 4px; padding-bottom: 6px; border-bottom: 1px solid var(--line); font: 600 var(--fs-2xs, 11.5px)/1.3 var(--font); letter-spacing: .05em; text-transform: uppercase; color: var(--fg-3); }
  .insp :global(.insp-row) { display: grid; grid-template-columns: 112px minmax(0, 1fr); align-items: center; column-gap: 10px; min-height: 32px; }
  .insp :global(.insp-k) { font-size: var(--fs-sm, 12.5px); font-weight: 600; color: var(--fg-2); }
  .insp :global(.insp-v) { min-width: 0; font-size: var(--fs-md, 13.5px); color: var(--fg); font-variant-numeric: tabular-nums; overflow-wrap: anywhere; }
  .insp :global(.insp-v.mono) { font-weight: 700; letter-spacing: .03em; }
  .insp :global(.insp-v.muted) { color: var(--fg-3); }
  /* Champ modifiable : texte au repos (classe globale .cell), aligné sur les valeurs lues. */
  .insp :global(.insp-v > .cell), .insp :global(.insp-num > .cell) { margin-left: -8px; width: calc(100% + 8px); }
  .insp :global(.cell::placeholder) { color: var(--fg-3); }
  .insp :global(.cell[aria-invalid='true']), .insp :global(.cell[aria-invalid='true']:focus-visible) { box-shadow: inset 0 0 0 1px var(--err), 0 0 0 3px color-mix(in srgb, var(--err) 18%, transparent); }
  .insp :global(.cell:disabled) { cursor: default; opacity: 1; }
  .insp :global(.insp-num) { position: relative; display: block; }
  .insp :global(.insp-num .cell) { padding-right: 40px; appearance: textfield; -moz-appearance: textfield; }
  .insp :global(.insp-num .cell::-webkit-inner-spin-button), .insp :global(.insp-num .cell::-webkit-outer-spin-button) { -webkit-appearance: none; margin: 0; }
  .insp :global(.insp-v > select.cell) { padding-left: 4px; }
  .insp :global(.insp-num .u) { position: absolute; right: 8px; top: 50%; transform: translateY(-50%); font: 600 var(--fs-sm, 12.5px) var(--mono); color: var(--fg-3); pointer-events: none; }
  .insp :global(.insp-row > .help) { grid-column: 2; padding: 0 0 4px; }
  .insp :global(.insp-note) { margin: 6px 0 0; font-size: var(--fs-sm, 12.5px); color: var(--fg-3); }

  .insp-f { display: flex; flex-direction: column; align-items: flex-start; gap: 4px; padding-top: 12px; border-top: 1px solid var(--line); }
  /* Boutons fantômes alignés sur le texte de la fiche. */
  .insp-f :global(.btn.ghost) { margin-left: -11px; }
  .insp-f :global(.danger) { margin-top: 6px; }
</style>
