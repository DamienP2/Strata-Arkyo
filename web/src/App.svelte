<script lang="ts">
  import { session, loadSession } from './lib/session.svelte';
  import { route, navigate } from './lib/router.svelte';
  import Toasts from './components/Toasts.svelte';
  import Shell from './components/Shell.svelte';
  import Setup from './routes/Setup.svelte';
  import Login from './routes/Login.svelte';
  import PasswordGate from './routes/PasswordGate.svelte';
  import Home from './routes/Home.svelte';
  import Admin from './routes/admin/Admin.svelte';
  import Catalogue from './routes/catalogue/Catalogue.svelte';
  import NotFound from './routes/NotFound.svelte';

  loadSession();

  const top = $derived(route.path.split('/')[1] ?? '');
  // Après la connexion, on revient à l'adresse demandée.
  $effect(() => {
    if (session.me && (route.path === '/connexion' || route.path === '/premier-compte')) navigate('/', true);
  });
</script>

{#if !session.ready}
  <div class="gate"><span class="spin" aria-label="Chargement"></span></div>
{:else if session.error && !session.me}
  <div class="gate"><div class="gate-card card"><h1>Serveur injoignable</h1><p class="lead">{session.error}</p><button class="btn primary" onclick={() => location.reload()}>Réessayer</button></div></div>
{:else if !session.me}
  {#if session.setupNeeded}<Setup />{:else}<Login />{/if}
{:else if session.me.must_change_password}
  <PasswordGate />
{:else}
  <Shell>
    {#if top === ''}<Home />
    {:else if top === 'catalogue'}<Catalogue />
    {:else if top === 'admin'}<Admin />
    {:else}<NotFound />{/if}
  </Shell>
{/if}
<Toasts />
