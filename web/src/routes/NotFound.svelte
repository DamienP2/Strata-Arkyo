<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import PageHead from '../components/PageHead.svelte';
  import EmptyState from '../components/EmptyState.svelte';
</script>
<div class="page">
  <PageHead title="Page introuvable" count="Erreur 404" />
  <EmptyState icon="map" message="Cette adresse ne correspond à aucun écran.">{#snippet action()}<a class="btn primary" href="/"><Icon name="home" />Revenir à l’accueil</a>{/snippet}</EmptyState>
</div>
