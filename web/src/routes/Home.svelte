<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import { api } from '../lib/api';
  import { session, can } from '../lib/session.svelte';
  import { nf, relative } from '../lib/format';

  const me = $derived(session.me!);
  const first = $derived(me.account.name.split(/\s+/)[0]);
  const today = new Date().toLocaleDateString('fr-FR', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });

  const KINDS = [
    { kind: 'equipment', label: 'Équipements', sub: 'fiches de matériel' },
    { kind: 'cables', label: 'Câbles', sub: 'références de câbles' },
    { kind: 'cable-models', label: 'Modèles de câble', sub: 'coax, paires, fibres…' },
    { kind: 'adapters', label: 'Adaptateurs', sub: 'adaptateurs et éclateurs' },
  ];
  let counts = $state<Record<string, number>>({});
  let site = $state<{ version: string; available: string | null; lastBackup: string | null; backupError: string | null } | null>(null);

  $effect(() => {
    if (can('catalogue', 'lecture')) {
      for (const k of KINDS) api.get<{ total: number }>(`/catalog/items?kind=${k.kind}&limit=1`).then((r) => (counts[k.kind] = r.total)).catch(() => {});
    }
    if (can('parametres', 'admin')) {
      Promise.all([api.get<any>('/admin/update'), api.get<any>('/admin/backups')])
        .then(([u, b]) => (site = { version: u.current, available: u.status?.available?.version ?? null, lastBackup: b.last_success, backupError: b.last_error }))
        .catch(() => {});
    }
  });
</script>

<div class="page">
  <section class="home-hero">
    <header class="hero">
      <p class="eyebrow">{today}</p>
      <h1 class="display">Bonjour {first}.</h1>
      <p class="lead">
        {#if can('catalogue', 'lecture')}Le catalogue du site compte {nf(counts['equipment'] ?? 0)} fiche{(counts['equipment'] ?? 0) > 1 ? 's' : ''} d’équipement. Les projets arrivent avec la prochaine version du moteur.
        {:else}Les projets arrivent avec la prochaine version du moteur.{/if}
      </p>
      <div class="head-actions">
        {#if can('catalogue', 'lecture')}<a class="btn primary" href="/catalogue"><Icon name="box" />Ouvrir le catalogue</a>{/if}
        {#if can('catalogue', 'ecriture')}<a class="btn" href="/catalogue/import"><Icon name="upload" />Importer un tableur</a>{/if}
      </div>
    </header>
    {#if site}
      <a class="next" href="/admin/mises-a-jour">
        <span class="next-top"><span class="lbl-t">État du site</span>{#if site.available}<span class="tally" style="--c:#7FA0FF;background:rgba(127,160,255,.12);box-shadow:inset 0 0 0 1px rgba(127,160,255,.35)">{site.available} disponible</span>{/if}</span>
        <span class="next-name">Strata Arkyo {site.version}</span>
        <span class="next-sub">{site.available ? `La version ${site.available} est prête à installer.` : 'Le moteur est à jour.'}</span>
        <span class="next-meta" style="grid-template-columns:repeat(2,minmax(0,1fr))">
          <span><span class="lbl-t">Dernière sauvegarde</span><b>{relative(site.lastBackup)}</b></span>
          <span><span class="lbl-t">Sauvegardes</span><b>{site.backupError ? 'En échec' : 'Automatiques à 03:00'}</b></span>
        </span>
      </a>
    {/if}
  </section>

  {#if can('catalogue', 'lecture')}
    <section class="figs">
      {#each KINDS as k}
        <a class="fig" href="/catalogue/{k.kind}" style="text-decoration:none">
          <span class="lbl-t">{k.label}</span>
          <span><b>{nf(counts[k.kind] ?? 0)}</b><span class="fig-s">{k.sub}</span></span>
        </a>
      {/each}
    </section>
  {/if}
</div>
