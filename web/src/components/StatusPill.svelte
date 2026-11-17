<script lang="ts">
  // Pastille de connexion du design commun Strata : « Connecté » (vert), « Connexion… » (gris,
  // point qui clignote), « Serveur perdu · reconnexion… » (rouge) quand le temps réel est coupé
  // depuis 800 ms (4 s au premier chargement).
  import Icon from './Icon.svelte';
  import { live } from '../lib/live.svelte';
  import { session } from '../lib/session.svelte';

  let lost = $state(false);
  let first = true;
  let seen = false;
  $effect(() => {
    const down = (live.wanted && !live.connected) || !!session.error;
    if (live.connected) seen = true;
    if (!down) { lost = false; first = false; return; }
    const t = setTimeout(() => { lost = true; first = false; }, first && !seen ? 4000 : 800);
    return () => clearTimeout(t);
  });
  const st = $derived(lost ? 'lost' : live.wanted && !live.connected ? 'wait' : 'ok');
  const tip = $derived(st === 'lost' ? 'Serveur perdu : reconnexion en cours' : st === 'wait' ? 'Connexion au serveur' : `Strata Arkyo · ${location.host}\nConnecté`);
</script>

<span class="ux1-pill {st === 'ok' ? '' : st}" title={tip} role="status">
  {#if st === 'lost'}
    <Icon name="wifioff" size="sm" /><span class="ux1-lost-txt"><span>Serveur perdu</span><span class="ux1-lost-sep">&nbsp;·&nbsp;</span><span>reconnexion…</span></span>
  {:else}
    <span class="ux1-dot"></span>{st === 'wait' ? 'Connexion…' : 'Connecté'}
  {/if}
</span>
