<script lang="ts">
  // Barre de sections : touches groupées sur grand écran, menu déroulant (Menu.svelte) sur mobile.
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import Menu from './Menu.svelte';
  export interface SecItem { key: string; label: string; href: string; badge?: string | number; icon?: string }
  export interface SecGroup { label?: string; lock?: boolean; items: SecItem[] }
  let { groups, active, label, context, extra }: {
    groups: SecGroup[]; active: string; label: string;
    /** Sur mobile, nom affiché devant la section (« REGIE 3 · Vue d’ensemble »). */
    context?: string;
    /** Sur mobile, contenu ajouté en tête du menu déroulant (statut du projet…). */
    extra?: Snippet;
  } = $props();

  const all = $derived(groups.flatMap((g) => g.items));
  const cur = $derived(all.find((i) => i.key === active) ?? all[0]);
  const curGroup = $derived(groups.find((g) => g.items.includes(cur!)));
  const flat = $derived(groups.length === 1 && !groups[0]?.label);
</script>

<nav class="subnav" class:flat aria-label={label}>
  <div class="sn-row">
    {#each groups as g}
      <div class="sn-group" class:lock={g.lock}>
        {#if g.label}<span class="sn-label" class:lock={g.lock}>{g.label}{#if g.lock}<Icon name="lock" />{/if}</span>{/if}
        <div class="sn-items">
          {#each g.items as it}
            <a href={it.href} class="snb" class:on={it.key === active} aria-current={it.key === active ? 'page' : undefined}>{it.label}{#if it.badge}<span class="badge">{it.badge}</span>{/if}</a>
          {/each}
        </div>
      </div>
    {/each}
  </div>
  <Menu {label} class="sn-pick" btnClass="sn-sum" menuClass="sn-panel">
    {#snippet trigger()}
      {#if context}
        <span class="sn-cur one"><b><span class="sn-ctx">{context}</span><span class="sn-sep">·</span><span class="sn-sec">{cur?.label}</span></b></span>
      {:else}
        <span class="sn-cur">{#if curGroup?.label}<span class="sn-label" class:lock={curGroup.lock}>{curGroup.label}</span>{/if}<b>{cur?.label}</b></span>
      {/if}
      <span class="chev"><Icon name="chevd" /></span>
    {/snippet}
    {#if extra}<div class="sn-extra">{@render extra()}</div>{/if}
    {#each groups as g}
      {#if g.label}<span class="sn-label" class:lock={g.lock}>{g.label}</span>{/if}
      {#each g.items as it}
        <a href={it.href} role="menuitem" class="snb" class:on={it.key === active} aria-current={it.key === active ? 'page' : undefined}>{it.label}{#if it.badge}<span class="badge">{it.badge}</span>{/if}</a>
      {/each}
    {/each}
  </Menu>
</nav>

<style>
  /* Les touches sont des liens. État actif = celui de la barre latérale (.ws-item.on) : carte
     claire, texte --fg 650, contour --line et repère d'accent ; aucun pavé sombre ni LED. */
  .snb { position: relative; display: inline-flex; align-items: center; gap: 8px; height: 38px; padding: 0 14px; border-radius: var(--r-sm, 8px); font-size: var(--fs-md, 13.5px); font-weight: 550; color: var(--fg-2); white-space: nowrap; text-decoration: none; transition: background .12s, color .12s; }
  .snb:hover { background: var(--surface-3); color: var(--fg); }
  .snb.on { background: var(--surface); color: var(--fg); font-weight: 650; box-shadow: 0 0 0 1px var(--line), 0 1px 2px rgba(0, 0, 0, .05); }
  .snb.on::before { content: ""; position: absolute; left: 0; top: 7px; bottom: 7px; width: 3px; height: auto; border-radius: 0 3px 3px 0; background: var(--accent); box-shadow: none; }
  .snb:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .snb .badge { margin-left: auto; }
  /* Menu mobile : entrées de 44px (cible tactile). */
  :global(.sn-panel) .snb { display: flex; width: 100%; height: 44px; padding: 0 12px; font-size: var(--fs-md, 13.5px); border-radius: var(--r-sm, 8px); }
  /* Bouton du menu mobile : reprend l'ancien résumé du <details> (.sn-pick summary). */
  .subnav :global(.sn-sum) { display: flex; align-items: center; gap: 10px; width: 100%; height: auto; min-height: 40px; padding: 4px 10px 4px 12px; border: 1px solid var(--line); border-radius: 8px; background: var(--panel-2); box-shadow: none; color: var(--fg); font: inherit; text-align: left; cursor: pointer; }
  .subnav :global(.sn-sum:hover) { background: var(--surface); }
  .subnav :global(.sn-sum:focus-visible) { outline: 2px solid var(--accent); outline-offset: 2px; }
  .subnav :global(.sn-sum .sn-label) { padding: 0; }
  .sn-cur { display: flex; flex-direction: column; gap: 5px; min-width: 0; }
  .sn-cur b { font-weight: 700; font-size: 14px; line-height: 1.2; }
  .chev { display: inline-grid; margin-left: auto; transition: transform .2s var(--ease); }
  .subnav :global(.sn-sum[aria-expanded="true"] .chev) { transform: rotate(180deg); }
  .subnav :global(.sn-sum:has(.sn-cur.one)) { min-height: 40px; padding-block: 4px; }
  .sn-cur.one { flex: 1; }
  .sn-cur.one b { display: flex; align-items: baseline; gap: 6px; min-width: 0; font-size: 14px; }
  .sn-ctx { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--fg-2); font-weight: 600; }
  .sn-sep { flex: none; color: var(--fg-3); }
  .sn-sec { flex: none; max-width: 70%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sn-extra { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 6px 12px 10px; margin-bottom: 4px; border-bottom: 1px solid var(--line); }
  .sn-extra > :global(span) { white-space: nowrap; }
</style>
