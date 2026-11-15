<script lang="ts" module>
  /** Étape du fil d'Ariane. `current` : page en cours (par défaut la dernière étape, affichée sans
   *  lien). `false` sur la dernière étape : elle n'est pas la page affichée (le titre l'est), elle
   *  garde son lien s'il y en a un (projet parent, groupe de sections « Conception »). */
  export interface Crumb { label: string; href?: string; current?: boolean }
</script>

<script lang="ts">
  // Gabarit unique d'en-tête de page : fil d'Ariane cliquable, titre (26 px) et compteur,
  // action principale à droite sur la ligne du titre ; description d'une ligne, méta, puis
  // recherche et filtres en dessous.
  import type { Snippet } from 'svelte';
  let { crumbs = [], title, count = '', desc = '', info, actions, children, ws = false, portal = false }: {
    crumbs?: Crumb[]; title: string; count?: string;
    /** Description d'une ligne sous le titre (le texte complet reste en infobulle) : l'en-tête garde la même hauteur partout. */
    desc?: string;
    /** Ligne d'information sous le titre (date de modification, métadonnées du projet). Libellés en Inter, jamais en mono. */
    info?: Snippet;
    actions?: Snippet | null; children?: Snippet;
    /** Dans un espace de travail : barre collée en haut du contenu. */
    ws?: boolean;
    /** Zone d'actions remplie par la sous-page affichée (components/HeadActions.svelte). */
    portal?: boolean;
  } = $props();
</script>

<header class="ph" class:ws-top={ws} class:ph-ws-info={ws && !!info}>
  {#if crumbs.length}
    <nav class="ph-crumbs" aria-label="Fil d’Ariane">
      <ol>{#each crumbs as c, i}<li>{#if c.href && (i < crumbs.length - 1 || c.current === false)}<a href={c.href}>{c.label}</a>{:else}<span aria-current={(c.current ?? i === crumbs.length - 1) ? 'page' : undefined}>{c.label}</span>{/if}</li>{/each}</ol>
    </nav>
  {/if}
  <div class="ph-row">
    <div class="ph-title"><h1>{title}</h1>{#if count}<span class="ph-count">{count}</span>{/if}</div>
    {#if actions}<div class="ph-actions">{@render actions()}</div>{:else if portal}<div class="ph-actions ph-slot"></div>{/if}
  </div>
  {#if desc}<p class="ph-desc" title={desc}>{desc}</p>{/if}
  {#if info}<div class="ph-meta">{@render info()}</div>{/if}
  {#if children}<div class="ph-tools">{@render children()}</div>{/if}
</header>

<style>
  /* Espace de travail avec méta (pages projet) : la méta se range à droite du fil d'Ariane, pour
     garder une barre aussi basse que celle du catalogue (fil, puis titre et actions). */
  @media (min-width: 861px) {
    .ph.ph-ws-info { display: grid; grid-template-columns: auto minmax(0, 1fr); align-items: center; gap: 3px 24px; }
    .ph-ws-info > * { grid-column: 1 / -1; }
    .ph-ws-info > .ph-crumbs { grid-column: 1; grid-row: 1; }
    .ph-ws-info > .ph-meta { grid-column: 2; grid-row: 1; display: flex; justify-content: flex-end; min-width: 0; }
  }
  .ph-desc { margin: 0; max-width: 100%; font-size: var(--fs-md); line-height: 1.4; color: var(--fg-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  @media (max-width: 860px) {
    .ws-top .ph-desc { display: none; }
  }
</style>
