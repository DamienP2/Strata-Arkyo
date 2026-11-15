<script lang="ts">
  // Changement de page avec des modifications non enregistrées (registerLeaveGuard) : on demande.
  import Confirm from './Confirm.svelte';
  import { leave, leaveAnyway } from '../lib/router.svelte';
</script>

{#if leave.to}
  <Confirm title="Quitter sans enregistrer ?" confirmLabel="Quitter sans enregistrer" cancelLabel="Continuer la saisie" danger onconfirm={leaveAnyway} onclose={() => (leave.to = null)}>
    <p class="lead">Ce qui n’a pas été enregistré sera perdu.</p>
  </Confirm>
{/if}
