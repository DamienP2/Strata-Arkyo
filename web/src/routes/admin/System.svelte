<script lang="ts">
  import { api } from '../../lib/api';
  import Unavailable from './Unavailable.svelte';
  let sys = $state<any>(null);
  let error = $state<string | null>(null);
  api.get<any>('/admin/system').then((s) => (sys = s)).catch((e) => (error = e.message));
</script>

{#if error}<Unavailable message={error} />
{:else if sys}
  <section class="card">
    <div class="card-head"><h2>Moteur</h2></div>
    <dl class="kv">
      <dt>Version</dt><dd class="mono">{sys.version}</dd>
      <dt>Schéma de la base</dt><dd class="mono">{sys.schema}</dd>
      <dt>Architecture</dt><dd class="mono">{sys.arch}</dd>
      <dt>Dossier de données</dt><dd class="mono">{sys.data_dir}</dd>
      <dt>Dossier d’installation</dt><dd class="mono">{sys.install_dir}</dd>
      <dt>Installation gérée (OTA)</dt><dd>{sys.managed ? 'Oui' : 'Non : lancé hors du paquet Debian'}</dd>
      <dt>Clés de publication de confiance</dt><dd class="mono">{sys.release_keys.length ? sys.release_keys.join(', ') : 'Aucune'}</dd>
      <dt>Source des mises à jour</dt><dd class="mono">{sys.update_url ? (sys.update_url.startsWith('github:') ? `GitHub · ${sys.update_url.slice(7)}` : sys.update_url) : 'Non configurée'}</dd>
    </dl>
  </section>
{:else}<span class="spin"></span>{/if}
