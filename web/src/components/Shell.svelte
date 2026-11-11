<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';
  import Avatar from './Avatar.svelte';
  import PasswordDialog from './PasswordDialog.svelte';
  import { session, can } from '../lib/session.svelte';
  import { route, navigate } from '../lib/router.svelte';
  import { isDark, toggleTheme, theme } from '../lib/theme.svelte';
  import { api } from '../lib/api';
  import { fail } from '../lib/toast.svelte';

  let { children }: { children: Snippet } = $props();
  let menuOpen = $state(false);
  let userOpen = $state(false);
  let pwOpen = $state(false);

  const top = $derived(route.path.split('/')[1] ?? '');
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
  function outside(e: MouseEvent) {
    if (userOpen && !(e.target as Element).closest('.user-zone')) userOpen = false;
  }
</script>

<svelte:window onclick={outside} onkeydown={(e) => { if (e.key === 'Escape') { userOpen = false; menuOpen = false; } }} />

<div class="app">
  <header class="mast">
    <button class="icon-btn menu-btn" aria-label="Ouvrir le menu" onclick={() => (menuOpen = true)}><Icon name="menu" /></button>
    <a class="word" href="/" aria-label="Strata Arkyo, accueil"><Logo /><span>Strata Arkyo</span></a>
    <nav class="topnav" aria-label="Navigation principale">
      {#each links as l}<a href={l.href} class:on={top === l.key} aria-current={top === l.key ? 'page' : undefined}>{l.label}</a>{/each}
    </nav>
    <div class="top-right">
      <button class="icon-btn" aria-label={dark ? 'Passer en thème clair' : 'Passer en thème sombre'} onclick={toggleTheme}><Icon name={dark ? 'sun' : 'moon'} /></button>
      <div class="user-zone" style="position:relative">
        <button class="av-btn" aria-label="Mon compte" aria-expanded={userOpen} onclick={() => (userOpen = !userOpen)}><Avatar name={me.account.name} color={me.role.color} /></button>
        {#if userOpen}
          <div class="user-menu" role="menu">
            <div class="who"><b>{me.account.name}</b><span>{me.role.name} · {me.account.email}</span></div>
            <button role="menuitem" onclick={() => { pwOpen = true; userOpen = false; }}><Icon name="lock" />Changer mon mot de passe</button>
            <button role="menuitem" onclick={logout}><Icon name="arrowr" />Se déconnecter</button>
          </div>
        {/if}
      </div>
    </div>
  </header>
  <main id="view" tabindex="-1">{@render children()}</main>
</div>

<aside class="sidebar" class:open={menuOpen} aria-label="Menu" aria-hidden={!menuOpen}>
  <div class="menu-top"><span class="word"><Logo /><span>Strata Arkyo</span></span><button class="icon-btn" aria-label="Fermer le menu" onclick={() => (menuOpen = false)}><Icon name="x" /></button></div>
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
