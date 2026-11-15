<script lang="ts">
  // Reconnexion par-dessus l'écran courant quand la session expire : rien n'est perdu.
  import { api, ApiError } from '../lib/api';
  import { session } from '../lib/session.svelte';
  import { modal } from '../lib/modal';
  import type { Me } from '../lib/types';
  let login = $state(session.me?.account.email ?? ''), password = $state(''), busy = $state(false), error = $state('');
  async function submit(e: SubmitEvent) {
    e.preventDefault(); busy = true; error = '';
    try {
      const me = await api.post<Me>('/auth/login', { login: login.trim(), password });
      if (session.me && me.account.id !== session.me.account.id) { location.reload(); return; }
      session.me = me; session.expired = false;
    } catch (x) { error = x instanceof ApiError ? x.message : 'Connexion impossible.'; }
    finally { busy = false; }
  }
</script>

<!-- Dans la pile des boîtes (Échap et Tab restent ici), mais impossible à fermer sans se reconnecter. -->
<div class="modal relog-bg" role="presentation" use:modal={{ close: () => {}, locked: () => true }}>
  <div class="dialog relog" role="dialog" aria-modal="true" aria-label="Session expirée" tabindex="-1"><form onsubmit={submit}>
    <div class="wz-body">
      <h2>Session expirée</h2>
      <p class="lead">Reconnectez-vous pour continuer : l’écran et vos saisies en cours sont conservés.</p>
      <div class="field"><label for="rl-l">Identifiant</label><input class="input" id="rl-l" bind:value={login} autocomplete="username" required></div>
      <div class="field"><label for="rl-p">Mot de passe</label><input class="input" id="rl-p" type="password" bind:value={password} autocomplete="current-password" required data-autofocus aria-invalid={error ? 'true' : undefined} aria-describedby={error ? 'rl-err' : undefined}></div>
      {#if error}<p class="err-msg" id="rl-err" role="alert">{error}</p>{/if}
    </div>
    <div class="wz-foot"><span></span><button type="submit" class="btn primary" disabled={busy}>{#if busy}<span class="spin" aria-hidden="true"></span>{/if}Se reconnecter</button></div>
  </form></div>
</div>

<style>
  /* Au-dessus des boîtes de l'écran, ouvertes après elle dans le document. */
  .relog-bg { z-index: 80; }
  .relog { width: min(440px, 100%); }
  .relog h2 { font: 700 var(--fs-xl, 20px)/1.2 var(--display); letter-spacing: -.015em; }
  .relog .wz-body { min-height: 0; }
  .err-msg { color: var(--err); margin: 0; }
</style>
