<script lang="ts">
  // Reconnexion par-dessus l'écran courant quand la session expire : rien n'est perdu.
  import { api, ApiError } from '../lib/api';
  import { session } from '../lib/session.svelte';
  import type { Me } from '../lib/types';
  let login = $state(session.me?.email ?? ''), password = $state(''), busy = $state(false), error = $state('');
  async function submit(e: SubmitEvent) {
    e.preventDefault(); busy = true; error = '';
    try {
      const me = await api.post<Me>('/auth/login', { login: login.trim(), password });
      if (session.me && me.id !== session.me.id) { location.reload(); return; }
      session.me = me; session.expired = false;
    } catch (x) { error = x instanceof ApiError ? x.message : 'Connexion impossible.'; }
    finally { busy = false; }
  }
</script>

<div class="modal" role="presentation">
  <form class="dialog relog" role="dialog" aria-modal="true" aria-label="Session expirée" onsubmit={submit}>
    <div class="wz-body">
      <h2>Session expirée</h2>
      <p class="lead">Reconnectez-vous pour continuer : l’écran et vos saisies en cours sont conservés.</p>
      <div class="field"><label for="rl-l">Identifiant</label><input class="input" id="rl-l" bind:value={login} autocomplete="username" required></div>
      <div class="field"><label for="rl-p">Mot de passe</label><!-- svelte-ignore a11y_autofocus --><input class="input" id="rl-p" type="password" bind:value={password} autocomplete="current-password" required autofocus></div>
      {#if error}<p class="err-msg" role="alert">{error}</p>{/if}
    </div>
    <div class="wz-foot"><button class="btn primary" disabled={busy}>{#if busy}<span class="spin"></span>{/if}Se reconnecter</button></div>
  </form>
</div>

<style>
  .relog { width: min(440px, 100%); }
  .err-msg { color: var(--err); margin: 0; }
</style>
