// Navigation par l'historique du navigateur : chaque écran a une adresse propre,
// que le moteur sert toujours avec la même page (application à page unique).

export const route = $state({ path: location.pathname, query: new URLSearchParams(location.search) });

function sync() {
  route.path = location.pathname;
  route.query = new URLSearchParams(location.search);
}

export function navigate(to: string, replace = false) {
  if (to === location.pathname + location.search) return;
  if (replace) history.replaceState(null, '', to);
  else history.pushState(null, '', to);
  sync();
  // La page défile dans <main id="view"> (la coque reste fixe).
  document.getElementById('view')?.scrollTo(0, 0);
}

addEventListener('popstate', sync);

/** Intercepte les clics sur les liens internes (<a href="/…">) pour ne pas recharger la page. */
addEventListener('click', (e) => {
  if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
  const a = (e.target as Element).closest?.('a[href]') as HTMLAnchorElement | null;
  if (!a || a.target || a.hasAttribute('download') || a.origin !== location.origin || a.pathname.startsWith('/api/')) return;
  e.preventDefault();
  navigate(a.pathname + a.search);
});

/** Premier segment de l'adresse : « catalogue », « admin »… */
export function section(): string {
  return route.path.split('/')[1] ?? '';
}
