<script lang="ts">
  // Espace de travail : barre latérale de sections (groupes, icônes, pastilles) et contenu.
  // La barre se replie en icônes ; sur mobile elle laisse place au sélecteur de SecNav.
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import SecNav, { type SecGroup } from './SecNav.svelte';

  let { groups, active, label, head, top, children, store = 'ws', context, mobileExtra }: {
    groups: SecGroup[]; active: string; label: string; head?: Snippet<[boolean]>; top?: Snippet; children: Snippet; store?: string;
    /** Mobile : nom affiché devant la section dans le sélecteur. */
    context?: string;
    /** Mobile : contenu en tête du menu déroulant des sections. */
    mobileExtra?: Snippet;
  } = $props();

  const key = () => `strata-ws-folded:${store}`;
  let folded = $state(false);
  $effect(() => { try { folded = localStorage.getItem(key()) === '1'; } catch { /* stockage indisponible */ } });
  function fold() { folded = !folded; try { localStorage.setItem(key(), folded ? '1' : '0'); } catch { /* stockage indisponible */ } }
</script>

<div class="ws" class:folded>
  <aside class="ws-side" aria-label={label}>
    {#if head}<div class="ws-head">{@render head(folded)}</div>{/if}
    <nav class="ws-nav">
      {#each groups as g}
        <div class="ws-group" class:lock={g.lock}>
          {#if g.label}<span class="ws-label" title={g.lock ? `${g.label} : une personne à la fois` : g.label}>{#if !folded}{g.label}{/if}{#if g.lock}<Icon name="lock" size="sm" />{/if}</span>{/if}
          {#each g.items as it}
            <a href={it.href} class="ws-item" class:on={it.key === active} aria-current={it.key === active ? 'page' : undefined} title={folded ? it.label : undefined}>
              <Icon name={it.icon ?? 'chevr'} size="sm" /><span class="ws-t">{it.label}</span>
              {#if it.badge}<span class="ws-badge">{it.badge}</span>{/if}
            </a>
          {/each}
        </div>
      {/each}
    </nav>
    <button class="ws-fold" onclick={fold} aria-label={folded ? 'Déplier la barre latérale' : 'Replier la barre latérale'} title={folded ? 'Déplier' : 'Replier'}><Icon name="collapse" size="sm" />{#if !folded}<span>Replier</span>{/if}</button>
  </aside>
  <div class="ws-main">
    <div class="ws-mob"><SecNav {groups} {active} {label} {context} extra={mobileExtra} /></div>
    {#if top}{@render top()}{/if}
    {@render children()}
  </div>
</div>
