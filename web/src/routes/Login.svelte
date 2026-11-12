<script lang="ts">
  import Brand from '../components/Brand.svelte';
  import { api, ApiError } from '../lib/api';
  import { session } from '../lib/session.svelte';
  import type { Me } from '../lib/types';

  let login = $state('');
  let password = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    busy = true; error = null;
    try {
      session.me = await api.post<Me>('/auth/login', { login: login.trim(), password });
    } catch (x) {
      error = x instanceof ApiError ? x.message : 'Connexion impossible.';
    } finally { busy = false; }
  }
</script>

<div class="gate">
  <div class="gate-card">
    <span class="word"><Brand stacked /></span>
    <div class="card stack" style="padding:30px 28px">
      <h1>Connexion</h1>
      <form onsubmit={submit}>
        <div class="field"><label for="login">E-mail ou identifiant</label><input class="input" id="login" autocomplete="username" bind:value={login} required></div>
        <div class="field"><label for="pw">Mot de passe</label><input class="input" id="pw" type="password" autocomplete="current-password" bind:value={password} required></div>
        {#if error}<div class="alert err" role="alert">{error}</div>{/if}
        <button class="btn primary" disabled={busy}>{#if busy}<span class="spin"></span>{/if}Se connecter</button>
      </form>
    </div>
    <p class="gate-foot">Mot de passe oublié : demandez à un administrateur du site de le réinitialiser.</p>
  </div>
</div>
