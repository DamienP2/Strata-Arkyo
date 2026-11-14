<script lang="ts">
  // Changement de page avec des modifications non enregistrées : on demande.
  import Confirm from './Confirm.svelte';
  import { leave, leaveAnyway } from '../lib/router.svelte';
</script>

{#if leave.to}
  <Confirm title="Quitter sans enregistrer ?" confirmLabel="Quitter sans enregistrer" danger onconfirm={leaveAnyway} onclose={() => (leave.to = null)}>
    <p>Des modifications de cette page ne sont pas enregistrées. Restez pour les enregistrer, ou quittez : elles seront perdues.</p>
  </Confirm>
{/if}
