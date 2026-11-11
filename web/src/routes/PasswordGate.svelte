<script lang="ts">
  import Logo from '../components/Logo.svelte';
  import PasswordForm from '../components/PasswordForm.svelte';
  import { toast } from '../lib/toast.svelte';
  import { session } from '../lib/session.svelte';
</script>

<div class="gate">
  <div class="gate-card">
    <span class="word"><Logo /><span>Strata Arkyo</span></span>
    <div class="card stack" style="padding:30px 28px">
      <p class="eyebrow" style="margin:0">Bienvenue, {session.me?.account.name}</p>
      <h1>Choisissez votre mot de passe</h1>
      <p class="muted">Vous vous êtes connecté avec un mot de passe temporaire. Remplacez-le pour continuer.</p>
      <PasswordForm currentLabel="Mot de passe temporaire" submitLabel="Continuer" ondone={() => toast('Mot de passe enregistré. Bienvenue !')} />
    </div>
  </div>
</div>
