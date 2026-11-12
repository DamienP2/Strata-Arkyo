<script lang="ts">
  import Workspace from '../../components/Workspace.svelte';
  import type { SecGroup } from '../../components/SecNav.svelte';
  import { route, navigate } from '../../lib/router.svelte';
  import { can } from '../../lib/session.svelte';
  import Updates from './Updates.svelte';
  import Backups from './Backups.svelte';
  import System from './System.svelte';
  import Members from './Members.svelte';
  import Roles from './Roles.svelte';
  import Journal from './Journal.svelte';

  const groups = $derived.by(() => {
    const g: SecGroup[] = [];
    if (can('parametres', 'admin')) g.push({ label: 'Site', items: [
      { key: 'mises-a-jour', label: 'Mises à jour', href: '/admin/mises-a-jour', icon: 'refresh' },
      { key: 'sauvegardes', label: 'Sauvegardes', href: '/admin/sauvegardes', icon: 'disk' },
      { key: 'systeme', label: 'Système', href: '/admin/systeme', icon: 'server' },
    ] });
    if (can('membres', 'admin')) g.push({ label: 'Équipe', items: [
      { key: 'membres', label: 'Membres', href: '/admin/membres', icon: 'users' },
      { key: 'roles', label: 'Rôles', href: '/admin/roles', icon: 'shield' },
      { key: 'journal', label: 'Journal', href: '/admin/journal', icon: 'terminal' },
    ] });
    return g;
  });
  const sub = $derived(route.path.split('/')[2] ?? '');
  const section = $derived(groups.flatMap((g) => g.items.map((i) => ({ ...i, group: g.label }))).find((i) => i.key === sub));
  const allowed = $derived(groups.flatMap((g) => g.items.map((i) => i.key)));
  $effect(() => { if (allowed.length && !allowed.includes(sub)) navigate('/admin/' + allowed[0], true); });
</script>

{#if !groups.length}
  <div class="page"><h1>Administration</h1><div class="alert info">Votre rôle ne donne accès à aucune section de l’administration.</div></div>
{:else}
  <Workspace {groups} active={sub} label="Sections de l’administration" store="admin">
    {#snippet head(folded: boolean)}
      {#if !folded}<div class="ws-proj"><b>Administration</b><span>Site et équipe</span></div>{/if}
    {/snippet}
    <header class="ws-top"><div class="ws-top-main"><span class="ws-crumb">{section?.group ?? 'Administration'}</span><h1>{section?.label ?? ''}</h1></div></header>
    <div class="page">
      {#if sub === 'mises-a-jour'}<Updates />
      {:else if sub === 'sauvegardes'}<Backups />
      {:else if sub === 'systeme'}<System />
      {:else if sub === 'membres'}<Members />
      {:else if sub === 'roles'}<Roles />
      {:else if sub === 'journal'}<Journal />{/if}
    </div>
  </Workspace>
{/if}
