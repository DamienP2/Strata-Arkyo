<script lang="ts" module>
  /** Groupe de raccourcis : titre, puis paires [touches, action]. */
  export type ShortcutGroup = [title: string, keys: [keys: string, action: string][]];
</script>

<script lang="ts">
  // Aide « ? » des raccourcis d'un écran (extraite du Synoptique) : boîte en lecture seule, groupes de [touches, action].
  import Dialog from './Dialog.svelte';
  let { title = 'Raccourcis clavier', eyebrow = 'Aide', groups, onclose }: {
    title?: string; eyebrow?: string;
    groups: ShortcutGroup[];
    onclose: () => void;
  } = $props();
</script>

<Dialog {title} {eyebrow} {onclose}>
  <div class="keys">
    {#each groups as [g, list]}
      <section><h3 class="eyebrow">{g}</h3>{#each list as [kk, l]}<div class="krow"><kbd>{kk}</kbd><span>{l}</span></div>{/each}</section>
    {/each}
  </div>
  {#snippet footer()}<span></span><button type="button" class="btn ghost" onclick={onclose}>Fermer</button>{/snippet}
</Dialog>

<style>
  .keys { display: grid; grid-template-columns: 1fr 1fr; gap: 18px; }
  .keys h3 { margin: 0 0 6px; }
  .krow { display: grid; grid-template-columns: 140px 1fr; gap: 8px; padding: 3px 0; font-size: var(--fs-md, 13.5px); align-items: baseline; }
  .krow kbd { justify-self: start; }
  @media (max-width: 860px) { .keys { grid-template-columns: 1fr; } }
</style>
