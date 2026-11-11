<script lang="ts">
  import Dialog from './Dialog.svelte';
  import PasswordForm from './PasswordForm.svelte';
  import { toast } from '../lib/toast.svelte';
  let { onclose }: { onclose: () => void } = $props();
</script>

<Dialog title="Changer mon mot de passe" eyebrow="Mon compte" {onclose}>
  <PasswordForm ondone={() => { toast('Mot de passe changé.'); onclose(); }} />
</Dialog>
