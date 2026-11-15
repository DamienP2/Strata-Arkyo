<script lang="ts">
  // Menu déroulant commun : bouton aria-haspopup + .pop-menu (role=menu), flèches/Début/Fin/Échap (menuKeys), clic dehors (outside), fermeture au choix d'un article.
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import { menuKeys } from '../lib/menukeys';
  import { outside } from '../lib/outside';
  let { label, icon, iconOnly = false, chevron, btnClass = 'btn', class: wrapClass = '', menuClass = '', align = 'right', menuLabel,
    open = $bindable(false), disabled = false, title, trigger, children }: {
    /** Nom du menu : texte du bouton, ou aria-label et title avec `iconOnly` (à poser aussi pour un `trigger` sans texte). */
    label: string;
    icon?: string;
    /** Bouton sans texte visible (icône seule avec btnClass="icon-btn sm", ou avatar en `trigger`). */
    iconOnly?: boolean;
    /** Chevron après le texte (par défaut : oui, sauf icône seule ou `trigger`). */
    chevron?: boolean;
    /** Classes du bouton d'ouverture (« btn », « btn sm », « btn ghost », « icon-btn sm »…). */
    btnClass?: string;
    /** Classes du conteneur (position relative). */
    class?: string;
    /** Classes ajoutées au panneau .pop-menu. */
    menuClass?: string;
    /** Bord du bouton auquel le panneau s'aligne. */
    align?: 'left' | 'right';
    /** Nom accessible du panneau (par défaut `label`). */
    menuLabel?: string;
    open?: boolean;
    disabled?: boolean;
    /** Infobulle du bouton (par défaut `label` pour une icône seule). */
    title?: string;
    /** Contenu libre du bouton (remplace icône, texte et chevron). */
    trigger?: Snippet;
    /** Articles : `<button role="menuitem">` ou `<a role="menuitem" href>` ; reçoit `close`. */
    children: Snippet<[() => void]>;
  } = $props();
  let btn = $state<HTMLButtonElement>();
  const compact = $derived(iconOnly);
  const showChev = $derived(chevron ?? !(iconOnly || trigger));
  function close() { open = false; }
  // Choisir un article ferme le menu et rend le focus au bouton (une boîte ouverte par
  // l'article le rendra ensuite à ce bouton).
  function picked(e: MouseEvent) {
    if (!(e.target as Element).closest('[role^="menuitem"]')) return;
    open = false;
    btn?.focus();
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape' && open) { e.preventDefault(); e.stopPropagation(); open = false; btn?.focus(); }
  }
</script>

<div class="menu-wrap {wrapClass}" use:outside={() => { if (open) open = false; }} onkeydown={key} role="presentation">
  <button type="button" bind:this={btn} class={btnClass} aria-haspopup="menu" aria-expanded={open} {disabled}
    aria-label={compact ? label : undefined} title={title ?? (compact ? label : undefined)} onclick={() => (open = !open)}>
    {#if trigger}{@render trigger()}{:else}
      {#if icon}<Icon name={icon} size={iconOnly && !btnClass.includes('sm') ? '' : 'sm'} />{/if}
      {#if !iconOnly}{label}{/if}
      {#if showChev}<Icon name="chevd" size="sm" />{/if}
    {/if}
  </button>
  {#if open}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="pop-menu {menuClass}" class:left={align === 'left'} role="menu" tabindex="-1" aria-label={menuLabel ?? label}
      use:menuKeys={{ close, trigger: btn }} onclick={picked}>
      {@render children(close)}
    </div>
  {/if}
</div>

<style>
  .menu-wrap { position: relative; }
  /* Styles de base de faible spécificité (:where) : une classe d'écran (.user-menu, .sn-panel) les remplace. */
  :global(:where(.pop-menu)) { position: absolute; right: 0; top: calc(100% + 6px); z-index: 60; min-width: 220px; max-width: calc(100vw - 32px); display: flex; flex-direction: column; gap: 2px; padding: 6px; border-radius: var(--r-md, 12px); background: var(--surface); box-shadow: var(--e1); font: 400 var(--fs-md, 13.5px)/1.35 var(--font); color: var(--fg); animation: pop .16s var(--ease); }
  :global(:where(.pop-menu.left)) { left: 0; right: auto; }
  :global(:where(.pop-menu) [role^="menuitem"]) { display: flex; align-items: center; gap: 10px; width: 100%; min-height: 34px; padding: 7px 10px; border: 0; border-radius: var(--r-sm, 8px); background: none; text-align: left; font: 400 var(--fs-md, 13.5px)/1.35 var(--font); color: var(--fg); text-decoration: none; cursor: pointer; }
  :global(:where(.pop-menu) [role^="menuitem"]:hover) { background: var(--surface-2); }
  :global(:where(.pop-menu) [role^="menuitem"]:focus-visible) { outline: 2px solid var(--accent); outline-offset: -2px; }
  :global(:where(.pop-menu) [role^="menuitem"][disabled]) { opacity: .45; cursor: not-allowed; background: none; }
  :global(:where(.pop-menu) [role^="menuitem"].danger) { color: var(--err); }
  :global(:where(.pop-menu) [role^="menuitem"] .ic) { flex: none; color: var(--fg-3); }
  :global(:where(.pop-menu) [role^="menuitem"].danger .ic) { color: var(--err); }
  :global(:where(.pop-menu) hr) { width: 100%; height: 1px; margin: 4px 0; border: 0; background: var(--line); }
  :global(:where(.pop-menu) .pm-sub) { display: block; font-size: var(--fs-sm, 12.5px); color: var(--fg-3); }
</style>
