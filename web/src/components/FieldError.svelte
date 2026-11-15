<script lang="ts">
  // Message d'erreur d'un champ : `id` (ou `for` → « {for}-err ») à relier par aria-describedby, avec aria-invalid sur le champ.
  let { message, id, for: forId }: {
    message?: string | null;
    /** Identifiant du message (prioritaire sur `for`). */
    id?: string;
    /** Identifiant du champ : le message prend l'id « {for}-err ». */
    for?: string;
  } = $props();
  const eid = $derived(id ?? (forId ? `${forId}-err` : undefined));
</script>

{#if message}<span class="help err" id={eid} role="alert">{message}</span>{/if}
