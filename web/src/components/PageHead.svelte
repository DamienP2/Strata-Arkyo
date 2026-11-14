<script lang="ts" module>
  export interface Crumb { label: string; href?: string }
</script>

<script lang="ts">
  // Gabarit unique d'en-tête de page : fil d'Ariane cliquable, titre (24–28 px) et compteur,
  // action principale à droite sur la ligne du titre ; recherche et filtres en dessous.
  import type { Snippet } from 'svelte';
  let { crumbs = [], title, count = '', info, actions, children, ws = false, portal = false }: {
    crumbs?: Crumb[]; title: string; count?: string;
    /** Ligne d'information sous le titre (date de modification…). */
    info?: Snippet;
    actions?: Snippet | null; children?: Snippet;
    /** Dans un espace de travail : barre collée en haut du contenu. */
    ws?: boolean;
    /** Zone d'actions remplie par la sous-page affichée (components/HeadActions.svelte). */
    portal?: boolean;
  } = $props();
</script>

<header class="ph" class:ws-top={ws}>
  {#if crumbs.length}
    <nav class="ph-crumbs" aria-label="Fil d’Ariane">
      <ol>{#each crumbs as c, i}<li>{#if c.href && i < crumbs.length - 1}<a href={c.href}>{c.label}</a>{:else}<span aria-current={i === crumbs.length - 1 ? 'page' : undefined}>{c.label}</span>{/if}</li>{/each}</ol>
    </nav>
  {/if}
  <div class="ph-row">
    <div class="ph-title"><h1>{title}</h1>{#if count}<span class="ph-count">{count}</span>{/if}</div>
    {#if actions}<div class="ph-actions">{@render actions()}</div>{:else if portal}<div class="ph-actions ph-slot"></div>{/if}
  </div>
  {#if info}<div class="ph-meta">{@render info()}</div>{/if}
  {#if children}<div class="ph-tools">{@render children()}</div>{/if}
</header>
