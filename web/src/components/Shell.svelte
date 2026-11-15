<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import Brand from './Brand.svelte';
  import Avatar from './Avatar.svelte';
  import PasswordDialog from './PasswordDialog.svelte';
  import CommandPalette from './CommandPalette.svelte';
  import Menu from './Menu.svelte';
  import { modalOpen } from '../lib/modal';
  import { session, can } from '../lib/session.svelte';
  import { route, navigate } from '../lib/router.svelte';
  import { isDark, toggleTheme, theme } from '../lib/theme.svelte';
  import { api } from '../lib/api';
  import { fail } from '../lib/toast.svelte';

  let { children }: { children: Snippet } = $props();
  let menuOpen = $state(false);
  let userOpen = $state(false);
  let pwOpen = $state(false);
  let paletteOpen = $state(false);

  const top = $derived(route.path.split('/')[1] ?? '');
  // Projet ouvert, catalogue et administration : barre latérale de sections, contenu pleine hauteur.
  const wsView = $derived(top === 'catalogue' || top === 'admin' || (top === 'projets' && !!route.path.split('/')[2]));
  const links = $derived([
    { key: '', label: 'Accueil', href: '/', show: true },
    { key: 'projets', label: 'Projets', href: '/projets', show: can('projets', 'lecture') },
    { key: 'catalogue', label: 'Catalogue', href: '/catalogue', show: can('catalogue', 'lecture') },
    { key: 'admin', label: 'Administration', href: '/admin', show: can('membres', 'admin') || can('parametres', 'admin') },
  ].filter((l) => l.show));
  const me = $derived(session.me!);
  const dark = $derived((theme.value, isDark()));

  $effect(() => { route.path; menuOpen = false; userOpen = false; });

  async function logout() {
    try { await api.post('/auth/logout'); } catch (e) { fail(e); }
    session.me = null;
    navigate('/', true);
  }
  // Échap ne ferme que le niveau le plus haut : palette et boîtes (modal.ts), menu du compte
  // (Menu.svelte), puis le menu latéral mobile. Ctrl+K est ignoré quand une boîte est ouverte :
  // la palette ne navigue pas sous une saisie en cours.
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape' && menuOpen && !modalOpen()) { e.stopPropagation(); menuOpen = false; return; }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
      if (!paletteOpen && modalOpen()) return;
      e.preventDefault(); paletteOpen = !paletteOpen;
    }
  }
</script>

<svelte:window onkeydown={key} />
<CommandPalette bind:open={paletteOpen} />

<div class="app">
  <header class="mast">
    <button class="icon-btn menu-btn" aria-label="Ouvrir le menu" title="Ouvrir le menu" aria-expanded={menuOpen} onclick={() => (menuOpen = true)}><Icon name="menu" /></button>
    <a class="word" href="/" aria-label="Strata Arkyo, accueil"><Brand /></a>
    <nav class="topnav" aria-label="Navigation principale">
      {#each links as l}<a href={l.href} class:on={top === l.key} aria-current={top === l.key ? 'page' : undefined}>{l.label}</a>{/each}
    </nav>
    <div class="top-right">
      <button class="icon-btn" aria-label="Rechercher (Ctrl+K)" title="Rechercher (Ctrl+K)" onclick={() => (paletteOpen = true)}><Icon name="search" /></button>
      <button class="icon-btn" aria-label={dark ? 'Passer en thème clair' : 'Passer en thème sombre'} title={dark ? 'Passer en thème clair' : 'Passer en thème sombre'} onclick={toggleTheme}><Icon name={dark ? 'sun' : 'moon'} /></button>
      <Menu label="Mon compte" iconOnly btnClass="av-btn" class="user-zone" menuClass="user-menu" bind:open={userOpen}>
        {#snippet trigger()}<Avatar name={me.account.name} color={me.role.color} />{/snippet}
        <div class="who"><b>{me.account.name}</b><span>{me.role.name} · {me.account.email}</span></div>
        <button type="button" role="menuitem" onclick={() => (pwOpen = true)}><Icon name="lock" />Changer mon mot de passe</button>
        <button type="button" role="menuitem" onclick={logout}><Icon name="arrowr" />Se déconnecter</button>
      </Menu>
    </div>
  </header>
  <main id="view" class:wsv={wsView} tabindex="-1">{@render children()}</main>
</div>

<aside class="sidebar" class:open={menuOpen} aria-label="Menu" aria-hidden={!menuOpen}>
  <div class="menu-top"><span class="word"><Brand /></span><button class="icon-btn" aria-label="Fermer le menu (Échap)" title="Fermer le menu (Échap)" onclick={() => (menuOpen = false)}><Icon name="x" /></button></div>
  <nav class="menu-links" aria-label="Menu principal">
    {#each links as l}<a href={l.href} class:on={top === l.key}>{l.label}</a>{/each}
  </nav>
  <div class="side-foot">
    <Avatar name={me.account.name} color={me.role.color} size="lg" />
    <div><b>{me.account.name}</b><span>{me.role.name}</span></div>
    <button class="btn sm" style="margin-left:auto" onclick={toggleTheme}><Icon name={dark ? 'sun' : 'moon'} size="sm" />Thème</button>
  </div>
  <div class="row-actions" style="justify-content:flex-start">
    <button class="btn sm" onclick={() => { pwOpen = true; menuOpen = false; }}><Icon name="lock" size="sm" />Mot de passe</button>
    <button class="btn sm" onclick={logout}>Se déconnecter</button>
  </div>
</aside>

{#if pwOpen}<PasswordDialog onclose={() => (pwOpen = false)} />{/if}
