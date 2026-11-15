<script lang="ts">
  // Chargement : .spin centré dans un conteneur role=status (padding 40), à afficher tant que les données valent null.
  let { label = 'Chargement', inline = false }: {
    /** Ce qui se charge (« Chargement des câbles »), lu par les lecteurs d'écran et affiché sous l'indicateur. */
    label?: string;
    /** Version en ligne, sans marge (dans une cellule ou une barre). */
    inline?: boolean;
  } = $props();
</script>

<div class="loading" class:inline role="status" aria-live="polite">
  <span class="spin" aria-hidden="true"></span>
  <span class="ld-l" class:sr-only={inline}>{label}</span>
</div>

<style>
  .loading { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; padding: 40px; color: var(--fg-3); text-align: center; }
  .loading.inline { display: inline-flex; flex-direction: row; padding: 0; gap: 8px; }
  .ld-l { font-size: var(--fs-sm, 12.5px); }
</style>
