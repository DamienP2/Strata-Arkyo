<script lang="ts">
  // Barre de sections : touches groupées sur grand écran, sélecteur déroulant sur mobile.
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
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
  const curGroup = $derived(groups.find((g) => g.items.includes(cur)));
  const flat = $derived(groups.length === 1 && !groups[0]?.label);
  let pick: HTMLDetailsElement;
</script>

<nav class="subnav" class:flat aria-label={label}>
  <div class="sn-row">
    {#each groups as g}
      <div class="sn-group" class:lock={g.lock}>
        {#if g.label}<span class="sn-label" class:lock={g.lock}>{g.label}{#if g.lock}<Icon name="lock" />{/if}</span>{/if}
        <div class="sn-items">
          {#each g.items as it}
            <a href={it.href} class="snb" class:on={it.key === active} aria-current={it.key === active ? 'page' : undefined}>{it.label}{#if it.badge}<span class="badge-n">{it.badge}</span>{/if}</a>
          {/each}
        </div>
      </div>
    {/each}
  </div>
  <details class="sn-pick" bind:this={pick}>
    <summary>
      {#if context}
        <span class="sn-cur one"><b><span class="sn-ctx">{context}</span><span class="sn-sep">·</span><span class="sn-sec">{cur?.label}</span></b></span>
      {:else}
        <span class="sn-cur">{#if curGroup?.label}<span class="sn-label" class:lock={curGroup.lock}>{curGroup.label}</span>{/if}<b>{cur?.label}</b></span>
      {/if}
      <span class="chev"><Icon name="chevd" /></span>
    </summary>
    <div class="sn-panel">
      {#if extra}<div class="sn-extra">{@render extra()}</div>{/if}
      {#each groups as g}
        {#if g.label}<span class="sn-label" class:lock={g.lock}>{g.label}</span>{/if}
        {#each g.items as it}
          <a href={it.href} class="snb" class:on={it.key === active} onclick={() => (pick.open = false)}>{it.label}{#if it.badge}<span class="badge-n">{it.badge}</span>{/if}</a>
        {/each}
      {/each}
    </div>
  </details>
</nav>

<style>
  /* Les touches sont des liens : on reprend l'aspect des boutons de la maquette. */
  .snb { display: inline-flex; align-items: center; gap: 8px; height: 38px; padding: 0 14px; border-radius: 10px; font-size: 15px; font-weight: 550; color: var(--fg-2); white-space: nowrap; text-decoration: none; transition: background .15s, color .15s; }
  .snb:hover { background: var(--surface-2); color: var(--fg); }
  .snb.on { background: var(--primary-bg); color: var(--primary-fg); box-shadow: var(--key-ink); font-weight: 650; }
  .snb.on::before { content: ""; width: 6px; height: 6px; border-radius: 50%; background: var(--key-led); box-shadow: 0 0 7px var(--key-led); }
  .sn-panel .snb { display: flex; width: 100%; height: 44px; padding: 0 12px; }
  .sn-pick summary:has(.sn-cur.one) { min-height: 48px; padding-block: 4px; }
  .sn-cur.one { flex: 1; }
  .sn-cur.one b { display: flex; align-items: baseline; gap: 6px; min-width: 0; font-size: 16px; }
  .sn-ctx { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--fg-2); font-weight: 600; }
  .sn-sep { flex: none; color: var(--fg-4); }
  .sn-sec { flex: none; max-width: 70%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sn-extra { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 6px 12px 10px; margin-bottom: 4px; border-bottom: 1px solid var(--line); }
  .chev { margin-left: auto; }
  .sn-extra > :global(span) { white-space: nowrap; }
  .badge-n { display: inline-grid; place-items: center; min-width: 18px; height: 18px; padding: 0 5px; border-radius: 9px; font: 700 10.5px var(--mono); background: var(--accent); color: #fff; }
</style>
