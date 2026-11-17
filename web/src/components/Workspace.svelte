<script lang="ts">
  // Espace de travail du design commun Strata : liste à gauche (240 px, sections et pages), page à
  // droite. `full` : éditeur en panneaux plein cadre (pas de marge, pas de défilement de page) ;
  // sinon page de contenu dans .library-inner (1400 px au plus, 28 px 24 px).
  // La liste se replie en icônes ; sur mobile elle laisse place au sélecteur de SecNav.
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import SecNav, { type SecGroup } from './SecNav.svelte';

  let { groups, active, label, head, top, children, store = 'ws', context, mobileExtra, full = false }: {
    groups: SecGroup[]; active: string; label: string; head?: Snippet<[boolean]>; top?: Snippet; children: Snippet; store?: string;
    /** Mobile : nom affiché devant la section dans le sélecteur. */
    context?: string;
    /** Mobile : contenu en tête du menu déroulant des sections. */
    mobileExtra?: Snippet;
    /** Éditeur plein cadre : la page occupe toute la place, sans marges ni défilement de page. */
    full?: boolean;
  } = $props();

  const key = () => `strata-ws-folded:${store}`;
  let folded = $state(false);
  $effect(() => { try { folded = localStorage.getItem(key()) === '1'; } catch { /* stockage indisponible */ } });
  function fold() { folded = !folded; try { localStorage.setItem(key(), folded ? '1' : '0'); } catch { /* stockage indisponible */ } }
</script>

<div class="ws projects-page" class:folded>
  <aside class="lib-nav ws-side" aria-label={label}>
    {#if head}<div class="ws-head">{@render head(folded)}</div>{/if}
    <nav class="ws-nav">
      {#each groups as g}
        <div class="sys-group" class:lock={g.lock}>
          {#if g.label}<div class="lib-nav-section" title={g.lock ? `${g.label} : une personne à la fois` : g.label}><span class="grow">{#if !folded}{g.label}{/if}</span>{#if g.lock}<Icon name="lock" size="sm" />{/if}</div>{/if}
          {#each g.items as it}
            <a href={it.href} class="lib-nav-item" class:on={it.key === active} aria-current={it.key === active ? 'page' : undefined} title={folded ? it.label : undefined}>
              <Icon name={it.icon ?? 'chevr'} size="sm" /><span class="grow">{it.label}</span>
              {#if it.badge}<span class="count">{it.badge}</span>{/if}
            </a>
          {/each}
        </div>
      {/each}
    </nav>
    <button class="lib-nav-item ws-fold" onclick={fold} aria-label={folded ? 'Déplier la liste' : 'Replier la liste'} title={folded ? 'Déplier' : 'Replier'}><Icon name="collapse" size="sm" />{#if !folded}<span class="grow">Replier</span>{/if}</button>
  </aside>
  {#if full}
    <div class="ws-main full">
      <div class="ws-mob"><SecNav {groups} {active} {label} {context} extra={mobileExtra} /></div>
      {#if top}{@render top()}{/if}
      {@render children()}
    </div>
  {:else}
    <div class="ws-main library">
      <div class="library-inner wide">
        <div class="ws-mob"><SecNav {groups} {active} {label} {context} extra={mobileExtra} /></div>
        {#if top}{@render top()}{/if}
        {@render children()}
      </div>
    </div>
  {/if}
</div>
