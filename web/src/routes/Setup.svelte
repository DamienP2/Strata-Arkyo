<script lang="ts">
  import Brand from '../components/Brand.svelte';
  import FieldError from '../components/FieldError.svelte';
  import { api, ApiError } from '../lib/api';
  import { session } from '../lib/session.svelte';
  import type { Me } from '../lib/types';

  let email = $state(''), name = $state(''), username = $state(''), password = $state(''), confirm = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);
  const mismatch = $derived(confirm.length > 0 && confirm !== password);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (mismatch) return;
    busy = true; error = null;
    try {
      session.me = await api.post<Me>('/setup', { email: email.trim(), name: name.trim() || null, username: username.trim() || null, password });
      session.setupNeeded = false;
    } catch (x) {
      error = x instanceof ApiError ? x.message : 'Création impossible.';
    } finally { busy = false; }
  }
</script>

<div class="gate">
  <div class="gate-card">
    <span class="word"><Brand stacked /></span>
    <div class="card stack" style="padding:30px 28px">
      <p class="eyebrow" style="margin:0">Première mise en route</p>
      <h1>Créez le compte propriétaire</h1>
      <p class="muted">Ce compte a tous les droits sur le site. Pour des raisons de sécurité, il se crée depuis la machine serveur elle-même (ou avec la commande <code>strata user add</code>).</p>
      <form onsubmit={submit}>
        <div class="field"><label for="em">E-mail</label><input class="input" id="em" type="email" autocomplete="email" bind:value={email} required></div>
        <div class="field"><label for="nm">Nom affiché</label><input class="input" id="nm" autocomplete="name" bind:value={name} placeholder="Facultatif"></div>
        <div class="field"><label for="un">Identifiant</label><input class="input" id="un" autocomplete="username" bind:value={username} placeholder="Facultatif, pour se connecter sans l'e-mail"></div>
        <div class="field"><label for="pw">Mot de passe</label><input class="input" id="pw" type="password" autocomplete="new-password" bind:value={password} required minlength="8"><span class="help">8 caractères au moins.</span></div>
        <div class="field"><label for="pw2">Confirmation</label><input class="input" id="pw2" type="password" autocomplete="new-password" bind:value={confirm} required><FieldError message={mismatch ? 'Les deux mots de passe diffèrent.' : null} /></div>
        {#if error}<div class="alert err" role="alert">{error}</div>{/if}
        <button class="btn primary" disabled={busy || mismatch}>{#if busy}<span class="spin"></span>{/if}Créer le compte</button>
      </form>
    </div>
  </div>
</div>
