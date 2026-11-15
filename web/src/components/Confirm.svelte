<script lang="ts">
  // Confirmation à corps riche (préférer ask() pour un simple texte) : Annuler + verbe, bloquée pendant l'envoi.
  import type { Snippet } from 'svelte';
  import Dialog from './Dialog.svelte';
  let { title, eyebrow = '', confirmLabel, cancelLabel = 'Annuler', danger = false, onconfirm, onclose, children }:
    { title: string; eyebrow?: string; confirmLabel: string;
      /** Libellé du bouton de renoncement (« Annuler », « Continuer la saisie »…). */
      cancelLabel?: string; danger?: boolean; onconfirm: () => Promise<unknown> | unknown; onclose: () => void; children: Snippet } = $props();
</script>

<!-- Pendant l'envoi, Annuler, Échap et le fond sont bloqués ; en cas d'échec, l'erreur est affichée et la boîte reste ouverte. -->
<Dialog {title} {eyebrow} {onclose} onsubmit={() => onconfirm()} submitLabel={confirmLabel} {cancelLabel} {danger}>
  {@render children()}
</Dialog>
