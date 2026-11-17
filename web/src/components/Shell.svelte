<script lang="ts">
  // Coquille du design commun Strata : barre du haut de 46 px (marque, groupe d'onglets, pastille de
  // connexion, recherche, aide, menu utilisateur avec langue et thème), contenu pleine hauteur dessous.
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import Brand from './Brand.svelte';
  import StatusPill from './StatusPill.svelte';
  import PasswordDialog from './PasswordDialog.svelte';
  import CommandPalette from './CommandPalette.svelte';
  import ShortcutsHelp, { type ShortcutGroup } from './ShortcutsHelp.svelte';
  import Menu from './Menu.svelte';
  import { modalOpen } from '../lib/modal';
  import { session, can } from '../lib/session.svelte';
  import { route, navigate } from '../lib/router.svelte';
  import { theme, setTheme, type Theme } from '../lib/theme.svelte';
  import { api } from '../lib/api';
  import { fail } from '../lib/toast.svelte';
  import { initials } from '../lib/format';

  let { children }: { children: Snippet } = $props();
  let menuOpen = $state(false);
  let userOpen = $state(false);
  let pwOpen = $state(false);
  let paletteOpen = $state(false);
  let helpOpen = $state(false);

  const top = $derived(route.path.split('/')[1] ?? '');
  // Projet ouvert, catalogue et administration : liste à gauche, page à droite, pleine hauteur.
  const wsView = $derived(top === 'catalogue' || top === 'admin' || (top === 'projets' && !!route.path.split('/')[2]));
  const links = $derived([
    { key: '', label: 'Accueil', href: '/', icon: 'home', show: true },
    { key: 'projets', label: 'Projets', href: '/projets', icon: 'folder', show: can('projets', 'lecture') },
    { key: 'catalogue', label: 'Catalogue', href: '/catalogue', icon: 'box', show: can('catalogue', 'lecture') },
    { key: 'admin', label: 'Administration', href: '/admin', icon: 'shield', show: can('membres', 'admin') || can('parametres', 'admin') },
  ].filter((l) => l.show));
  const me = $derived(session.me!);
  const THEME_OPTS: { value: Theme; label: string }[] = [{ value: 'dark', label: 'Sombre' }, { value: 'light', label: 'Clair' }, { value: 'system', label: 'Système' }];
  // Aide : les raccourcis communs à toute l'application (chaque éditeur a aussi les siens, touche « ? »).
  const HELP: ShortcutGroup[] = [
    ['Partout', [['Ctrl K', 'Rechercher'], ['Échap', 'Fermer le menu ou la fenêtre'], ['↑ ↓ Entrée', 'Choisir dans un menu']]],
    ['Éditeurs', [['?', 'Raccourcis de l’éditeur'], ['Ctrl Z', 'Annuler'], ['Suppr', 'Supprimer la sélection']]],
  ];

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
  <header class="topbar">
    <button class="icon-btn ghost menu-btn" aria-label="Ouvrir le menu" title="Ouvrir le menu" aria-expanded={menuOpen} onclick={() => (menuOpen = true)}><Icon name="menu" /></button>
    <a class="brand-link" href="/" aria-label="Strata Arkyo, accueil"><Brand /></a>
    <nav class="tabs home-tabs" aria-label="Navigation principale">
      {#each links as l}<a href={l.href} class:on={top === l.key} aria-current={top === l.key ? 'page' : undefined}><Icon name={l.icon} size="sm" /><span class="tab-label">{l.label}</span></a>{/each}
    </nav>
    <div class="spacer"></div>
    <StatusPill />
    <button class="learn-help" aria-label="Rechercher (Ctrl+K)" title="Rechercher (Ctrl+K)" onclick={() => (paletteOpen = true)}><Icon name="search" /></button>
    <button class="learn-help" class:on={helpOpen} aria-label="Aide" title="Aide" onclick={() => (helpOpen = true)}><Icon name="help" /></button>
    <Menu label="Mon compte" btnClass="ux1-user-btn named {userOpen ? 'on' : ''}" class="ux1-user" menuClass="menu ux1-umenu" bind:open={userOpen}>
      {#snippet trigger()}<span class="avatar">{initials(me.account.name).slice(0, 1)}</span><span class="user-name">{me.account.name}</span><Icon name="chevd" size="sm" />{/snippet}
      <div class="ux1-who"><span class="avatar">{initials(me.account.name).slice(0, 1)}</span><div><b>{me.account.name}</b><small>{me.role.name}</small></div></div>
      <div class="menu-sep"></div>
      <div class="ux1-mrow"><Icon name="lang" size="sm" />Langue
        <div class="lang-switch" role="group" aria-label="Langue"><button type="button" class="on" aria-pressed="true" title="Français">FR</button></div>
      </div>
      <div class="ux1-mrow"><Icon name="sunmoon" size="sm" />Thème
        <div class="lang-switch theme-switch" role="group" aria-label="Thème">
          {#each THEME_OPTS as o}<button type="button" class:on={theme.value === o.value} aria-pressed={theme.value === o.value} onclick={() => setTheme(o.value)}>{o.label}</button>{/each}
        </div>
      </div>
      <button type="button" role="menuitem" class="ux1-mi" onclick={() => (helpOpen = true)}><Icon name="keys" size="sm" />Raccourcis clavier<span class="ux1-cap">?</span></button>
      <button type="button" role="menuitem" class="ux1-mi" onclick={() => (pwOpen = true)}><Icon name="lock" size="sm" />Changer de mot de passe</button>
      <div class="menu-sep"></div>
      <button type="button" role="menuitem" class="ux1-mi danger" onclick={logout}><Icon name="logout" size="sm" />Se déconnecter</button>
    </Menu>
  </header>
  <main id="view" class="content" class:wsv={wsView} tabindex="-1">{@render children()}</main>
</div>

<aside class="sidebar" class:open={menuOpen} aria-label="Menu" aria-hidden={!menuOpen}>
  <div class="menu-top"><Brand /><button class="icon-btn ghost" aria-label="Fermer le menu (Échap)" title="Fermer le menu (Échap)" onclick={() => (menuOpen = false)}><Icon name="x" /></button></div>
  <nav class="menu-links" aria-label="Menu principal">
    {#each links as l}<a href={l.href} class="lib-nav-item" class:on={top === l.key}><Icon name={l.icon} size="sm" /><span class="grow">{l.label}</span></a>{/each}
  </nav>
  <div class="side-foot">
    <span class="avatar">{initials(me.account.name).slice(0, 1)}</span>
    <div><b>{me.account.name}</b><span>{me.role.name}</span></div>
  </div>
  <div class="ux1-mrow"><Icon name="sunmoon" size="sm" />Thème
    <div class="lang-switch theme-switch" role="group" aria-label="Thème">
      {#each THEME_OPTS as o}<button type="button" class:on={theme.value === o.value} aria-pressed={theme.value === o.value} onclick={() => setTheme(o.value)}>{o.label}</button>{/each}
    </div>
  </div>
  <div class="row-actions" style="justify-content:flex-start">
    <button class="btn small" onclick={() => { pwOpen = true; menuOpen = false; }}><Icon name="lock" size="sm" />Mot de passe</button>
    <button class="btn small danger" onclick={logout}><Icon name="logout" size="sm" />Se déconnecter</button>
  </div>
</aside>

{#if pwOpen}<PasswordDialog onclose={() => (pwOpen = false)} />{/if}
{#if helpOpen}<ShortcutsHelp title="Raccourcis clavier" eyebrow="Aide" groups={HELP} onclose={() => (helpOpen = false)} />{/if}
