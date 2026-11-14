<script lang="ts">
  // Action principale d'une sous-page, affichée dans l'en-tête de l'espace de travail.
  import type { Snippet } from 'svelte';
  import { toHead } from '../lib/pagehead.svelte';
  let { children }: { children: Snippet } = $props();
</script>

<div class="ph-port"><div class="ph-portal" use:toHead>{@render children()}</div></div>
