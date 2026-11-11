<script lang="ts">
  import SecNav, { type SecGroup } from '../../components/SecNav.svelte';
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
      { key: 'mises-a-jour', label: 'Mises à jour', href: '/admin/mises-a-jour' },
      { key: 'sauvegardes', label: 'Sauvegardes', href: '/admin/sauvegardes' },
      { key: 'systeme', label: 'Système', href: '/admin/systeme' },
    ] });
    if (can('membres', 'admin')) g.push({ label: 'Équipe', items: [
      { key: 'membres', label: 'Membres', href: '/admin/membres' },
      { key: 'roles', label: 'Rôles', href: '/admin/roles' },
      { key: 'journal', label: 'Journal', href: '/admin/journal' },
    ] });
    return g;
  });
  const sub = $derived(route.path.split('/')[2] ?? '');
  const allowed = $derived(groups.flatMap((g) => g.items.map((i) => i.key)));
  $effect(() => { if (allowed.length && !allowed.includes(sub)) navigate('/admin/' + allowed[0], true); });
</script>

<div class="page">
  <div class="page-head"><div><p class="eyebrow">Site</p><h1 class="display">Administration</h1></div></div>
  {#if !groups.length}
    <div class="alert info">Votre rôle ne donne accès à aucune section de l’administration.</div>
  {:else}
    <SecNav {groups} active={sub} label="Sections de l’administration" />
    {#if sub === 'mises-a-jour'}<Updates />
    {:else if sub === 'sauvegardes'}<Backups />
    {:else if sub === 'systeme'}<System />
    {:else if sub === 'membres'}<Members />
    {:else if sub === 'roles'}<Roles />
    {:else if sub === 'journal'}<Journal />{/if}
  {/if}
</div>
